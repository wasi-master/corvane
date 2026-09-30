#!/usr/bin/env python3
"""Vendor tree-sitter highlight queries into crates/corvane-grammars/queries.

    python3 tools/ts-queries/sync.py                 # every grammar in languages.toml
    python3 tools/ts-queries/sync.py rust haskell    # some
    python3 tools/ts-queries/sync.py --candidates    # every source of every grammar
                                                     # into target/ts-queries/candidates
    python3 tools/ts-queries/sync.py --check         # fail if a vendored file is stale

Sources, in the order the plan prefers them (languages.toml picks one per
grammar, see README.md):

- upstream: the grammar package's own queries (the locked crate version,
  found with `cargo metadata`; tree-sitter.json says which files);
- nvim: nvim-treesitter at NVIM_REV (Apache-2.0);
- helix: Helix at HELIX_REV (MPL-2.0);
- corvane: hand-written, kept as is (tools/ts-queries/corvane/<name>/).

nvim and Helix queries are rewritten for tree-sitter-highlight: `; inherits:`
is resolved, `#lua-match?` / `#vim-match?` become `#match?`, predicates and
directives it cannot evaluate are dropped (noted in the file header), and
Helix files are reversed pattern by pattern (Helix lets the first matching pattern win,
tree-sitter-highlight the last). Files in tools/ts-queries/patches/<name>/
are appended (Corvane additions, MIT).
"""

from __future__ import annotations

import json
import re
import subprocess
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
TOOLS = ROOT / "tools" / "ts-queries"
OUT = ROOT / "crates" / "corvane-grammars" / "queries"
CACHE = ROOT / "target" / "ts-queries"
KINDS = ("highlights", "injections", "locals")

# Pinned query sources (bump deliberately, then rerun and review the diff).
NVIM_REPO = "https://github.com/nvim-treesitter/nvim-treesitter"
NVIM_REV = "main"
HELIX_REPO = "https://github.com/helix-editor/helix"
HELIX_REV = "master"

LICENSES = {"nvim": "Apache-2.0", "helix": "MPL-2.0", "corvane": "MIT"}


def load_languages() -> dict:
    with open(TOOLS / "languages.toml", "rb") as f:
        return tomllib.load(f)


# --- upstream ---------------------------------------------------------------

_packages: dict[str, dict] | None = None


def packages() -> dict[str, dict]:
    """name -> {version, dir, license} for every package in the grammar crate's graph."""
    global _packages
    if _packages is None:
        meta = json.loads(
            subprocess.run(
                ["cargo", "metadata", "--format-version", "1", "--features", "corvane-grammars/all"],
                cwd=ROOT,
                check=True,
                capture_output=True,
                text=True,
            ).stdout
        )
        _packages = {}
        for pkg in meta["packages"]:
            _packages[pkg["name"]] = {
                "version": pkg["version"],
                "dir": Path(pkg["manifest_path"]).parent,
                "license": pkg.get("license") or "see the package",
                "repository": pkg.get("repository") or "",
            }
    return _packages


def upstream_files(name: str, lang: dict) -> tuple[dict[str, list[Path]], str] | None:
    pkg = packages().get(lang["package"])
    if pkg is None:
        return None
    root = pkg["dir"]
    config = {}
    ts_json = root / "tree-sitter.json"
    if ts_json.exists():
        try:
            grammars = json.loads(ts_json.read_text())["grammars"]
            wanted = lang.get("grammar", name)
            config = next((g for g in grammars if g.get("name") == wanted), grammars[0] if len(grammars) == 1 else {})
        except (ValueError, KeyError):
            config = {}
    files: dict[str, list[Path]] = {}
    for kind in KINDS:
        listed = lang.get(kind, config.get(kind))
        if listed is None:
            default = root / "queries" / f"{kind}.scm"
            listed = [str(default.relative_to(root))] if default.exists() else []
        if isinstance(listed, str):
            listed = [listed]
        files[kind] = [root / p for p in listed if (root / p).exists()]
    label = f"{lang['package']} {pkg['version']}"
    return files, f"{label} ({pkg['license']})"


