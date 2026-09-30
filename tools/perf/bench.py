#!/usr/bin/env python3
"""Latency benchmarks for the actions people use most.

Drives a Corvane build (`--features snapshots`) over `CORVANE_CONTROL` with the
same input the parity harness sends and times, inside the app, how long each
action takes until the state it asks for is there (`bench` command in
`crates/corvane/src/parity_control.rs`) plus the frame that shows it.

    cargo build --profile profiling -p corvane --features snapshots
    python3 tools/perf/fixture.py big target/perf/big
    python3 tools/perf/bench.py [--binary target/profiling/corvane] [--repo target/perf/big] [--runs 5] [case…]

Prints a table (median / p90 / max of total ms per case) and writes
`target/perf/latest.json`. `CORVANE_FLAGS` passes through (compare presets
or experimental flags: `CORVANE_FLAGS=preset=github-desktop`).
"""

from __future__ import annotations

import argparse
import json
import os
import statistics
import subprocess
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools" / "parity"))
from drivers import Corvane  # noqa: E402

W, H = 1367, 814
# GHD layout at 1367×814 (points): tabs, the Changes list's first row
# (below the filter and select-all rows), rows 29pt
SIDEBAR_X = 125
TAB_CHANGES = (62, 97)
TAB_HISTORY = (187, 97)
LIST_TOP = 170
ROW = 29
DIFF_POINT = (800, 400)
TOOLBAR_REPOSITORY = (125, 56)
COMMIT_SUMMARY = (140, 583)  # with the Undo bar below the form; 627 without
FIRST_BRANCH_ROW = (300, 171)
TOOLBAR_BRANCH = (370, 56)


def row_y(index: int, top: int = LIST_TOP) -> float:
    return top + index * ROW + ROW / 2


