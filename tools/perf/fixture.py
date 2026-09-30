#!/usr/bin/env python3
"""Synthetic repositories for the latency benchmarks (`tools/perf/bench.py`).

`big`: 50,000 tracked files in nested directories, 20,000 commits (built with
`git fast-import`, a few seconds), 500 branches, 40 tags, then a dirty working
tree: 200 modified files (one 5,000-line source file among them), 30 untracked
files, one staged file. Deterministic author and dates.

    python3 tools/perf/fixture.py big <dir>
"""

from __future__ import annotations

import os
import random
import subprocess
import sys
from pathlib import Path

AUTHOR = "Perf Bench <perf@example.com>"
EPOCH = 1_700_000_000


def git(repo: Path, *args: str, stdin: bytes | None = None) -> None:
    env = dict(os.environ, GIT_CONFIG_NOSYSTEM="1", HOME=str(repo.parent))
    subprocess.run(["git", "-C", str(repo), *args], input=stdin, check=True, env=env,
                   stdout=subprocess.DEVNULL)


def source_file(rng: random.Random, lines: int) -> bytes:
    out = []
    for i in range(lines):
        indent = "    " * (i % 4)
        out.append(f"{indent}fn item_{i}(value: u32) -> u32 {{ value.wrapping_mul({rng.randrange(1, 999)}) }} // {i}\n")
    return "".join(out).encode()


def big(root: Path, files: int = 50_000, commits: int = 20_000, branches: int = 500) -> None:
    rng = random.Random(7)
    if root.exists():
        subprocess.run(["rm", "-rf", str(root)], check=True)
    root.mkdir(parents=True)
    git(root, "init", "-q", "-b", "main")
    git(root, "config", "commit.gpgsign", "false")
    git(root, "config", "user.name", "Perf Bench")
    git(root, "config", "user.email", "perf@example.com")

    paths = [f"src/mod{i // 1000:02}/sub{(i // 50) % 20:02}/file{i:05}.rs" for i in range(files)]
    paths[0] = "src/big.rs"
    stream = bytearray()
    mark = 0

    def blob(data: bytes) -> int:
        nonlocal mark
        mark += 1
        stream.extend(b"blob\nmark :%d\ndata %d\n" % (mark, len(data)))
        stream.extend(data)
        stream.extend(b"\n")
        return mark

    # the initial tree: every file in one commit
    initial = []
    for i, path in enumerate(paths):
        data = source_file(rng, 5000 if i == 0 else rng.randrange(5, 60))
        initial.append((path, blob(data)))
    stamp = EPOCH
    commit_marks = []
    for n in range(commits):
        if n == 0:
            changes = initial
        else:
            changes = []
            for _ in range(rng.randrange(1, 4)):
                path = paths[rng.randrange(1, files)]
                changes.append((path, blob(source_file(rng, rng.randrange(5, 60)))))
        mark += 1
        commit_mark = mark
        stamp += 600
        msg = f"Change {n}: adjust {changes[0][0].rsplit('/', 1)[-1]}\n".encode()
        stream.extend(b"commit refs/heads/main\nmark :%d\n" % commit_mark)
        stream.extend(b"author %s %d +0000\ncommitter %s %d +0000\n" % (AUTHOR.encode(), stamp, AUTHOR.encode(), stamp))
        stream.extend(b"data %d\n%s" % (len(msg), msg))
        if commit_marks:
            stream.extend(b"from :%d\n" % commit_marks[-1])
        for path, b in changes:
            stream.extend(b"M 100644 :%d %s\n" % (b, path.encode()))
        stream.extend(b"\n")
        commit_marks.append(commit_mark)
    for b in range(branches):
        at = commit_marks[rng.randrange(len(commit_marks) // 2, len(commit_marks))]
        stream.extend(b"reset refs/heads/feature/topic-%03d\nfrom :%d\n\n" % (b, at))
    for t in range(40):
        stream.extend(b"reset refs/tags/v1.%d.0\nfrom :%d\n\n" % (t, commit_marks[t * (commits // 40)]))
    git(root, "fast-import", "--quiet", stdin=bytes(stream))
    git(root, "checkout", "-q", "-f", "main")

    # dirty working tree
    for i in rng.sample(range(1, files), 199):
        p = root / paths[i]
        p.write_text(p.read_text() + f"// edited {i}\n")
    big_rs = root / paths[0]
    text = big_rs.read_text().splitlines(keepends=True)
    for i in range(0, len(text), 25):
        text[i] = text[i].replace("wrapping_mul", "wrapping_add")
    big_rs.write_text("".join(text))
    for i in range(30):
        (root / f"untracked-{i:02}.txt").write_text(f"new file {i}\n" * 20)
    staged = root / paths[1]
    staged.write_text(staged.read_text() + "// staged\n")
    git(root, "add", paths[1])
    git(root, "gc", "-q", "--aggressive" if "--aggressive" in sys.argv else "--auto")
    print(root)


if __name__ == "__main__":
    kind, target = sys.argv[1], Path(sys.argv[2]).resolve()
    {"big": big}[kind](target)