# --- nvim-treesitter / Helix ------------------------------------------------


def checkout(repo: str, rev: str, dest: Path) -> Path:
    if not dest.exists():
        dest.parent.mkdir(parents=True, exist_ok=True)
        subprocess.run(["git", "init", "-q", str(dest)], check=True)
        subprocess.run(["git", "-C", str(dest), "remote", "add", "origin", repo], check=True)
    subprocess.run(["git", "-C", str(dest), "fetch", "-q", "--depth", "1", "origin", rev], check=True)
    subprocess.run(["git", "-C", str(dest), "checkout", "-q", "FETCH_HEAD"], check=True)
    return dest


def head(dest: Path) -> str:
    return subprocess.run(
        ["git", "-C", str(dest), "rev-parse", "--short=12", "HEAD"], check=True, capture_output=True, text=True
    ).stdout.strip()


_trees: dict[str, Path] = {}


def tree(source: str) -> Path:
    if source not in _trees:
        if source == "nvim":
            _trees[source] = checkout(NVIM_REPO, NVIM_REV, CACHE / "nvim-treesitter")
        else:
            _trees[source] = checkout(HELIX_REPO, HELIX_REV, CACHE / "helix")
    return _trees[source]


def query_dir(source: str) -> Path:
    base = tree(source)
    if source == "nvim":
        for sub in ("runtime/queries", "queries"):
            if (base / sub).is_dir():
                return base / sub
    return base / "runtime" / "queries"


INHERITS = re.compile(r"^;+\s*inherits\s*:?\s*(.+)$", re.M)


def read_with_inherits(source: str, lang: str, kind: str, seen: set[str] | None = None) -> str | None:
    path = query_dir(source) / lang / f"{kind}.scm"
    if not path.exists():
        return None
    seen = seen or set()
    seen.add(lang)
    text = path.read_text()
    parts = []
    for m in INHERITS.finditer(text):
        for parent in re.split(r"[,\s]+", m.group(1).strip()):
            parent = parent.strip("()")
            if parent and parent not in seen:
                inherited = read_with_inherits(source, parent, kind, seen)
                if inherited:
                    parts.append(f"; inherited from {parent}\n{inherited}")
    parts.append(INHERITS.sub("", text))
    return "\n".join(parts)


# --- rewriting --------------------------------------------------------------

LUA_CLASSES = {
    "a": "[A-Za-z]", "d": "[0-9]", "l": "[a-z]", "u": "[A-Z]", "s": r"\s",
    "w": "[A-Za-z0-9]", "x": "[0-9A-Fa-f]", "p": r"[!-/:-@\[-`{-~]", "c": r"[\x00-\x1f]",
    "A": "[^A-Za-z]", "D": "[^0-9]", "L": "[^a-z]", "U": "[^A-Z]", "S": r"\S", "W": "[^A-Za-z0-9]",
}


def unquote(text: str) -> str:
    """A query string literal's content (between the quotes) as its value."""
    out, i = [], 0
    while i < len(text):
        if text[i] == "\\" and i + 1 < len(text):
            nxt = text[i + 1]
            out.append({"n": "\n", "t": "\t", "r": "\r", "0": "\0"}.get(nxt, nxt))
            i += 2
        else:
            out.append(text[i])
            i += 1
    return "".join(out)


def quote(value: str) -> str:
    """A value as query string literal content."""
    return value.replace("\\", "\\\\").replace('"', '\\"').replace("\n", "\\n").replace("\t", "\\t")


