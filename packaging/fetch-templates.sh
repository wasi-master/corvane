#!/usr/bin/env bash
# Re-fetch the gitignore and license templates bundled in assets/templates.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$ROOT/assets/templates"
mkdir -p "$OUT/gitignore" "$OUT/licenses"
curl -s "https://api.github.com/repos/github/gitignore/contents?ref=main" \
  | python3 -c 'import json,sys
for e in json.load(sys.stdin):
    if e["type"]=="file" and e["name"].endswith(".gitignore"): print(e["name"], e["download_url"])' \
  | while read -r name url; do curl -sfL "$url" -o "$OUT/gitignore/$name"; done
curl -s "https://api.github.com/repos/github/choosealicense.com/contents/_licenses?ref=gh-pages" \
  | python3 -c 'import json,sys
for e in json.load(sys.stdin):
    if e["type"]=="file" and e["name"].endswith(".txt"): print(e["name"], e["download_url"])' \
  | while read -r name url; do curl -sfL "$url" -o "$OUT/licenses/$name"; done
echo "gitignores: $(ls "$OUT/gitignore" | wc -l), licenses: $(ls "$OUT/licenses" | wc -l)"
