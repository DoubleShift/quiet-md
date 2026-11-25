"""What an import brings in is in front as soon as it finishes: a Markdown
file opens, an imported folder (Markdown, an Obsidian vault) shows its page,
a 思源笔记 workspace its 导入报告.

Build with --release --features devtools, then run this file. Each import
goes through 我的文档's "+" menu (思源笔记 and Obsidian through its
从其他应用导入… panel) and a `pick` instead of the system panel, into a
temporary library; `showing` checks what is in front. `found_vault` imports a
vault the panel found in a stand-in home folder (an Obsidian iCloud vault), so
the user's own is never looked at. No system clipboard or user library is
accessed. Pass case names to run only those.
"""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

here = Path(__file__).resolve().parent
repo = here.parent.parent
work = Path(tempfile.mkdtemp(prefix="quiet-import-open-"))
source = work / "source"
(source / "docs").mkdir(parents=True)
DOC = "aaaaaaaa-0000-0000-0000-000000000001"
(source / "docs" / f"{DOC}.xml").write_text("<title>Start</title>\n<p>start</p>\n")
(source / "index.json").write_text(json.dumps({DOC: {"created": 1, "modified": 1, "opened": 1}}))
(source / "preferences.json").write_text(json.dumps({
    "appearance": "light", "english": True, "language": "en", "seen_version": "9.9.9",
    "auto_update": False, "quick_hotkey": False,
}))

files = work / "files"
(files / "notes").mkdir(parents=True)
(files / "Single.md").write_text("# Single\n\nOne file.\n")
(files / "notes" / "a.md").write_text("# A\n\nIn a folder.\n")
(files / "notes" / "b.md").write_text("# B\n\nAlso.\n")
(files / "Vault" / ".obsidian").mkdir(parents=True)
(files / "Vault" / ".obsidian" / "app.json").write_text("{}")
(files / "Vault" / "Daily.md").write_text("# Daily\n\nSee [[Other]].\n")
(files / "Vault" / "Other.md").write_text("# Other\n\nlinked\n")
shutil.copytree(repo / "crates/core/tests/fixtures/siyuan", files / "siyuan")

# A stand-in home: what the panel finds on "this computer".
home = work / "home"
found = home / "Library/Mobile Documents/iCloud~md~obsidian/Documents/Thesis"
(found / ".obsidian").mkdir(parents=True)
(found / "Intro.md").write_text("# Intro\n\nFound.\n")

# The "+" menu under 我的文档 (English): its 导入 rows, then the panel's row if any.
APPS = "Import from Another App…"
CASES = {
    "markdown_file": (["Import Markdown Files…"], files / "Single.md", "Single"),
    "markdown_folder": (["Import Folder…"], files / "notes", "notes"),
    "obsidian_vault": (["Import Folder…"], files / "Vault", "Vault"),
    "obsidian_app": ([APPS, "Obsidian"], files / "Vault", "Vault"),
    "siyuan": ([APPS, "SiYuan"], files / "siyuan", "SiYuan Import Report"),
    "found_vault": ([APPS, "Obsidian · Thesis"], None, "Thesis"),
}

failed = []
for name in sys.argv[1:] or CASES:
    rows, path, title = CASES[name]
    script = work / f"{name}.txt"
    # The panel looks for other apps' notes as it opens; the pick waits for the row that opens the file panel.
    steps = [f"row {rows[0]}", "wait 1500"] + [f"pick {path}"] * (path is not None) + [f"row {r}" for r in rows[1:]]
    if len(rows) == 1: steps = [f"pick {path}", f"row {rows[0]}"]
    script.write_text("move 5 5\nwait 800\nshowing Start\nanchor create-sidebar\nwait 400\n" + "\n".join(steps) + f"\nwait 2500\nshot {name}\nshowing {title}\n")
    out = work / name
    r = subprocess.run([sys.executable, str(here / "run.py"), str(script), str(source), str(out)],
                       env=dict(os.environ, FDL_NOBLINK="1", HOME=str(home)), timeout=120)
    log = (out / "log").read_text(errors="replace") if (out / "log").exists() else ""
    said = [l for l in log.splitlines() if "showing" in l]
    if r.returncode == 0: print(f"{name}: passed", flush=True)
    else:
        failed.append(name)
        print(f"{name}: FAILED {said[-1] if said else ''}", flush=True)
print(f"import_open: {'passed' if not failed else 'FAILED ' + ', '.join(failed)}; artifacts: {work}")
sys.exit(1 if failed else 0)
