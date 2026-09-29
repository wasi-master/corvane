#!/usr/bin/env python3
"""Regenerate golden token files for crates/corvane-highlight/tests/cm.

Every `samples/<name>.<ext>` gets `expected/<name>.<ext>.tokens` with one
`line start length style` row per token, as GitHub Desktop's highlighter
worker produces it (via cmtok.js). The MIME comes from GHD's own tables
(the sample's extension, or its file name for basename-mapped files).

    python3 tools/cm-oracle/extract.py      # once per GHD version
    python3 tools/cm-oracle/gen.py [sample…]
"""

import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
TESTS = ROOT / "crates" / "corvane-highlight" / "tests" / "cm"
ORACLE = ROOT / "target" / "cm-oracle"


def tables():
    src = (ORACLE / "src" / "index.ts").read_text()
    ext, base, module = {}, {}, {}
    for block in re.finditer(r"install: \(\) => import\('([^']+)'\),\s*mappings: \{([^}]*)\}", src):
        mod = block.group(1)
        for k, v in re.findall(r"'?([\w.+-]+)'?: '([^']+)'", block.group(2)):
            (ext if k.startswith(".") else base)[k] = v
            module[v] = mod
    return ext, base, module


def main():
    ext, base, module = tables()
    samples = sorted((TESTS / "samples").glob("*"))
    if len(sys.argv) > 1:
        samples = [s for s in samples if s.name in sys.argv[1:] or s.stem in sys.argv[1:]]
    (TESTS / "expected").mkdir(parents=True, exist_ok=True)
    for sample in samples:
        name = sample.name.lower()
        mime = ext.get(sample.suffix.lower()) or base.get(name) or base.get(name.split(".", 1)[0])
        if not mime:
            print(f"skip {sample.name}: no GHD mapping")
            continue
        mod = module[mime]
        mod = mod if mod.startswith("codemirror-mode") else mod.replace("codemirror/mode/", "") + ".js"
        out = subprocess.run(["node", str(Path(__file__).parent / "cmtok.js"), mime, mod, str(sample)],
                             capture_output=True, text=True, check=True).stdout
        rows = [json.loads(l) for l in out.splitlines() if l.strip()]
        text = "".join(f"{r[0]} {r[1]} {r[2]} {r[3]}\n" for r in rows)
        (TESTS / "expected" / f"{sample.name}.tokens").write_text(text)
        print(f"{sample.name}: {mime}, {len(rows)} tokens")


if __name__ == "__main__":
    main()
