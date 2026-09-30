#!/usr/bin/env python3
"""Add grammars nvim-treesitter knows and languages.toml does not, built
from source at nvim-treesitter's pinned revision (so its queries match).

    python3 tools/ts-queries/import_nvim.py [--dry-run] [names…]

Needs target/ts-queries/nvim-treesitter (sync.py --candidates fetches it)
and GitHub's `gh` CLI (licenses). Skips grammars that need
`tree-sitter generate` (no committed parser.c) and non-permissive licenses.
File types come from the grammar's tree-sitter.json, else from GitHub
Linguist's languages.yml (MIT). Appends to languages.toml; review the
result, then run fetch.py, sync.py, gen.py.
"""

from __future__ import annotations

import json
import re
import subprocess
import sys
import tomllib
import urllib.request
from pathlib import Path

import yaml

sys.path.insert(0, str(Path(__file__).parent))
import fetch  # noqa: E402

ROOT = Path(__file__).resolve().parents[2]
TOOLS = ROOT / "tools" / "ts-queries"
NVIM = ROOT / "target" / "ts-queries" / "nvim-treesitter"
LINGUIST_URL = "https://raw.githubusercontent.com/github-linguist/linguist/main/lib/linguist/languages.yml"
LINGUIST = ROOT / "target" / "ts-queries" / "linguist-languages.yml"
PERMISSIVE = {"MIT", "Apache-2.0", "BSD-2-Clause", "BSD-3-Clause", "ISC", "Unlicense", "CC0-1.0", "0BSD", "Zlib", "WTFPL", "MPL-2.0"}
# helpers nvim-treesitter's queries inherit from; not grammars
QUERY_ONLY = {"ecma", "html_tags", "jsx"}


def parsers() -> dict:
    text = (NVIM / "lua" / "nvim-treesitter" / "parsers.lua").read_text()
    out = {}
    for m in re.finditer(r"^  ([a-z0-9_]+) = \{\n(.*?)^  \},", text, re.M | re.S):
        body = m.group(2)
        d = {k: mm.group(1) for k in ("revision", "url", "location") if (mm := re.search(rf"{k} = '([^']*)'", body))}
        d["generate"] = "generate = true" in body
        out[m.group(1)] = d
    return out


def license_of(url: str, revision: str, location: str) -> str:
    repo = url.removeprefix("https://github.com/").strip("/")
    spdx = subprocess.run(["gh", "api", f"repos/{repo}", "--jq", '.license.spdx_id // ""'], capture_output=True, text=True).stdout.strip()
    if spdx and spdx != "NOASSERTION":
        return spdx
    for path in ([f"{location}/package.json"] if location else []) + ["package.json", "Cargo.toml"]:
        try:
            text = urllib.request.urlopen(f"https://raw.githubusercontent.com/{repo}/{revision}/{path}", timeout=20).read().decode()
        except Exception:  # noqa: BLE001
            continue
        if m := re.search(r'"license"\s*:\s*"([^"]+)"|^license\s*=\s*"([^"]+)"', text, re.M):
            return m.group(1) or m.group(2)
    return "unknown"


def linguist() -> dict:
    if not LINGUIST.exists():
        LINGUIST.write_bytes(urllib.request.urlopen(LINGUIST_URL, timeout=60).read())
    data = yaml.safe_load(LINGUIST.read_text())
    index = {}
    for lang, info in data.items():
        for key in [lang, *info.get("aliases", [])]:
            index.setdefault(key.lower().replace(" ", "_").replace("-", "_"), info)
    return index


def file_types(name: str, ling: dict) -> tuple[list[str], list[str], str]:
    """(extensions, filenames, first_line) from tree-sitter.json, else Linguist."""
    exts, files = [], []
    ts = fetch.DEST / name / "tree-sitter.json"
    if ts.exists():
        try:
            grammars = json.loads(ts.read_text()).get("grammars") or []
            for g in grammars:
                if g.get("name") in (name, name.replace("_", "-")) or len(grammars) == 1:
                    for t in g.get("file-types") or []:
                        is_file = t[:1].isupper() or (t.startswith(".") and "." not in t[1:])
                        (files if is_file else exts).append(t.lower().lstrip("."))
        except (ValueError, KeyError, TypeError, AttributeError):
            pass
    info = ling.get(name) or ling.get(name.replace("_", ""))
    if info:
        exts += [e.lower().lstrip(".") for e in info.get("extensions", [])]
        files += [f.lower() for f in info.get("filenames", [])]
    interpreters = info.get("interpreters", []) if info else []
    first = rf"^#!.*\b({'|'.join(re.escape(i) for i in interpreters)})\b" if interpreters else ""
    dedupe = lambda xs: list(dict.fromkeys(x for x in xs if x))  # noqa: E731
    return dedupe(exts), dedupe(files), first


def toml_list(xs: list[str]) -> str:
    return "[" + ", ".join(json.dumps(x) for x in xs) + "]"


def main(argv: list[str]) -> int:
    dry = "--dry-run" in argv
    only = [a for a in argv if not a.startswith("--")]
    with open(TOOLS / "languages.toml", "rb") as f:
        have = set(tomllib.load(f))
    ling = linguist()
    blocks, skipped = [], []
    for name, p in sorted(parsers().items()):
        if name in have or name in QUERY_ONLY or (only and name not in only):
            continue
        if not p.get("url", "").startswith("https://github.com/"):
            skipped.append(f"{name}: not on GitHub")
            continue
        if p["generate"]:
            skipped.append(f"{name}: needs tree-sitter generate")
            continue
        lic = license_of(p["url"], p["revision"], p.get("location", ""))
        if lic.split(" ")[0] not in PERMISSIVE:
            skipped.append(f"{name}: license {lic}")
            continue
        lang = {"url": p["url"], "revision": p["revision"], "location": p.get("location", "")}
        try:
            fetch.fetch(name, lang, fetch.lock(), update=True)
        except (SystemExit, OSError) as err:
            skipped.append(f"{name}: {err}")
            continue
        try:
            exts, files, first = file_types(name, ling)
        except Exception as err:  # noqa: BLE001
            skipped.append(f"{name}: file types: {err}")
            continue
        lines = [f"[{name}]", f'url = "{p["url"]}"', f'revision = "{p["revision"]}"']
        if p.get("location"):
            lines.append(f'location = "{p["location"]}"')
        lines += [f'license = "{lic}"', f"extensions = {toml_list(exts)}"]
        if files:
            lines.append(f"filenames = {toml_list(files)}")
        if first:
            lines.append(f"first_line = {json.dumps(first)}")
        lines += ['source = "nvim"', ""]
        blocks.append("\n".join(lines))
        print(f"{name}: {lic} {exts[:6]} {files[:3]}")
    for s in skipped:
        print("skip", s, file=sys.stderr)
    if not dry and blocks:
        with open(TOOLS / "languages.toml", "a") as f:
            f.write("\n# Built from source at nvim-treesitter's pinned revision (import_nvim.py)\n\n" + "\n".join(blocks))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