class Bench:
    def __init__(self, binary: Path, repo: Path, runs: int, extra_env: dict):
        self.binary, self.repo, self.runs, self.extra_env = binary, repo, runs, extra_env
        self.results: dict[str, list[dict]] = {}
        self.data = Path(tempfile.mkdtemp(prefix="corvane-perf-"))
        self.cv: Corvane | None = None

    # -- app lifecycle ---------------------------------------------------
    def start(self) -> float:
        started = time.perf_counter()
        self.cv = Corvane(self.binary, self.data, self.data / "corvane.log", "dark")
        self.cv.start(timeout=60, extra_env=self.extra_env)
        return started

    def stop(self):
        if self.cv:
            self.cv.stop()
            self.cv = None

    def bench(self, steps: list[dict], until: str, timeout_ms: int = 30_000) -> dict:
        assert self.cv
        return self.cv.cmd("bench", steps=steps, until=until, timeout_ms=timeout_ms)

    def record(self, case: str, reply: dict):
        self.results.setdefault(case, []).append(reply)

    # -- helpers ----------------------------------------------------------
    @staticmethod
    def click(x, y, **kw) -> dict:
        return {"cmd": "click", "x": x, "y": y, **kw}

    @staticmethod
    def key(keys: str) -> dict:
        return {"cmd": "key", "keys": keys}

    @staticmethod
    def type_(text: str) -> dict:
        return {"cmd": "type", "text": text}

    def status_files(self) -> list[str]:
        out = subprocess.run(
            ["git", "-C", str(self.repo), "status", "--porcelain=v1", "-z", "--untracked-files=all"],
            capture_output=True, check=True).stdout.decode()
        paths = [e[3:] for e in out.split("\0") if e]
        return sorted(paths, key=str.lower)

    def commits(self, n: int) -> list[str]:
        out = subprocess.run(["git", "-C", str(self.repo), "log", f"-{n}", "--format=%H"],
                             capture_output=True, check=True, text=True).stdout
        return out.split()

    # -- cases --------------------------------------------------------------
    def setup(self):
        started = self.start()
        cv = self.cv
        cv.resize(W, H)
        cv.hook("complete-welcome")
        cv.hook("add-repo", str(self.repo))
        reply = self.bench([], f"repo:{self.repo.name}", 60_000)
        reply["total_ms"] = (time.perf_counter() - started) * 1000
        self.record("first-open (spawn → repo ready)", reply)

    def run_cases(self, cases: set[str]):
        files = self.status_files()
        want = lambda c: not cases or c in cases  # noqa: E731
        for _ in range(self.runs):
            if want("refresh"):
                self.record("refresh (focus)", self.bench([{"cmd": "hook", "name": "refresh"}], "idle"))
            if want("select-file"):
                self.bench([self.click(*TAB_CHANGES)], "idle")
                for i in (3, 0, 5):
                    self.record("select file (click)", self.bench([self.click(SIDEBAR_X, row_y(i))], f"diff:{files[i]}"))
            if want("next-file"):
                self.bench([self.click(SIDEBAR_X, row_y(0))], f"diff:{files[0]}")
                for i in range(1, 6):
                    self.record("next file (↓)", self.bench([self.key("down")], f"diff:{files[i]}"))
            if want("big-diff"):
                i = files.index("src/big.rs") if "src/big.rs" in files else 0
                j = 1 if i == 0 else 0
                self.bench([self.click(SIDEBAR_X, row_y(j))], f"diff:{files[j]}")
                self.record("open 5k-line diff", self.bench([self.click(SIDEBAR_X, row_y(i))], f"diff:{files[i]}"))
            if want("scroll-diff"):
                r = self.cv.cmd("scroll-frames", x=DIFF_POINT[0], y=DIFF_POINT[1], dy=120, n=30)
                self.record("scroll diff (frame)", {"total_ms": r["max_ms"], **r})
            if want("scroll-changes"):
                r = self.cv.cmd("scroll-frames", x=SIDEBAR_X, y=400, dy=120, n=30)
                self.record("scroll changes list (frame)", {"total_ms": r["max_ms"], **r})
                self.cv.cmd("scroll-frames", x=SIDEBAR_X, y=400, dy=-120, n=30)
            if want("toggle"):
                self.record("toggle include (click)", self.bench([self.click(16, row_y(2))], "frame"))
                self.bench([self.click(16, row_y(2))], "frame")
            if want("history"):
                self.bench([self.key("cmd-1")], "idle")
                self.record("open History (⌘2)", self.bench([self.key("cmd-2")], "commits:1"))
                shas = self.cv.cmd("state")["commits"]
                at = shas.index(self.cv.cmd("state")["selected_commit"])
                # walk down, or up when near the end of the loaded page
                step, key = (1, "down") if at + 6 < len(shas) else (-1, "up")
                for i in range(1, 6):
                    self.record("next commit (↓)", self.bench([self.key(key)], f"commit:{shas[at + step * i][:12]}"))
                r = self.cv.cmd("scroll-frames", x=SIDEBAR_X, y=400, dy=240, n=40)
                self.record("scroll history (frame)", {"total_ms": r["max_ms"], **r})
                self.cv.cmd("scroll-frames", x=SIDEBAR_X, y=400, dy=-240, n=40)
                self.bench([self.key("cmd-1")], "idle")
            if want("branches"):
                self.record("open branches (click)", self.bench([self.click(*TOOLBAR_BRANCH)], "frame"))
                for ch in "topic-4":
                    self.record("branch filter keystroke", self.bench([self.type_(ch)], "frame"))
                self.bench([self.key("escape")], "frame")
            if want("type-summary"):
                self.bench([self.click(*COMMIT_SUMMARY)], "frame")
                for ch in "Fix the parser":
                    self.record("commit summary keystroke", self.bench([self.type_(ch)], "frame"))
                self.bench([self.key("cmd-a"), self.key("backspace"), self.key("escape")], "frame")
            if want("repo-list"):
                self.record("open repository list (click)", self.bench([self.click(*TOOLBAR_REPOSITORY)], "frame"))
                self.bench([self.key("escape")], "frame")
            if want("frames"):
                r = self.cv.cmd("frames", n=30)
                self.record("idle frame (changes)", {"total_ms": r["max_ms"], **r})

    def clean_repo(self) -> Path:
        """A clone of the fixture without local changes (checkout, commit)."""
        clean = self.repo.parent / f"{self.repo.name}-clean"
        if not clean.exists():
            subprocess.run(["git", "clone", "-q", str(self.repo), str(clean)], check=True)
            subprocess.run(["git", "-C", str(clean), "config", "commit.gpgsign", "false"], check=True)
            # branches a few commits from main, as feature branches usually are
            for b, at in (("bench-near", "main~5"), ("bench-other", "main~12")):
                subprocess.run(["git", "-C", str(clean), "branch", "-q", b, at], check=True)
        return clean

    def switch_to(self, repo: Path) -> dict:
        self.bench([self.click(*TOOLBAR_REPOSITORY), self.key("cmd-a backspace")], "frame")
        return self.bench([self.type_(repo.name), self.key("enter")], f"repo:{repo.name}", 25_000)

    def run_clean_cases(self, cases: set[str]):
        want = lambda c: not cases or c in cases  # noqa: E731
        if not any(want(c) for c in ("switch-repo", "checkout", "commit")):
            return
        clean = self.clean_repo()
        self.cv.hook("add-repo", str(clean))
        self.bench([], f"repo:{clean.name}", 25_000)
        for i in range(self.runs):
            if want("switch-repo"):
                self.record("switch repository (foldout, enter)", self.switch_to(self.repo))
                self.bench([], "idle")
                self.record("switch repository (foldout, enter)", self.switch_to(clean))
                self.bench([], "idle")
            if want("checkout"):
                for name in ("bench-near", "main", "bench-other", "main"):
                    self.bench([self.click(*TOOLBAR_BRANCH), self.key("cmd-a backspace"), self.type_(name)], "frame")
                    # the first row under the filter (no Enter handler in the list)
                    self.record("checkout branch", self.bench([self.click(*FIRST_BRANCH_ROW)], f"branch:{name}", 25_000))
                    self.bench([], "idle")
            if want("commit"):
                target = clean / "src" / "mod01" / "sub01" / "file01050.rs"
                target.write_text(target.read_text() + f"// bench edit {time.time()}\n")
                self.record("external edit → shown (watcher)", self.bench([], "files:1", 10_000))
                self.bench([], "idle")
                y = COMMIT_SUMMARY[1] if self.cv.cmd("state").get("undo_bar") else 627
                self.bench([self.click(COMMIT_SUMMARY[0], y)], "frame")
                self.record("commit (⌘↩ → list empty)", self.bench([self.type_(f"Bench commit {i}"), self.key("cmd-enter")], "files:0", 25_000))
                self.bench([self.key("escape")], "frame")

    def relaunch(self):
        """Quit and start again on the same store: spawn → repository ready."""
        self.stop()
        started = self.start()
        # whichever repository was selected last
        reply = self.bench([], "repo:", 25_000)
        reply["total_ms"] = (time.perf_counter() - started) * 1000
        self.record("relaunch (spawn → repo ready)", reply)
        reply = self.bench([], "idle", 25_000)
        reply["total_ms"] = (time.perf_counter() - started) * 1000
        self.record("relaunch (spawn → idle, diff shown)", reply)

    def report(self) -> str:
        """Medians. `cpu` is main-thread CPU time of the frame (and of the
        input handling), which a loaded machine does not inflate."""
        med = lambda rows, k: statistics.median(r[k] for r in rows)  # noqa: E731
        lines = [f"{'case':38} {'total':>7} {'p90':>7} {'settle':>7} {'draw':>6} {'cpu':>6} {'Minstr':>7}   (ms, medians; Minstr = main-thread instructions, millions)"]
        for case, rows in self.results.items():
            totals = sorted(r["total_ms"] for r in rows)
            p90 = totals[min(len(totals) - 1, int(len(totals) * 0.9))]
            if "settle_ms" in rows[0]:
                cpu = med(rows, "draw_cpu_ms") + med(rows, "input_cpu_ms") if "draw_cpu_ms" in rows[0] else float("nan")
                instr = med(rows, "draw_minstr") + med(rows, "input_minstr") if "draw_minstr" in rows[0] else float("nan")
                lines.append(f"{case:38} {statistics.median(totals):7.1f} {p90:7.1f} {med(rows, 'settle_ms'):7.1f} {med(rows, 'draw_ms'):6.1f} {cpu:6.1f} {instr:7.1f}")
            else:
                cpu = med(rows, "cpu_ms") if "cpu_ms" in rows[0] else float("nan")
                instr = med(rows, "minstr") if "minstr" in rows[0] else float("nan")
                lines.append(f"{case:38} {med(rows, 'avg_ms'):7.1f} {p90:7.1f} {'':>7} {'':>6} {cpu:6.1f} {instr:7.1f}   (per frame)")
        return "\n".join(lines)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--binary", type=Path, default=ROOT / "target" / "profiling" / "corvane")
    ap.add_argument("--repo", type=Path, default=ROOT / "target" / "perf" / "big")
    ap.add_argument("--runs", type=int, default=5)
    ap.add_argument("--out", type=Path, default=ROOT / "target" / "perf" / "latest.json")
    ap.add_argument("--keep", action="store_true", help="leave the app running")
    ap.add_argument("cases", nargs="*")
    args = ap.parse_args()
    # the default preset unless asked (the parity driver pins github-desktop)
    extra = {"CORVANE_FLAGS": os.environ.get("CORVANE_FLAGS", "")}
    b = Bench(args.binary.resolve(), args.repo.resolve(), args.runs, extra)
    try:
        b.setup()
        b.run_cases(set(args.cases))
        b.run_clean_cases(set(args.cases))
        if not args.cases or "relaunch" in args.cases:
            b.relaunch()
    except Exception as err:  # keep what was measured
        print(f"error: {err}", file=sys.stderr)
    finally:
        if not args.keep:
            b.stop()
    print(b.report())
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(b.results, indent=1))


if __name__ == "__main__":
    main()
