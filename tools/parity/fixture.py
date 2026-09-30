"""Deterministic fixture repository shown by both apps.

Fixed author, committer and dates make every copy byte-identical (same SHAs),
so GHD and Corvane each get their own copy (no index.lock races, destructive
scenarios stay possible) that renders the same text.
"""

from __future__ import annotations

import os
import shutil
import subprocess
from pathlib import Path

NAME = "parity-fixture"
AUTHOR = ("Parity Bot", "parity@example.com")

_COMMITS = [
    ("2026-08-03T09:12:00+00:00", "Initial commit", "", {
        "README.md": "# Parity fixture\n\nA small repository for GitHub Desktop parity runs.\n",
        ".gitignore": "target/\n*.log\n",
    }),
    ("2026-08-05T14:40:00+00:00", "Add the command line entry point", "Parses arguments and prints a greeting.", {
        "src/main.rs": 'fn main() {\n    let name = std::env::args().nth(1).unwrap_or_else(|| "world".into());\n    println!("Hello, {name}!");\n}\n',
        "Cargo.toml": '[package]\nname = "parity-fixture"\nversion = "0.1.0"\nedition = "2024"\n',
    }),
    ("2026-08-11T10:05:00+00:00", "Write the user guide", "", {
        "docs/guide.md": "# Guide\n\n1. Build with `cargo build`.\n2. Run `parity-fixture <name>`.\n\nThat is all.\n",
        "docs/old.md": "Outdated notes that will be deleted.\n",
    }),
    ("2026-08-19T16:30:00+00:00", "Support a --shout flag", "Upper-cases the greeting.\n\nCloses #4.", {
        "src/main.rs": 'fn main() {\n    let args: Vec<String> = std::env::args().skip(1).collect();\n    let shout = args.iter().any(|a| a == "--shout");\n    let name = args.iter().find(|a| !a.starts_with("--")).cloned().unwrap_or_else(|| "world".into());\n    let line = format!("Hello, {name}!");\n    println!("{}", if shout { line.to_uppercase() } else { line });\n}\n',
    }),
    ("2026-09-02T08:00:00+00:00", "Add a logo", "", {
        "assets/logo.svg": '<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"><circle cx="8" cy="8" r="7" fill="#2da44e"/></svg>\n',
    }),
    ("2026-09-14T11:45:00+00:00", "Explain flags in the README", "", {
        "README.md": "# Parity fixture\n\nA small repository for GitHub Desktop parity runs.\n\n## Flags\n\n- `--shout`: upper-case the greeting\n",
    }),
]

_WORKING_CHANGES = {
    "README.md": "# Parity fixture\n\nA small repository for GitHub Desktop parity runs.\n\n## Flags\n\n- `--shout`: upper-case the greeting\n- `--quiet`: print nothing\n\n## License\n\nMIT\n",
    "notes.txt": "Remember to update the changelog.\n",
    "src/lib.rs": "pub fn greet(name: &str) -> String {\n    format!(\"Hello, {name}!\")\n}\n",
    # one changed word (intra-line highlight) next to a line long enough to wrap
    "src/main.rs": 'fn main() {\n    let args: Vec<String> = std::env::args().skip(1).collect();\n    let shout = args.iter().any(|a| a == "--loud");\n    let name = args.iter().find(|a| !a.starts_with("--")).cloned().unwrap_or_else(|| std::env::var("USER").unwrap_or_else(|_| "world".into())).trim().to_string(); // falls back to $USER, then to "world"\n    let line = format!("Hello, {name}!");\n    println!("{}", if shout { line.to_uppercase() } else { line });\n}\n',
}
_DELETED = ["docs/old.md"]


def _git(repo: Path, *args: str, date: str | None = None):
    env = {k: v for k, v in os.environ.items() if not k.startswith("GIT_")}
    env.update(
        GIT_AUTHOR_NAME=AUTHOR[0], GIT_AUTHOR_EMAIL=AUTHOR[1],
        GIT_COMMITTER_NAME=AUTHOR[0], GIT_COMMITTER_EMAIL=AUTHOR[1],
        GIT_CONFIG_NOSYSTEM="1",
    )
    if date:
        env.update(GIT_AUTHOR_DATE=date, GIT_COMMITTER_DATE=date)
    subprocess.run(["git", *args], cwd=repo, env=env, check=True, capture_output=True)


def build(parent: Path) -> Path:
    """(Re)create `<parent>/parity-fixture` and return its path."""
    repo = parent / NAME
    if repo.exists():
        shutil.rmtree(repo)
    repo.mkdir(parents=True)
    _git(repo, "init", "-q", "-b", "main")
    for key, value in (("user.name", AUTHOR[0]), ("user.email", AUTHOR[1]), ("commit.gpgsign", "false"), ("tag.gpgsign", "false")):
        _git(repo, "config", key, value)
    for i, (date, summary, body, files) in enumerate(_COMMITS):
        for rel, text in files.items():
            p = repo / rel
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text(text)
        _git(repo, "add", "-A")
        msg = summary + (f"\n\n{body}" if body else "")
        _git(repo, "commit", "-q", "-m", msg, date=date)
        if i == 3:
            _git(repo, "branch", "feature/login")
            _git(repo, "branch", "bugfix/typo-in-guide")
    _git(repo, "tag", "v0.1.0", "HEAD~1")
    for rel, text in _WORKING_CHANGES.items():
        p = repo / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(text)
    for rel in _DELETED:
        (repo / rel).unlink()
    return repo


if __name__ == "__main__":
    import sys

    print(build(Path(sys.argv[1] if len(sys.argv) > 1 else ".")))
