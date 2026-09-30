#!/usr/bin/env python3
"""Cached-view staleness check: after each interaction, snap the frame a
display link would draw (`snap {cached: true}`: only invalidated views
re-render) and a forced full re-render; a difference means a cached view
missed an update. The parity harness cannot see this (its snaps re-render
everything). A 3 px wide difference is the text caret blinking.

    python3 tools/perf/stale_check.py [--binary target/profiling/corvane] [--repo target/perf/big]
"""
import argparse
import sys
import tempfile
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "parity"))
from bench import ROOT, Bench, row_y  # noqa: E402
from PIL import Image, ImageChops  # noqa: E402

ap = argparse.ArgumentParser()
ap.add_argument("--binary", type=Path, default=ROOT / "target" / "profiling" / "corvane")
ap.add_argument("--repo", type=Path, default=ROOT / "target" / "perf" / "big")
args = ap.parse_args()
S = tempfile.mkdtemp(prefix="corvane-stale-")
b = Bench(args.binary.resolve(), args.repo.resolve(), 1, {"CORVANE_FLAGS": ""})
b.setup(); b.bench([], "idle")
cv = b.cv
steps = [
  ("filter-click", [b.click(200,129)]), ("filter-type", [b.type_("mod0")]), ("filter-clear", [b.key("cmd-a backspace")]),
  ("hover-row", [{"cmd":"move","x":150,"y":row_y(3)}]), ("select-row", [b.click(150,row_y(3))]),
  ("down", [b.key("down")]), ("toggle", [b.click(16,row_y(1))]), ("toggle-back", [b.click(16,row_y(1))]),
  ("summary", [b.click(140,583), b.type_("Hello")]), ("summary-clear", [b.key("cmd-a backspace")]),
  ("diff-scroll", [{"cmd":"scroll","x":800,"y":400,"dx":0,"dy":300}]), ("diff-hover-gutter", [{"cmd":"move","x":420,"y":300}]),
  ("list-scroll", [{"cmd":"scroll","x":150,"y":400,"dx":0,"dy":200}]),
  ("history", [b.click(187,97)]), ("history-hover", [{"cmd":"move","x":150,"y":300}]), ("history-select", [b.click(150,300)]),
  ("history-down", [b.key("down")]), ("history-scroll", [{"cmd":"scroll","x":150,"y":400,"dx":0,"dy":300}]),
  ("changes", [b.click(62,97)]),
  ("branches", [b.click(370,56)]), ("branch-type", [b.type_("topic-1")]), ("branch-hover", [{"cmd":"move","x":500,"y":250}]), ("branch-esc", [b.key("escape")]),
  ("repos", [b.click(125,56)]), ("repos-esc", [b.key("escape")]),
  ("refresh", [{"cmd":"hook","name":"refresh"}]),
]
bad = 0
for name, st in steps:
    b.bench(st, "frame")
    time.sleep(0.4)  # background work (highlighting, diff loads) lands
    b.bench([], "idle")
    a, f = f"{S}/stale-{name}-cached.png", f"{S}/stale-{name}-full.png"
    cv.cmd("snap", path=a, cached=True)
    cv.cmd("snap", path=f)
    d = ImageChops.difference(Image.open(a).convert("RGB"), Image.open(f).convert("RGB"))
    box = d.getbbox()
    n = sum(1 for p in d.getdata() if max(p) > 24) if box else 0
    print(f"{name:20} {'OK' if n < 50 else 'STALE'} {n} px {box}")
    bad += n >= 50
b.stop()
print("stale steps:", bad, "(images in", S + ")")
sys.exit(1 if bad else 0)
