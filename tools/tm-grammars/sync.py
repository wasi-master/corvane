#!/usr/bin/env python3
"""TextMate grammars for the languages no other highlighter covers, compiled
into the default build (crates/corvane-highlight/assets/syntaxes.packdump).

    python3 tools/tm-grammars/sync.py            # convert into tools/tm-grammars/syntaxes/
    cargo run -p corvane-highlight --features pack-builder --example tm-build -- \\
        tools/tm-grammars/syntaxes target/tm/samples.json \\
        crates/corvane-highlight/assets/syntaxes.packdump tools/tm-grammars/rejected.txt

Source: GitHub Linguist at LINGUIST_TAG. Its release ships every grammar it
vendors (VS Code, Atom, Sublime Text and TextMate bundles; all permissive,
licenses in vendor/licenses) compiled to TextMate JSON, and languages.yml
names each language's scope, extensions, file names and interpreters.

A language is taken when none of its file types is covered already: a
tree-sitter grammar (tools/ts-queries/languages.toml), a ported CodeMirror
mode, syntect's defaults or two-face (the `list-syntaxes` example). Scopes a
taken grammar includes come along (hidden) when the set lacks them. Each
grammar is converted TextMate → sublime-syntax (the format syntect reads);
tm-build then rejects the ones that fail on Linguist's samples.
"""

from __future__ import annotations

import hashlib
import io
import json
import re
import subprocess
import sys
import tarfile
import tomllib
import urllib.request
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[2]
TOOLS = ROOT / "tools" / "tm-grammars"
OUT = TOOLS / "syntaxes"
WORK = ROOT / "target" / "tm"
LINGUIST_TAG = "v9.7.0"
GRAMMARS_URL = f"https://github.com/github-linguist/linguist/releases/download/{LINGUIST_TAG}/linguist-grammars.tar.gz"
GRAMMARS_SHA256 = "263da41b28f96d2d494d3ecd9480581d4425194befb68551722a6d8994d44eb6"
LINGUIST = WORK / "linguist"


def linguist_checkout() -> Path:
    if not LINGUIST.exists():
        subprocess.run(
            ["git", "clone", "-q", "--depth", "1", "--branch", LINGUIST_TAG,
             "https://github.com/github-linguist/linguist", str(LINGUIST)],
            check=True,
        )
    return LINGUIST


def grammars() -> dict[str, dict]:
    """scope → TextMate grammar (JSON) from the pinned release."""
    cache = WORK / f"linguist-grammars-{LINGUIST_TAG}.tar.gz"
    if not cache.exists():
        WORK.mkdir(parents=True, exist_ok=True)
        cache.write_bytes(urllib.request.urlopen(GRAMMARS_URL, timeout=120).read())
    data = cache.read_bytes()
    if hashlib.sha256(data).hexdigest() != GRAMMARS_SHA256:
        raise SystemExit("linguist-grammars.tar.gz does not match the pinned sha256")
    out = {}
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as tar:
        for m in tar.getmembers():
            if m.isfile() and m.name.endswith(".json"):
                g = json.load(tar.extractfile(m))
                if isinstance(g, dict) and g.get("scopeName"):
                    out[g["scopeName"]] = g
    return out


def licenses(root: Path) -> dict[str, tuple[str, str]]:
    """scope → (license, grammar repository) from Linguist's vendor records."""
    scopes = yaml.safe_load((root / "grammars.yml").read_text())
    out = {}
    for path, names in scopes.items():
        repo = path.removeprefix("vendor/grammars/")
        dep = root / "vendor" / "licenses" / "git_submodule" / f"{repo}.dep.yml"
        lic, home = "unknown", path
        if dep.exists():
            meta = yaml.safe_load(dep.read_text().split("licenses:")[0])
            lic, home = str(meta.get("license", "unknown")), meta.get("homepage") or path
        for scope in names or []:
            out[scope] = (lic, home)
    return out


# --- coverage ---------------------------------------------------------------