def lua_to_regex(pattern: str) -> str:
    """A Lua pattern as a Rust regex (the subset queries use)."""
    out = []
    i = 0
    in_class = False
    while i < len(pattern):
        c = pattern[i]
        if c == "%" and i + 1 < len(pattern):
            n = pattern[i + 1]
            cls = LUA_CLASSES.get(n)
            if cls:
                out.append(cls[1:-1] if in_class and cls.startswith("[") and not cls.startswith("[^") else cls)
            else:
                out.append(re.escape(n))
            i += 2
            continue
        if c == "[":
            in_class = True
        elif c == "]":
            in_class = False
        elif c == "-" and not in_class:
            out.append("*?")
            i += 1
            continue
        elif c in "{}|\\" and not in_class:
            out.append("\\" + c)
            i += 1
            continue
        elif c == "\\" and in_class:
            out.append("\\\\")
            i += 1
            continue
        out.append(c)
        i += 1
    return "".join(out)


def vim_to_regex(pattern: str) -> str:
    """A (magic) Vim regex as a Rust regex, for the forms queries use."""
    flags = ""
    if "\\c" in pattern:
        flags = "(?i)"
        pattern = pattern.replace("\\c", "")
    pattern = pattern.replace("\\C", "").replace("\\v", "")
    for vim, rust in (("\\(", "("), ("\\)", ")"), ("\\|", "|"), ("\\+", "+"), ("\\=", "?"), ("\\<", "\\b"), ("\\>", "\\b")):
        pattern = pattern.replace(vim, rust)
    return flags + pattern


def split_patterns(text: str) -> list[str]:
    """Top-level query patterns in order, each with the comments above it and
    the captures / quantifiers after it."""
    patterns: list[str] = []
    buf: list[str] = []
    depth = 0
    has_body = False
    i, n = 0, len(text)

    def flush() -> None:
        nonlocal has_body
        if buf:
            chunk = "".join(buf)
            patterns.append(chunk if chunk.endswith("\n") else chunk + "\n")
            buf.clear()
        has_body = False

    while i < n:
        c = text[i]
        if c == ";":
            j = text.find("\n", i)
            j = n if j < 0 else j + 1
            if depth == 0 and has_body:
                flush()
            buf.append(text[i:j])
            i = j
            continue
        if c == '"':
            j = i + 1
            while j < n and text[j] != '"':
                j += 2 if text[j] == "\\" else 1
            if depth == 0 and has_body:
                flush()
            buf.append(text[i : j + 1])
            i = j + 1
            if depth == 0:
                has_body = True
            continue
        if c in "([":
            if depth == 0 and has_body:
                flush()
            depth += 1
        elif c in ")]":
            depth -= 1
            if depth == 0:
                has_body = True
        buf.append(c)
        i += 1
    flush()
    return patterns


UNSUPPORTED_PREDICATES = re.compile(
    r"\(#(?!eq\?|not-eq\?|any-eq\?|any-not-eq\?|match\?|not-match\?|any-match\?|any-not-match\?|any-of\?|not-any-of\?|set!|is\?|is-not\?)[\w-]+[?!][^()]*\)"
)


def rewrite(text: str, source: str, dropped: list[str]) -> str:
    def lua_match(m: re.Match) -> str:
        neg = m.group(1) or ""
        regex = lua_to_regex(unquote(m.group(3)))
        return f'(#{neg}match? {m.group(2)} "{quote(regex)}")'

    text = re.sub(r'\(#(not-)?lua-match\?\s+(@[\w.]+)\s+"((?:[^"\\]|\\.)*)"\s*\)', lua_match, text)

    def vim_match(m: re.Match) -> str:
        neg = m.group(1) or ""
        regex = vim_to_regex(unquote(m.group(3)))
        return f'(#{neg}match? {m.group(2)} "{quote(regex)}")'

    text = re.sub(r'\(#(not-)?vim-match\?\s+(@[\w.]+)\s+"((?:[^"\\]|\\.)*)"\s*\)', vim_match, text)
    if source == "nvim":
        # Neovim's `#match?` is a Vim regex too
        text = re.sub(r'\(#(not-)?match\?\s+(@[\w.]+)\s+"((?:[^"\\]|\\.)*)"\s*\)', vim_match, text)
    # nvim's `(#set! @node key @other)` stores a node as metadata; tree-sitter
    # only takes a string value
    text = re.sub(r"\(#set!\s+@[\w.]+\s+[\w.-]+\s+@[\w.]+\s*\)", "", text)

    def drop(m: re.Match) -> str:
        dropped.append(m.group(0).split()[0].lstrip("("))
        return ""

    text = UNSUPPORTED_PREDICATES.sub(drop, text)
    if source == "helix":
        text = "".join(reversed(split_patterns(text)))
    return text


