//! Notion sync (after Notion Lite's client, note_repository): the library's
//! Markdown notes (`.md`, top level and folders) against a Notion parent page,
//! through the REST API (Notion-Version 2022-06-28). One-way both ways, chosen
//! by the call: [`push`] writes local files to pages (create or update),
//! [`pull`] reads pages the library has no file for. What each note came from
//! and went to is in the state file (`.quiet/notion-sync.json`), keyed by the
//! file's path relative to the library.
//!
//! Requests go through [`remote::Http`] — the system curl on desktop — so the
//! token never shows in a process list and a proxy can be set (`sync_dir_with`).

use crate::remote::{CurlHttp, Http, Request};
use crate::vfs::PathExt;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const API: &str = "https://api.notion.com/v1";
const VERSION: &str = "2022-06-28";
/// How many blocks one append may carry (Notion's limit).
const CHUNK: usize = 100;
/// Notion Lite's wording for a new note with nowhere to go (its `syncNote`).
const NEEDS_PARENT: &str = "新建笔记需要父页面：先同步 Notion 或在设置里填写";

// ---- state

#[derive(Clone, Serialize, Deserialize, Default)]
struct Mapping {
    page_id: String,
    /// The page title Notion had, or the file name `pull` wrote.
    title: String,
    /// FNV-1a of the file's bytes at the last push (0 on a pull).
    pushed: u64,
    /// The page's `last_edited_time` at the last pull (empty on a push).
    last_edited: String,
}

#[derive(Serialize, Deserialize, Default)]
struct State(BTreeMap<String, Mapping>);

