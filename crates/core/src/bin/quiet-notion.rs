//! `quiet-notion`: the Notion sync of the library, from the command line.
//!
//! ```text
//! QUIET_NOTION_TOKEN=secret_xxx        (Notion integration token)
//! QUIET_NOTION_PARENT=<page id or URL> (where notes are created / read from)
//! QUIET_NOTION_PROXY=socks5://127.0.0.1:10810   (optional)
//! quiet-notion [push|pull|sync] [library folder]
//! ```

use fdl_core::notion;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let action = args.first().map(String::as_str).unwrap_or("sync");
    let token = std::env::var("QUIET_NOTION_TOKEN").unwrap_or_default();
    let parent = std::env::var("QUIET_NOTION_PARENT").unwrap_or_default();
    if token.is_empty() {
        eprintln!("需要 QUIET_NOTION_TOKEN 环境变量");
        std::process::exit(2);
    }
    let root = match args.get(1) {
        Some(dir) => std::path::PathBuf::from(dir),
        None => fdl_core::store::Store::default_dir(),
    };
    let proxy = std::env::var("QUIET_NOTION_PROXY").ok().map(|p| (p, String::new()));
    let client = notion::Client::new(&token, proxy.clone());
    let result = match action {
        "push" => notion::push(&root, &client, Some(&parent)).map(|(c, u, f)| format!("推送 新建{c} 更新{u} 失败{}", f.len())),
        "pull" => notion::pull(&root, &client).map(|(i, r, f)| format!("拉取 导入{i} 刷新{r} 失败{}", f.len())),
        "sync" => notion::sync_dir_with(&root, &token, Some(&parent), proxy),
        other => { eprintln!("未知操作 {other}：push | pull | sync"); std::process::exit(2); }
    };
    match result {
        Ok(msg) => println!("{msg}"),
        Err(e) => { eprintln!("同步失败：{e}"); std::process::exit(1); }
    }
}