def drop_patterns(text: str, names: list[str]) -> tuple[str, int]:
    """Remove the top-level patterns that use any of `names` as a node type,
    an anonymous node or a field (not in the pinned grammar version)."""
    if not names:
        return text, 0
    alternatives = "|".join(re.escape(n) for n in names)
    uses = re.compile(rf'\((?:{alternatives})(?=[\s)])|"(?:{alternatives})"|(?<![\w.-])(?:{alternatives}):')
    kept, dropped = [], 0
    for pattern in split_patterns(text):
        code = "\n".join(line.split(";", 1)[0] for line in pattern.splitlines())
        if uses.search(code):
            dropped += 1
        else:
            kept.append(pattern)
    return "".join(kept), dropped


def other_files(source: str, name: str, lang: dict) -> tuple[dict[str, str], str] | None:
    remote = lang.get(source, name)
    texts = {kind: read_with_inherits(source, remote, kind) for kind in KINDS}
    if not texts["highlights"]:
        return None
    base = "nvim-treesitter" if source == "nvim" else "helix"
    return {k: v or "" for k, v in texts.items()}, f"{base}@{head(tree(source))} queries/{remote} ({LICENSES[source]})"


# --- output -----------------------------------------------------------------


def render(name: str, lang: dict, source: str) -> dict[str, str] | None:
    """kind -> file text for one grammar from one source, minus the patterns
    `drop` names (only for the configured source)."""
    files = render_source(name, lang, source)
    drop = lang.get("drop", []) if source == lang.get("source") else []
    if files is None or not drop:
        return files
    for kind, text in files.items():
        head, _, body = text.partition("\n; Source: ")
        source_line, _, body = body.partition("\n")
        body, dropped = drop_patterns(body, drop)
        note = f"; dropped {dropped} pattern(s) using {', '.join(drop)} (not in the pinned grammar)\n" if dropped else ""
        files[kind] = f"{head}\n; Source: {source_line}\n{note}{body}"
    return files


def render_source(name: str, lang: dict, source: str) -> dict[str, str] | None:
    out = {}
    if source == "upstream":
        found = upstream_files(name, lang)
        if not found:
            return None
        files, label = found
        if not files["highlights"]:
            return None
        for kind in KINDS:
            body = "".join(p.read_text() for p in files[kind])
            rels = ", ".join(str(p.relative_to(packages()[lang["package"]]["dir"])) for p in files[kind])
            out[kind] = header(name, f"{label}: {rels}" if rels else label) + body if body else header(name, label)
        return out
    if source == "corvane":
        base = TOOLS / "corvane" / name
        if not (base / "highlights.scm").exists():
            return None
        for kind in KINDS:
            path = base / f"{kind}.scm"
            body = path.read_text() if path.exists() else ""
            out[kind] = header(name, "Corvane (MIT)") + body
        return out
    found = other_files(source, name, lang)
    if not found:
        return None
    texts, label = found
    for kind in KINDS:
        dropped: list[str] = []
        body = rewrite(texts[kind], source, dropped)
        note = f"; dropped predicates tree-sitter-highlight cannot evaluate: {', '.join(sorted(set(dropped)))}\n" if dropped else ""
        out[kind] = header(name, label) + note + body
    return out