impl State {
    fn path(root: &Path) -> PathBuf {
        let internal = if root.join(".quiet").fs_is_dir() { root.join(".quiet") } else { root.to_path_buf() };
        internal.join("notion-sync.json")
    }
    fn load(root: &Path) -> State {
        std::fs::read(State::path(root)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
    }
    fn save(&self, root: &Path) -> Result<()> {
        let path = State::path(root);
        if let Some(p) = path.parent() { std::fs::create_dir_all(p)?; }
        let bytes = serde_json::to_vec_pretty(self)?;
        let out = path.with_extension("tmp");
        std::fs::write(&out, bytes)?;
        std::fs::rename(&out, &path)?;
        Ok(())
    }
}

/// FNV-1a 64-bit; only decides "changed since the last sync".
fn fnv(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_3255;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

// ---- client

/// A Notion API client over an [`Http`].
pub struct Client<H: Http> {
    http: H,
    token: String,
}

impl Client<CurlHttp> {
    /// The real client: the system curl, with `proxy` (`socks5://…` or
    /// `http://…`) when one is given.
    pub fn new(token: &str, proxy: Option<(String, String)>) -> Client<CurlHttp> {
        let mut http = CurlHttp::default();
        http.proxy = proxy;
        Client { http, token: token.trim().to_string() }
    }
}

impl<H: Http> Client<H> {
    fn request(&self, method: &str, path: &str, body: Option<Value>) -> Result<Value> {
        let body = match body {
            Some(v) => Some(serde_json::to_vec(&v)?),
            None => None,
        };
        let req = Request {
            method: method.into(),
            url: format!("{API}{path}"),
            headers: vec![
                ("Authorization".into(), format!("Bearer {}", self.token)),
                ("Notion-Version".into(), VERSION.into()),
                ("Content-Type".into(), "application/json".into()),
            ],
            body,
        };
        let resp = self.http.send(req)?;
        let ok = resp.status == 200;
        let v: Value = serde_json::from_slice(&resp.body).unwrap_or(Value::Null);
        if !ok {
            let msg = v["message"].as_str().unwrap_or("request failed").to_string();
            anyhow::bail!("notion {}: {}", resp.status, msg);
        }
        Ok(v)
    }

    /// Who the integration is: a token check (`v1/users/me`).
    pub fn me(&self) -> Result<String> {
        let v = self.request("GET", "/users/me", None)?;
        Ok(v["name"].as_str().unwrap_or("?").to_string())
    }

    /// Every page the integration can reach (paginated); Notion's search has
    /// no parent filter, so Notion Lite filtered client-side and so does this.
    pub fn search_pages(&self) -> Result<Vec<Value>> {
        let mut out = vec![];
        let mut cursor: Option<String> = None;
        loop {
            let mut body = json!({"filter": {"property": "object", "value": "page"}, "page_size": 100});
            if let Some(c) = &cursor { body["start_cursor"] = Value::from(c.clone()); }
            let v = self.request("POST", "/search", Some(body));
            let v = match v { Ok(v) => v, Err(e) => if out.is_empty() { return Err(e) } else { break } };
            out.extend(v["results"].as_array().cloned().unwrap_or_default());
            cursor = v["next_cursor"].as_str().map(str::to_string);
            if v["has_more"].as_bool() != Some(true) { break; }
        }
        Ok(out)
    }

    /// A page's blocks, following `start_cursor` to the end.
    pub fn block_children(&self, page_id: &str) -> Result<Vec<Value>> {
        let mut out = vec![];
        let mut cursor: Option<String> = None;
        loop {
            let mut path = format!("/blocks/{page_id}/children?page_size=100");
            if let Some(c) = &cursor { path = format!("/blocks/{page_id}/children?page_size=100&start_cursor={c}"); }
            let v = self.request("GET", &path, None)?;
            out.extend(v["results"].as_array().cloned().unwrap_or_default());
            cursor = v["next_cursor"].as_str().map(str::to_string);
            if v["has_more"].as_bool() != Some(true) { break; }
        }
        Ok(out)
    }

    /// A new page under `parent` with `children` (chunked against Notion's
    /// limit: the first chunk is the page, the rest appended).
    pub fn create_page(&self, parent: &str, title: &str, children: Vec<Value>) -> Result<String> {
        let mut items = children.chunks(CHUNK);
        let first = items.next().unwrap_or(&[]).to_vec();
        let page = self.request("POST", "/pages", Some(json!({
            "parent": {"page_id": parent},
            "properties": {"title": [{"text": {"content": title}}]},
            "children": first,
        })))?;
        let id = page["id"].as_str().unwrap_or_default().to_string();
        for chunk in items {
            self.append(&id, chunk.to_vec())?;
        }
        Ok(id)
    }

    fn append(&self, parent: &str, chunk: Vec<Value>) -> Result<()> {
        self.request("PATCH", &format!("/blocks/{parent}/children"), Some(json!({"children": chunk})))?;
        Ok(())
    }

    /// One raw GET, for a pre-signed S3 file URL (no auth header wanted).
    pub fn fetch(&self, url: &str) -> Result<Vec<u8>> {
        let resp = self.http.send(Request { method: "GET".into(), url: url.into(), headers: vec![], body: None })?;
        if resp.status == 200 { Ok(resp.body) } else { anyhow::bail!("{}: GET {url}", resp.status) }
    }

    /// Upload one file (Notion Lite's `uploadFileToNotion`): the file upload
    /// is created, the bytes sent as multipart, and the `file_upload` id
    /// returned for an image or file block. The id is fresh every time —
    /// Notion's file ids do not survive a re-push, so each push re-uploads.
    pub fn upload_file(&self, filename: &str, bytes: &[u8]) -> Result<String> {
        let mime = mime_of(bytes, filename);
        let created = self.request("POST", "/file_uploads", Some(json!({"mode": "single", "filename": filename, "content_type": mime})))?;
        let id = created["id"].as_str().unwrap_or_default().to_string();
        let boundary = format!("quiet-{}", uuid::Uuid::new_v4());
        let mut body = Vec::new();
        body.extend(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\nContent-Type: {mime}\r\n\r\n").into_bytes());
        body.extend_from_slice(bytes);
        body.extend(format!("\r\n--{boundary}--\r\n").into_bytes());
        let resp = self.http.send(Request {
            method: "POST".into(),
            url: format!("{API}/file_uploads/{id}/send"),
            headers: vec![
                ("Authorization".into(), format!("Bearer {}", self.token)),
                ("Notion-Version".into(), VERSION.into()),
                ("Content-Type".into(), format!("multipart/form-data; boundary={boundary}")),
            ],
            body: Some(body),
        })?;
        if resp.status != 200 { anyhow::bail!("{}: uploading {filename}", resp.status); }
        Ok(id)
    }

    /// The page's title (its `title` property) as plain text.
    pub fn page_title(&self, page: &Value) -> String {
        page["properties"].as_object()
            .and_then(|props| props.values().find(|p| p["type"] == "title" || p.get("title").is_some()))
            .and_then(|p| p["title"].as_array().map(|a| a.iter().filter_map(|r| r["plain_text"].as_str()).collect::<Vec<_>>().join("")))
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| "无标题".into())
    }

    /// Replace the page's whole content: every block deleted, the new ones
    /// appended (Notion Lite's `updateExistingPage`).
    pub fn update_page_content(&self, page_id: &str, children: Vec<Value>) -> Result<()> {
        for block in self.block_children(page_id)? {
            if let Some(id) = block["id"].as_str() {
                let _ = self.request("DELETE", &format!("/blocks/{id}"), None);
            }
        }
        for chunk in children.chunks(CHUNK) {
            self.append(page_id, chunk.to_vec())?;
        }
        Ok(())
    }

    /// Retitle the page (`PATCH /pages`).
    pub fn rename_page(&self, page_id: &str, title: &str) -> Result<()> {
        self.request("PATCH", &format!("/pages/{page_id}"), Some(json!({
            "properties": {"title": [{"text": {"content": title}}]}
        })))?;
        Ok(())
    }
}

// ---- markdown ⇄ blocks

/// The Markdown of one page's blocks, in the shape Notion Lite reads back
/// (`parsePageBlocks`): headings, lists, to-dos, quotes, callouts and code
/// fences; images and files keep their URL.
pub fn blocks_to_markdown(blocks: &[Value]) -> String {
    blocks_to_markdown_with(blocks, &mut |_| None)
}

/// `blocks_to_markdown`, letting the caller take an image or file's URL and
/// return its Markdown in place — the local copy's, for the library's pulls.
pub fn blocks_to_markdown_with(blocks: &[Value], resolve: &mut dyn FnMut(&str) -> Option<String>) -> String {
    let mut out = String::new();
    for b in blocks {
        let t = b["type"].as_str().unwrap_or("");
        let body = &b[t];
        let text: String = body["rich_text"].as_array().map(|a| a.iter().map(|r| r["plain_text"].as_str().unwrap_or("")).collect::<Vec<_>>().join("")).unwrap_or_default();
        match t {
            "heading_1" => out.push_str(&format!("# {text}\n\n")),
            "heading_2" => out.push_str(&format!("## {text}\n\n")),
            "heading_3" => out.push_str(&format!("### {text}\n\n")),
            "bulleted_list_item" => out.push_str(&format!("- {text}\n")),
            "numbered_list_item" => out.push_str(&format!("1. {text}\n")),
            "to_do" => out.push_str(&format!("- [{}] {text}\n", if body["checked"].as_bool() == Some(true) { "x" } else { " " })),
            "quote" => out.push_str(&format!("> {text}\n\n")),
            "callout" => out.push_str(&format!("> [!NOTE] {text}\n\n")),
            "code" => out.push_str(&format!("```{}\n{}\n```\n\n", body["language"].as_str().unwrap_or(""), text)),
            "divider" => out.push_str("---\n\n"),
            "image" | "file" => {
                let url = body["file"]["url"].as_str().or(body["external"]["url"].as_str()).unwrap_or("");
                let alt = if t == "image" { "图片" } else { "附件" };
                let line = resolve(url).unwrap_or_else(|| format!("![{alt}]({url})"));
                out.push_str(&format!("{line}\n\n"));
            }
            _ => if !text.is_empty() { out.push_str(&format!("{text}\n\n")) },
        }
    }
    while out.ends_with("\n\n\n") { out.pop(); }
    out
}

/// Notion children blocks for a Markdown note, in the shape Notion Lite writes
/// (`buildNoteChildren`): one block per Markdown paragraph or list item.
pub fn markdown_to_blocks(md: &str) -> Vec<Value> {
    markdown_to_blocks_with(md, &mut |_| None)
}

/// `markdown_to_blocks`, letting the caller take an image's URL or local path
/// and return a Notion `file_upload` id — the local pictures a push uploads.
pub fn markdown_to_blocks_with(md: &str, resolve: &mut dyn FnMut(&str) -> Option<String>) -> Vec<Value> {
    let mut blocks = vec![];
    let mut para = String::new();
    let mut fence: Option<(String, String)> = None;

    let flush = |para: &mut String, blocks: &mut Vec<Value>| {
        if !para.trim().is_empty() {
            blocks.push(text_block("paragraph", para.trim()));
        }
        para.clear();
    };

    for line in md.lines() {
        if let Some((lang, mut code)) = fence.take() {
            if line.trim_end().ends_with("```") {
                blocks.push(json!({"object": "block", "type": "code", "code": {
                    "rich_text": [text_rich(code.trim_end())], "language": if lang.is_empty() { "plain text" } else { lang.as_str() }
                }}));
            } else {
                code.push_str(line);
                code.push('\n');
                fence = Some((lang, code));
            }
            continue;
        }
        let trimmed = line.trim_end();
        if let Some(lang) = trimmed.strip_prefix("```") {
            flush(&mut para, &mut blocks);
            fence = Some((lang.trim().to_string(), String::new()));
            continue;
        }
        if trimmed.is_empty() {
            flush(&mut para, &mut blocks);
            continue;
        }
        if trimmed.starts_with("![") && trimmed.ends_with(')') && let Some(sep) = trimmed.find("](") {
            let url = trimmed[sep + 2..trimmed.len() - 1].to_string();
            flush(&mut para, &mut blocks);
            let block = match resolve(&url) {
                Some(id) => json!({"object": "block", "type": "image", "image": {"type": "file_upload", "file_upload": {"id": id}}}),
                None => json!({"object": "block", "type": "image", "image": {"type": "external", "external": {"url": url}}}),
            };
            blocks.push(block);
            continue;
        }
        if trimmed.starts_with('#') {
            let level = trimmed.chars().take_while(|c| *c == '#').count().min(3);
            if trimmed[level..].starts_with(' ') || level == 0 {
                flush(&mut para, &mut blocks);
                blocks.push(text_block(&format!("heading_{level}"), trimmed[level..].trim()));
                continue;
            }
        }
        if let Some(rest) = trimmed.strip_prefix("- [x] ").or(trimmed.strip_prefix("- [X] ")) {
            flush(&mut para, &mut blocks);
            blocks.push(to_do(rest, true));
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("- [ ] ") {
            flush(&mut para, &mut blocks);
            blocks.push(to_do(rest, false));
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("- ").or(trimmed.strip_prefix("* ")) {
            flush(&mut para, &mut blocks);
            blocks.push(text_block("bulleted_list_item", rest));
            continue;
        }
        let digits = trimmed.chars().take_while(|c| c.is_ascii_digit()).count();
        if digits > 0 && trimmed[digits..].starts_with(". ") {
            flush(&mut para, &mut blocks);
            blocks.push(text_block("numbered_list_item", trimmed[digits..].trim_start_matches(". ")));
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("> ") {
            flush(&mut para, &mut blocks);
            blocks.push(text_block("quote", rest));
            continue;
        }
        if trimmed == "---" {
            flush(&mut para, &mut blocks);
            blocks.push(json!({"object": "block", "type": "divider", "divider": {}}));
            continue;
        }
        if !para.is_empty() { para.push('\n'); }
        para.push_str(trimmed);
    }
    flush(&mut para, &mut blocks);
    blocks
}

fn text_rich(text: &str) -> Value {
    json!({"type": "text", "text": {"content": text}, "plain_text": text})
}

fn text_block(kind: &str, text: &str) -> Value {
    json!({"object": "block", "type": kind, kind: {"rich_text": [text_rich(text)]}})
}

fn to_do(text: &str, checked: bool) -> Value {
    json!({"object": "block", "type": "to_do", "to_do": {"rich_text": [text_rich(text)], "checked": checked}})
}

// ---- the sync itself

/// The library's Markdown notes, relative paths under `md/`: `.md` files
/// anywhere but a name starting with `.`.
pub fn list_markdown(root: &Path) -> Vec<String> {
    fn walk(dir: &Path, rel: &Path, out: &mut Vec<String>) {
        let entries = match std::fs::read_dir(dir) { Ok(e) => e, Err(_) => return };
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            let child_rel = rel.join(&name);
            let p = e.path();
            if p.is_dir() {
                if name.starts_with('.') { continue }
                walk(&p, &child_rel, out);
            } else if name.to_lowercase().ends_with(".md") && !name.starts_with('.') {
                out.push(child_rel.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    let notes = root.join("md");
    let mut out = vec![];
    walk(&notes, Path::new(""), &mut out);
    out.sort();
    out
}

/// A file name for a page title, the way the library names documents.
fn file_name_for(title: &str) -> String {
    let clean: String = title.chars()
        .map(|c| if matches!(c, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|') { ' ' } else { c })
        .collect();
    let clean = clean.trim();
    if clean.is_empty() { "无标题.md".into() } else { format!("{clean}.md") }
}

/// Push: every `.md` under `root` up to Notion under `parent` (a page id, or a
/// full page URL — the bare id is taken from it). A note not seen before gets
/// a page; a note pushed before is rewritten when its file changed. `parent`
/// is optional: like Notion Lite, a new note without one fails with a hint to
/// sync first (which imports the pages to pick from).
/// Returns (created, updated, failed).
pub fn push<H: Http>(root: &Path, client: &Client<H>, parent: Option<&str>) -> Result<(usize, usize, Vec<String>)> {
    let parent = parent.map(page_id).filter(|p| !p.is_empty());
    let image_ref = regex::Regex::new(r"!\[[^\]]*\]\(([^)]+)\)").unwrap();
    let mut state = State::load(root);
    let (mut created, mut updated) = (0, 0);
    let mut failed = vec![];
    for rel in list_markdown(root) {
        let bytes = match std::fs::read(root.join("md").join(&rel)) { Ok(b) => b, Err(_) => { failed.push(rel); continue } };
        let title = rel.rsplit('/').next().unwrap_or(&rel).trim_end_matches(".md").to_string();
        let hash = fnv(&bytes);
        let known = state.0.get(&rel).cloned();
        if let Some(m) = &known && m.pushed == hash && !m.page_id.is_empty() { continue; }
        let md = String::from_utf8_lossy(&bytes).to_string();
        // 本地图片上传 Notion：file id 每次推送都会变，直接重传（Notion
        // Lite 的 `uploadFileToNotion` 同样如此）。远端 URL 原样保留。
        let assets = root.join("assets");
        let mut uploads: BTreeMap<String, String> = BTreeMap::new();
        let md = image_ref.replace_all(&md, |caps: &regex::Captures| {
            let url = caps[1].to_string();
            if !url.contains("assets/") { return format!("![图片]({url})"); }
            let name = url.rsplit('/').next().unwrap_or("").to_string();
            if name.is_empty() { return format!("![图片]({url})"); }
            if let Some(id) = uploads.get(&name) { return format!("![图片](notion-file://{id})"); }
            match std::fs::read(assets.join(&name)) {
                Ok(bytes) => match client.upload_file(&name, &bytes) {
                    Ok(id) => { uploads.insert(name.clone(), id.clone()); format!("![图片](notion-file://{id})") }
                    Err(e) => { failed.push(format!("{rel}: {name}: {e}")); format!("![图片]({url})") }
                },
                Err(_) => format!("![图片]({url})"),
            }
        }).to_string();
        let mut resolve = |url: &str| -> Option<String> { url.strip_prefix("notion-file://").map(str::to_string) };
        let blocks = markdown_to_blocks_with(&md, &mut resolve);
        let result = match &known {
            Some(m) if !m.page_id.is_empty() => {
                client.rename_page(&m.page_id, &title)?;
                client.update_page_content(&m.page_id, blocks).map(|_| m.page_id.clone())
            }
            _ if parent.is_none() => {
                failed.push(format!("{rel}: {NEEDS_PARENT}"));
                continue;
            }
            _ => client.create_page(parent.as_deref().unwrap(), &title, blocks),
        };
        match result {
            Ok(id) => {
                state.0.insert(rel.clone(), Mapping { page_id: id, title, pushed: hash, last_edited: String::new() });
                if known.is_some() { updated += 1; } else { created += 1; }
            }
            Err(e) => failed.push(format!("{rel}: {e}")),
        }
    }
    if failed.is_empty() { state.save(root)?; }
    Ok((created, updated, failed))
}

/// Pull: every page the integration can reach (Notion Lite's `syncFromNotion`
/// walks the whole search result) becomes `.md` under the library's `md/`
/// folder, nested in the folders of its ancestor pages — a 📁/📂 page is a
/// folder, not a note (Notion Lite's `isFolderPage`) — and every image is
/// downloaded into `assets/`, so notes read offline. Pages already mapped are
/// refetched only when Notion's `last_edited_time` moved since the last pull.
/// Returns (imported, refreshed, failed).
pub fn pull<H: Http>(root: &Path, client: &Client<H>) -> Result<(usize, usize, Vec<String>)> {
    let notes = root.join("md");
    let assets = root.join("assets");
    let mut state = State::load(root);
    let (mut imported, mut refreshed) = (0, 0);
    let mut failed = vec![];
    // The whole reachable space first: each page's title, parent and edit
    // time. The parents are what give the files their folder structure.
    let mut info: BTreeMap<String, (String, Option<String>, String)> = BTreeMap::new();
    for page in client.search_pages()? {
        if page["archived"].as_bool() == Some(true) || page["in_trash"].as_bool() == Some(true) { continue }
        if let Some(id) = page["id"].as_str() {
            let parent = page["parent"]["page_id"].as_str().map(str::to_string);
            info.insert(id.to_string(), (client.page_title(&page), parent, page["last_edited_time"].as_str().unwrap_or("").to_string()));
        }
    }
    for (id, (title, parent, edited)) in &info {
        if folder_title(title) { continue }
        if let Some(m) = state.0.values().find(|m| m.page_id == *id) && m.last_edited == *edited { continue; }
        // The ancestors top-down are the folders, each a page's title itself.
        let mut parts: Vec<String> = vec![];
        let mut cur = parent.clone();
        let mut guard = 0;
        while let Some(p) = cur && guard < 20 {
            guard += 1;
            match info.get(&p) { Some((t, pp, _)) => { parts.push(clean_name(t)); cur = pp.clone(); }, None => break }
        }
        // Notes live one level down (`md/`), so every asset reference needs
        // one `../` more than the folder depth.
        let depth = parts.len();
        let (dir, rel) = if depth == 0 { (notes.clone(), file_name_for(title)) } else {
            let dir = notes.join(parts.iter().collect::<PathBuf>());
            (dir.clone(), format!("{}/{}", parts.join("/"), file_name_for(title)))
        };
        let blocks = match client.block_children(id) {
            Ok(b) => b,
            Err(e) => { failed.push(format!("{title}: {e}")); continue }
        };
        // Images and files land in the library's `assets/`, referenced
        // relative to the note (one `../` per folder level, plus `md/`).
        let mut resolve = |url: &str| -> Option<String> {
            let bytes = client.fetch(url).ok()?;
            let name = format!("{}{}", fnv(url.as_bytes()), ext_of(&bytes, url));
            std::fs::create_dir_all(&assets).ok()?;
            std::fs::write(assets.join(&name), &bytes).ok()?;
            Some(format!("![图片]({}assets/{name})", "../".repeat(depth + 1)))
        };
        let md = blocks_to_markdown_with(&blocks, &mut resolve);
        if std::fs::create_dir_all(&dir).is_ok() && std::fs::write(dir.join(file_name_for(title)), md.as_bytes()).is_ok() {
            let old = state.0.get(&rel).cloned();
            state.0.insert(rel.clone(), Mapping { page_id: id.clone(), title: title.clone(), pushed: 0, last_edited: edited.clone() });
            if old.is_some() { refreshed += 1; } else { imported += 1; }
        } else {
            failed.push(title.clone());
        }
    }
    if failed.is_empty() { state.save(root)?; }
    Ok((imported, refreshed, failed))
}

/// A folder page: 📁/📂 in the icon or at the title's start (Notion Lite's
/// `isFolderPage`). Folder pages hold other pages; they are not notes.
fn folder_title(title: &str) -> bool {
    title.starts_with("📁") || title.starts_with("📂")
}

/// A title as a file or folder name on this disk: Notion Lite's character
/// sweep, the folder emojis gone.
fn clean_name(title: &str) -> String {
    let clean: String = title.trim_start_matches("📁").trim_start_matches("📂").trim().chars()
        .map(|c| if matches!(c, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|') { ' ' } else { c })
        .collect();
    let clean = clean.trim().trim_start_matches("📁").trim();
    if clean.is_empty() { "未命名".into() } else { clean.to_string() }
}

/// A file extension by the downloaded bytes' magic, else by the URL's tail.
fn ext_of(bytes: &[u8], url: &str) -> String {
    let from_magic = if bytes.starts_with(b"\x89PNG") { "png" }
        else if bytes.starts_with(b"\xff\xd8") { "jpg" }
        else if bytes.starts_with(b"GIF8") { "gif" }
        else if bytes.len() > 12 && &bytes[8..12] == b"WEBP" { "webp" }
        else if bytes.starts_with(b"%PDF") { "pdf" }
        else { return url.rsplit('.').next().unwrap_or("bin").split('?').next().unwrap_or("bin").chars().take(8).collect() };
    from_magic.into()
}

/// A MIME type by the bytes' magic, else `application/octet-stream`.
fn mime_of(bytes: &[u8], filename: &str) -> String {
    let ext = ext_of(bytes, filename);
    match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    }.into()
}

/// The bare page id out of a full `https://www.notion.so/…-<id>` URL or an id
/// with its dashes stripped.
pub fn page_id(s: &str) -> String {
    let s = s.trim();
    let id = s.rsplit('-').find(|p| p.len() == 32 && p.chars().all(|c| c.is_ascii_alphanumeric()));
    id.map(str::to_string).unwrap_or_else(|| s.to_string())
}

/// Push then pull, once: `sync_dir_with(root, token, parent, proxy)`. `parent`
/// is optional (None or empty: pull everything, push only rewrites known
/// notes); the proxy is `(url, hosts-to-bypass)`, e.g. `("socks5://127.0.0.1:10810", "")`.
pub fn sync_dir_with(root: &Path, token: &str, parent: Option<&str>, proxy: Option<(String, String)>) -> Result<String> {
    let client = Client::<CurlHttp>::new(token, proxy);
    let who = client.me()?;
    let (c, u, cf) = push(root, &client, parent)?;
    let (i, r, pf) = pull(root, &client)?;
    Ok(format!(
        "已连接 {who}；推送 新建{c} 更新{u} 失败{}；拉取 导入{i} 刷新{r} 失败{}",
        cf.len(), pf.len()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_round_trips_to_the_same_blocks() {
        let md = "# 标题\n\n一段话。\n\n- 项目\n- [x] 已办\n- [ ] 未办\n\n1. 第一\n> 引用\n```\ncode()\n```\n---\n";
        let blocks = markdown_to_blocks(md);
        let kinds: Vec<&str> = blocks.iter().map(|b| b["type"].as_str().unwrap()).collect();
        assert_eq!(kinds, vec!["heading_1", "paragraph", "bulleted_list_item", "to_do", "to_do", "numbered_list_item", "quote", "code", "divider"]);
        let back = blocks_to_markdown(&blocks);
        assert!(back.contains("# 标题"));
        assert!(back.contains("- [x] 已办"));
        assert!(back.contains("- [ ] 未办"));
        assert!(back.contains("1. 第一"));
        assert!(back.contains("> 引用"));
        assert!(back.contains("code()"));
        assert!(back.contains("---"));
    }

    #[test]
    fn page_id_takes_the_url_tail() {
        assert_eq!(page_id("https://www.notion.so/My-Notes-1234abcd1234abcd1234abcd1234abcd"), "1234abcd1234abcd1234abcd1234abcd");
        assert_eq!(page_id("abc123"), "abc123");
    }

    /// A fake Http: records the requests, answers canned responses.
    struct Fake { last: std::rc::Rc<std::cell::RefCell<Vec<Request>>> }
    impl Http for Fake {
        fn send(&self, req: Request) -> crate::remote::Result<crate::remote::Response> {
            self.last.borrow_mut().push(req);
            Ok(crate::remote::Response { status: 200, headers: vec![], body: serde_json::to_vec(&json!({"id": "page-1", "name": "Workspace", "has_more": false, "results": []})).unwrap() })
        }
    }

    #[test]
    fn requests_carry_the_token_and_version() {
        let last = std::rc::Rc::new(std::cell::RefCell::new(vec![]));
        let client = Client { http: Fake { last: last.clone() }, token: "secret".into() };
        let _ = client.me();
        let req = last.borrow()[0].clone();
        assert_eq!(req.url, "https://api.notion.com/v1/users/me");
        assert_eq!(req.headers[0].1, "Bearer secret");
        assert_eq!(req.headers[1].1, "2022-06-28");
    }

    #[test]
    fn state_round_trips() {
        let dir = std::env::temp_dir().join(format!("quiet-notion-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut s = State::default();
        s.0.insert("a.md".into(), Mapping { page_id: "p1".into(), title: "a".into(), pushed: fnv(b"a"), last_edited: String::new() });
        s.save(&dir).unwrap();
        let back = State::load(&dir);
        assert_eq!(back.0.get("a.md").unwrap().page_id, "p1");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