def covered_types() -> set[str]:
    """Extensions (`.x`) and lowercase file names some highlighter handles."""
    covered: set[str] = set()
    with open(ROOT / "tools" / "ts-queries" / "languages.toml", "rb") as f:
        for lang in tomllib.load(f).values():
            covered |= {"." + e for e in lang.get("extensions", [])}
            covered |= set(lang.get("filenames", []))
    modes = (ROOT / "crates" / "corvane-highlight" / "src" / "cm" / "modes" / "mod.rs").read_text()
    covered |= set(re.findall(r'"(\.[^"]+)"\s*(?:\||=>)', modes))
    covered |= set(re.findall(r'"([a-z0-9_.-]+)"\s*=>\s*"text/', modes))
    existing = WORK / "existing.json"
    if not existing.exists():
        existing.write_text(subprocess.run(
            ["cargo", "run", "-q", "-p", "corvane-highlight", "--features", "pack-builder", "--example", "list-syntaxes"],
            cwd=ROOT, check=True, capture_output=True, text=True,
        ).stdout)
    for s in json.loads(existing.read_text()):
        for e in s["extensions"]:
            covered.add(e.lower() if "." in e and not e.startswith(".") else "." + e.lower().lstrip("."))
            covered.add(e.lower())
    return covered


def existing_scopes() -> set[str]:
    return {s["scope"] for s in json.loads((WORK / "existing.json").read_text())}


# --- TextMate → sublime-syntax ----------------------------------------------

G_ANCHOR = re.compile(r"\\G")


def fix_regex(rx: str) -> str:
    """Oniguruma-isms fancy-regex rejects, where a safe rewrite exists."""
    rx = G_ANCHOR.sub("", rx)  # \G (match where the last one ended): usually redundant
    rx = rx.replace("\\h", "[0-9A-Fa-f]").replace("\\H", "[^0-9A-Fa-f]")
    return rx


def scope_name(name: str | None) -> str | None:
    if not name:
        return None
    # `$1`-style substitutions are TextMate-only
    parts = [p for p in name.split() if "$" not in p and "\\" not in p]
    return " ".join(parts) or None


def captures(caps: dict | None) -> dict | None:
    if not caps:
        return None
    out = {}
    for k, v in caps.items():
        if str(k).isdigit() and isinstance(v, dict) and (n := scope_name(v.get("name"))):
            out[int(k)] = n
    return out or None


class Converter:
    def __init__(self, grammar: dict):
        self.g = grammar
        self.contexts: dict[str, list] = {}

    def include(self, ref: str) -> dict | None:
        if ref.startswith("#"):
            name = "repo_" + ref[1:]
            return {"include": name} if ref[1:] in self.g.get("repository", {}) else None
        if ref in ("$self", "$base"):
            return {"include": "main"}
        scope = ref.split("#", 1)[0]
        if scope == self.g["scopeName"]:
            return {"include": "main"}
        return {"include": f"scope:{scope}"}

    def patterns(self, rules: list | None) -> list:
        out = []
        for rule in rules or []:
            out += self.rule(rule)
        return out

    def rule(self, rule: dict) -> list:
        if not isinstance(rule, dict) or rule.get("disabled"):
            return []
        if "include" in rule:
            inc = self.include(rule["include"])
            return [inc] if inc else []
        if "match" in rule:
            item: dict = {"match": fix_regex(rule["match"])}
            if n := scope_name(rule.get("name")):
                item["scope"] = n
            if c := captures(rule.get("captures")):
                item["captures"] = c
            return [item]
        if "begin" in rule and ("end" in rule or "while" in rule):
            begin: dict = {"match": fix_regex(rule["begin"])}
            if c := captures(rule.get("beginCaptures") or rule.get("captures")):
                begin["captures"] = c
            body: list = []
            if n := scope_name(rule.get("name")):
                body.append({"meta_scope": n})
            if n := scope_name(rule.get("contentName")):
                body.append({"meta_content_scope": n})
            if "end" in rule:
                end: dict = {"match": fix_regex(rule["end"]), "pop": True}
                if c := captures(rule.get("endCaptures") or rule.get("captures")):
                    end["captures"] = c
            else:
                # `while`: leave at the first line the pattern does not match
                end = {"match": f"^(?!{fix_regex(rule['while'])})", "pop": True}
            inner = self.patterns(rule.get("patterns"))
            body += inner + [end] if rule.get("applyEndPatternLast") else [end] + inner
            begin["push"] = body
            return [begin]
        if "patterns" in rule:
            return self.patterns(rule["patterns"])
        return []

    def convert(self, name: str, extensions: list[str], first_line: str | None, hidden: bool) -> dict:
        for key, rule in (self.g.get("repository") or {}).items():
            self.contexts["repo_" + key] = self.rule(rule) or []
        self.contexts = {"main": self.patterns(self.g.get("patterns")), **self.contexts}
        out: dict = {"name": name, "scope": self.g["scopeName"]}
        if extensions:
            out["file_extensions"] = extensions
        if first_line:
            out["first_line_match"] = first_line
        if hidden:
            out["hidden"] = True
        out["contexts"] = self.contexts
        return out