def header(name: str, label: str) -> str:
    return (
        f"; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/{name}/).\n"
        f"; Source: {label}\n"
    )


def with_patches(name: str, files: dict[str, str]) -> dict[str, str]:
    for kind in KINDS:
        patch = TOOLS / "patches" / name / f"{kind}.scm"
        if patch.exists():
            files[kind] += f"\n; Corvane additions (tools/ts-queries/patches/{name}/{kind}.scm, MIT)\n" + patch.read_text()
    return files


def third_party(languages: dict) -> str:
    """crates/corvane-grammars/THIRD_PARTY.md: every grammar's package and
    query source with their licenses (shipped inside the grammar packs)."""
    lines = [
        "# Third-party grammars and queries",
        "",
        "Generated by `tools/ts-queries/sync.py`. The tree-sitter packs and the full build contain",
        "these grammars (compiled C parsers) and highlight queries.",
        "",
        "| Grammar | Parser | License | Queries |",
        "|---|---|---|---|",
    ]
    for name in sorted(languages):
        lang = languages[name]
        pkg = packages().get(lang["package"], {})
        repo = pkg.get("repository", "")
        parser = f"[{lang['package']} {pkg.get('version', '?')}]({repo})" if repo else f"{lang['package']} {pkg.get('version', '?')}"
        source = ""
        path = OUT / name / "highlights.scm"
        if path.exists():
            for line in path.read_text().splitlines()[:3]:
                if line.startswith("; Source: "):
                    source = line[len("; Source: ") :]
        lines.append(f"| {name} | {parser} | {pkg.get('license', '?')} | {source} |")
    lines += [
        "",
        "nvim-treesitter queries: Apache-2.0, https://github.com/nvim-treesitter/nvim-treesitter.",
        "Helix queries: MPL-2.0, https://github.com/helix-editor/helix (source of the modified files:",
        "`crates/corvane-grammars/queries/` in https://github.com/wasi-master/corvane).",
        "",
    ]
    return "\n".join(lines)


def main(argv: list[str]) -> int:
    languages = load_languages()
    check = "--check" in argv
    candidates = "--candidates" in argv
    names = [a for a in argv if not a.startswith("--")] or sorted(languages)
    stale = []
    for name in names:
        lang = languages.get(name)
        if lang is None:
            print(f"{name}: not in languages.toml", file=sys.stderr)
            return 1
        if candidates:
            for source in ("upstream", "nvim", "helix", "corvane"):
                files = render(name, lang, source)
                if not files:
                    continue
                dest = CACHE / "candidates" / source / name
                dest.mkdir(parents=True, exist_ok=True)
                for kind, text in with_patches(name, files).items():
                    (dest / f"{kind}.scm").write_text(text)
            continue
        source = lang["source"]
        files = render(name, lang, source)
        if files is None and "--fallback" in argv:
            for source in ("nvim", "upstream", "helix", "corvane"):
                files = render(name, lang, source)
                if files is not None:
                    break
        if files is None:
            print(f"{name}: no {lang['source']} queries", file=sys.stderr)
            return 1
        files = with_patches(name, files)
        dest = OUT / name
        for kind, text in files.items():
            path = dest / f"{kind}.scm"
            if check:
                if not path.exists() or path.read_text() != text:
                    stale.append(str(path.relative_to(ROOT)))
                continue
            dest.mkdir(parents=True, exist_ok=True)
            path.write_text(text)
        if not check:
            print(f"{name}: {source}")
    if not candidates:
        doc = third_party(languages)
        path = ROOT / "crates" / "corvane-grammars" / "THIRD_PARTY.md"
        if check:
            if not path.exists() or path.read_text() != doc:
                stale.append(str(path.relative_to(ROOT)))
        else:
            path.write_text(doc)
    if stale:
        print("stale (run tools/ts-queries/sync.py):\n  " + "\n  ".join(stale), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
