#!/usr/bin/env python3
"""Fetch the sources of grammars built from source (languages.toml entries
with `url` + `revision`) into target/grammar-src/<name>/.

    python3 tools/ts-queries/fetch.py            # every source grammar
    python3 tools/ts-queries/fetch.py astro vue  # some
    python3 tools/ts-queries/fetch.py --update   # (re)pin sha256 / symbol in sources.lock.json

These grammars have no usable crate (none published, or one whose Rust
bindings pin an older tree-sitter runtime), so corvane-grammars' build.rs
compiles their `parser.c` (+ scanner) itself. Only `src/`, `queries/` and
the metadata files are extracted. `sources.lock.json` pins each tarball's
sha256 and records the grammar's C entry point; a mismatch is an error.
`CORVANE_GRAMMAR_SOURCES` overrides the destination (build.rs reads it too).
"""

from __future__ import annotations

import hashlib
import io
import json
import os
import re
import sys
import tarfile
import tomllib
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
TOOLS = ROOT / "tools" / "ts-queries"
LOCK = TOOLS / "sources.lock.json"
DEST = Path(os.environ.get("CORVANE_GRAMMAR_SOURCES", ROOT / "target" / "grammar-src"))


def load() -> dict:
    with open(TOOLS / "languages.toml", "rb") as f:
        return {k: v for k, v in tomllib.load(f).items() if "url" in v}


def lock() -> dict:
    return json.loads(LOCK.read_text()) if LOCK.exists() else {}


def tarball_url(url: str, revision: str) -> str:
    repo = url.removeprefix("https://github.com/").removesuffix(".git").strip("/")
    return f"https://codeload.github.com/{repo}/tar.gz/{revision}"


TREE_SITTER = os.environ.get("CORVANE_TREE_SITTER", "tree-sitter")
# the CLI that generates parser.c for `generate = true` grammars (matches the runtime)
TREE_SITTER_VERSION = "0.27.0"


def wanted(member: str, location: str, everything: bool = False) -> str | None:
    """The member's path inside the grammar folder, if it is extracted."""
    parts = member.split("/", 1)
    if len(parts) < 2:
        return None
    rest = parts[1]
    if location:
        if not rest.startswith(location + "/"):
            return rest if rest in ("LICENSE", "LICENSE.md", "LICENSE.txt", "COPYING") else None
        rest = rest[len(location) + 1 :]
    if everything:
        return rest
    if rest.startswith(("src/", "queries/")) or rest in (
        "tree-sitter.json", "package.json", "grammar.js", "LICENSE", "LICENSE.md", "LICENSE.txt", "COPYING",
    ):
        return rest
    return None


def fetch(name: str, lang: dict, pins: dict, update: bool) -> dict:
    url = tarball_url(lang["url"], lang["revision"])
    cache = DEST / "_downloads" / f"{name}-{lang['revision'][:12]}.tar.gz"
    cache.parent.mkdir(parents=True, exist_ok=True)
    if not cache.exists():
        req = urllib.request.Request(url, headers={"User-Agent": "corvane-grammars"})
        data = urllib.request.urlopen(req, timeout=120).read()
        cache.write_bytes(data)
    data = cache.read_bytes()
    sha = hashlib.sha256(data).hexdigest()
    pin = pins.get(name, {})
    if pin.get("revision") == lang["revision"] and pin.get("sha256") and pin["sha256"] != sha and not update:
        raise SystemExit(f"{name}: sha256 {sha} does not match the pin {pin['sha256']}")
    out = DEST / name
    if out.exists():
        for p in sorted(out.rglob("*"), reverse=True):
            p.unlink() if p.is_file() or p.is_symlink() else p.rmdir()
    out.mkdir(parents=True, exist_ok=True)
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as tar:
        for member in tar.getmembers():
            rel = wanted(member.name, lang.get("location", ""), everything=bool(lang.get("generate")))
            if rel is None or not member.isfile():
                continue
            target = out / rel
            target.parent.mkdir(parents=True, exist_ok=True)
            src = tar.extractfile(member)
            if src:
                target.write_bytes(src.read())
    parser = out / "src" / "parser.c"
    if lang.get("generate"):
        generate(name, out)
    if not parser.exists():
        raise SystemExit(f"{name}: no src/parser.c at {lang['revision']} (needs `tree-sitter generate`)")
    exports = re.findall(r"tree_sitter_(\w+)\s*\(\s*(?:void)?\s*\)\s*\{", parser.read_text(errors="replace"))
    if not exports:
        raise SystemExit(f"{name}: no tree_sitter_<name>() in parser.c")
    scanner = next((f.name for f in (out / "src").iterdir() if f.name in ("scanner.c", "scanner.cc")), "")
    pin = {"revision": lang["revision"], "sha256": sha, "symbol": f"tree_sitter_{exports[-1]}", "scanner": scanner}
    if lang.get("generate"):
        pin["generated_by"] = f"tree-sitter {TREE_SITTER_VERSION}"
    return pin


def generate(name: str, out: Path) -> None:
    """parser.c for a grammar that does not commit it (`tree-sitter generate`,
    from grammar.js through node, else from src/grammar.json)."""
    import subprocess

    version = subprocess.run([TREE_SITTER, "--version"], capture_output=True, text=True, check=True).stdout.split()[-1]
    if version != TREE_SITTER_VERSION:
        raise SystemExit(f"{name}: needs tree-sitter {TREE_SITTER_VERSION} to generate, found {version} (CORVANE_TREE_SITTER)")
    args = [TREE_SITTER, "generate"]
    if not (out / "grammar.js").exists():
        args.append("src/grammar.json")
    result = subprocess.run(args, cwd=out, capture_output=True, text=True)
    if result.returncode != 0:
        raise SystemExit(f"{name}: tree-sitter generate failed: {result.stderr.strip()[-400:]}")


def main(argv: list[str]) -> int:
    languages = load()
    update = "--update" in argv
    names = [a for a in argv if not a.startswith("--")] or sorted(languages)
    pins = lock()
    failed = []
    for name in names:
        try:
            pins[name] = fetch(name, languages[name], pins, update)
            print(f"{name}: {pins[name]['symbol']}")
        except (SystemExit, OSError, urllib.error.URLError) as err:
            print(f"{name}: {err}", file=sys.stderr)
            failed.append(name)
    for name in list(pins):
        if name not in languages:
            del pins[name]
    LOCK.write_text(json.dumps(dict(sorted(pins.items())), indent=1) + "\n")
    if failed:
        print("failed: " + " ".join(failed), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
