#!/usr/bin/env python3
"""Rebuild GitHub Desktop's CodeMirror (runmode + every highlighter mode)
from the source maps inside the installed app, for the oracle.

    python3 tools/cm-oracle/extract.py [--app "/Applications/GitHub Desktop.app"]

Writes target/cm-oracle/node_modules/{codemirror,codemirror-mode-*} and
target/cm-oracle/src/index.ts (GHD's highlighter worker, for reference).
Nothing from GHD is committed: the tree is regenerated from the app.
"""

import argparse
import glob
import json
import os
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--app", default="/Applications/GitHub Desktop.app")
    ap.add_argument("--out", default=str(ROOT / "target" / "cm-oracle"))
    args = ap.parse_args()
    app = Path(args.app) / "Contents" / "Resources" / "app"
    out = Path(args.out)
    maps = [app / "highlighter.js.map", *map(Path, glob.glob(str(app / "highlighter" / "**" / "*.map"), recursive=True))]
    n = 0
    for mapfile in maps:
        m = json.loads(mapfile.read_text())
        for src, content in zip(m["sources"], m.get("sourcesContent") or []):
            if not content:
                continue
            if "node_modules/" in src:
                rel = "node_modules/" + src.split("node_modules/", 1)[1]
            elif "app/src/highlighter/" in src:
                rel = "src/" + src.split("app/src/highlighter/", 1)[1]
            else:
                continue
            p = out / rel
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text(content)
            n += 1
    # runmode.node.js swaps these two module ids for itself in require.cache;
    # they only need to resolve
    for stub in ("node_modules/codemirror/lib/codemirror.js", "node_modules/codemirror/addon/runmode/runmode.js"):
        p = out / stub
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text("// replaced by runmode.node.js's require-cache shim\n")
    print(f"{n} files → {out}")


if __name__ == "__main__":
    main()
