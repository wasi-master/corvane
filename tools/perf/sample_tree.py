#!/usr/bin/env python3
"""Summarise a macOS `sample` report: inclusive and self sample counts per
function on one thread (the main thread by default), Rust v0 symbols shortened
to `crate::…::name`.

    sample <pid> 5 -file out.txt
    python3 tools/perf/sample_tree.py out.txt [--thread main] [--top 60] [--filter corvane]
"""

from __future__ import annotations

import argparse
import re
from collections import Counter

LINE = re.compile(r"^(?P<indent>[ +!:|]*)(?P<count>\d+) (?P<sym>.+?)(?:  \(in (?P<lib>[^)]+)\))?(?:  \+ .*|  \[.*)?$")


def short(sym: str) -> str:
    """Drop offsets; demangle first (`--demangle <rustc-demangle filter>`)."""
    return sym.split(" + ")[0]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("file")
    ap.add_argument("--thread", default="main-thread")
    ap.add_argument("--top", type=int, default=60)
    ap.add_argument("--filter", default="")
    ap.add_argument("--demangle", help="a filter that demangles Rust symbols on stdin")
    args = ap.parse_args()
    text = open(args.file, errors="replace").read()
    if args.demangle:
        import subprocess
        text = subprocess.run([args.demangle], input=text, capture_output=True, text=True).stdout
    lines = text.splitlines()
    inclusive, selfc = Counter(), Counter()
    in_thread = False
    stack: list[tuple[int, str, int]] = []
    total = 0
    for line in lines:
        if line.startswith("    ") and "Thread_" in line and line.strip()[0].isdigit():
            in_thread = args.thread in line
            stack = []
            if in_thread:
                total = int(line.split()[0])
            continue
        if line.startswith("Total number in stack"):
            break
        if not in_thread:
            continue
        m = LINE.match(line)
        if not m:
            continue
        depth = len(m.group("indent"))
        count = int(m.group("count"))
        name = short(m.group("sym").strip())
        while stack and stack[-1][0] >= depth:
            d, n, c = stack.pop()
        # self time of the parent shrinks by this child
        if stack:
            pd, pn, pc = stack[-1]
            selfc[pn] -= count
        selfc[name] += count
        # count a function once per path (recursion)
        if name not in {n for _, n, _ in stack}:
            inclusive[name] += count
        stack.append((depth, name, count))
    print(f"thread {args.thread}: {total} samples")
    print(f"{'incl':>7} {'self':>7}  function")
    rows = [(inclusive[n], selfc[n], n) for n in inclusive if args.filter in n]
    for inc, sf, n in sorted(rows, reverse=True)[: args.top]:
        print(f"{inc:7} {sf:7}  {n[:150]}")


if __name__ == "__main__":
    main()
