#!/usr/bin/env python3
"""Readable view of a GHD DOM dump (`dump` scenario steps).

    python3 tools/parity/spec.py target/parity/latest/shots/main-window-dark/01-initial-ghd-dom.json \
        --region 0,80,252,220 [--text] [--depth 30]

One line per element inside the region: indentation = DOM depth, tag.class,
own text, rect (window points) and only the styles that differ from its
parent's (inherited values are dropped), so it reads like a spec.
"""

from __future__ import annotations

import argparse
import json
import sys


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("dump")
    ap.add_argument("--region", help="x,y,w,h in window points (elements intersecting it)")
    ap.add_argument("--text", action="store_true", help="only elements with own text")
    ap.add_argument("--depth", type=int, default=99)
    ap.add_argument("--grep", help="substring of class / id / text")
    args = ap.parse_args()
    items = json.load(open(args.dump))
    rx = ry = rw = rh = None
    if args.region:
        rx, ry, rw, rh = map(float, args.region.split(","))
    parents: dict[int, dict] = {}
    for e in items:
        depth = e["depth"]
        parent = parents.get(depth - 1, {})
        parents[depth] = e["style"]
        if depth > args.depth:
            continue
        x, y, w, h = e["rect"]
        if rx is not None and (x + w < rx or x > rx + rw or y + h < ry or y > ry + rh):
            continue
        if args.text and not e.get("text"):
            continue
        label = e["tag"] + (f"#{e['id']}" if e.get("id") else "") + ("." + e["cls"].replace(" ", ".") if e.get("cls") else "")
        if args.grep and args.grep not in label and args.grep not in (e.get("text") or ""):
            continue
        own = {k: v for k, v in e["style"].items() if parent.get(k) != v}
        style = " ".join(f"{k}={v}" for k, v in own.items())
        text = f' "{e["text"]}"' if e.get("text") else ""
        rect = ",".join(f"{v:g}" for v in e["rect"])
        print(f"{'  ' * min(depth, 30)}{label}{text} [{rect}] {style}")


if __name__ == "__main__":
    sys.exit(main())