def includes(grammar: dict) -> set[str]:
    found: set[str] = set()

    def walk(x):
        if isinstance(x, dict):
            if isinstance(x.get("include"), str):
                ref = x["include"]
                if not ref.startswith(("#", "$")):
                    found.add(ref.split("#", 1)[0])
            for v in x.values():
                walk(v)
        elif isinstance(x, list):
            for v in x:
                walk(v)

    walk(grammar)
    return found


def main(argv: list[str]) -> int:
    root = linguist_checkout()
    all_grammars = grammars()
    lics = licenses(root)
    languages = yaml.safe_load((root / "lib" / "linguist" / "languages.yml").read_text())
    covered = covered_types()
    have = existing_scopes()
    chosen: dict[str, dict] = {}  # scope → {language, extensions, first_line}
    for lang, info in languages.items():
        scope = info.get("tm_scope")
        if not scope or scope == "none" or scope not in all_grammars or scope in have:
            continue
        exts = [e.lower() for e in info.get("extensions", [])]
        files = [f.lower() for f in info.get("filenames", [])]
        types = exts + files
        if not types or any(t in covered for t in types):
            continue
        lic = lics.get(scope, ("unknown", ""))[0]
        if lic in ("other", "unknown"):
            continue
        entry = chosen.setdefault(scope, {"language": lang, "extensions": [], "interpreters": []})
        # sublime-syntax file_extensions: extensions without the dot, or file names
        entry["extensions"] += [e.lstrip(".") for e in exts] + [f for f in info.get("filenames", [])]
        entry["interpreters"] += info.get("interpreters", [])
    # included scopes the set would lack
    queue = list(chosen)
    deps: set[str] = set()
    while queue:
        for inc in includes(all_grammars[queue.pop()]):
            if inc in all_grammars and inc not in have and inc not in chosen and inc not in deps:
                if lics.get(inc, ("unknown",))[0] in ("other", "unknown"):
                    continue
                deps.add(inc)
                queue.append(inc)
    OUT.mkdir(parents=True, exist_ok=True)
    for old in OUT.glob("*.sublime-syntax"):
        old.unlink()
    samples: dict[str, list[str]] = {}
    notices = ["# TextMate grammars in the default build", "",
               f"Converted from GitHub Linguist {LINGUIST_TAG}'s compiled grammars by `tools/tm-grammars/sync.py`.", "",
               "| Language | Scope | License | Source |", "|---|---|---|---|"]
    for scope in sorted(set(chosen) | deps):
        g = all_grammars[scope]
        info = chosen.get(scope)
        name = info["language"] if info else g.get("name", scope)
        interp = info["interpreters"] if info else []
        first = rf"^#!.*\b({'|'.join(re.escape(i) for i in interp)})\b" if interp else None
        exts = list(dict.fromkeys(info["extensions"])) if info else []
        doc = Converter(g).convert(name, exts, first, hidden=info is None)
        lic, home = lics.get(scope, ("unknown", ""))
        text = (f"%YAML 1.2\n---\n# {name} ({scope}): converted from TextMate by tools/tm-grammars/sync.py;\n"
                f"# source {home} via GitHub Linguist {LINGUIST_TAG} (license: {lic}).\n"
                + yaml.safe_dump(doc, sort_keys=False, allow_unicode=False, width=1000))
        (OUT / f"{scope}.sublime-syntax").write_text(text)
        notices.append(f"| {name} | `{scope}` | {lic} | {home} |")
        if info:
            dirs = [root / "samples" / info["language"]]
            samples[scope] = [str(p) for d in dirs if d.exists() for p in sorted(d.rglob("*")) if p.is_file()][:6]
    (WORK / "samples.json").write_text(json.dumps(samples, indent=1))
    (TOOLS / "THIRD_PARTY.md").write_text("\n".join(notices) + "\n")
    print(f"{len(chosen)} languages, {len(deps)} included scopes")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
