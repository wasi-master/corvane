#!/usr/bin/env bash
# Build the on-demand pack archives and their manifest (PLAN.md §3.7).
#
#   packaging/packs.sh [out-dir]        # default: target/release-assets/packs
#
# Produces <out>/syntax-extended-<pack version>.zip (two-face's grammar
# collection as a syntect dump) and <out>/packs-manifest.json whose `url`s
# point at the GitHub release of the app version in Cargo.toml. `release.sh`
# signs the manifest with minisign next to the app assets.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="${1:-$ROOT/target/release-assets/packs}"
APP_VERSION="$(grep -m1 '^version' "$ROOT/Cargo.toml" | sed -E 's/.*"([^"]+)".*/\1/')"
# keep in sync with corvane_highlight::syntaxes::EXTENDED_PACK_VERSION
PACK_VERSION="$(grep -m1 'EXTENDED_PACK_VERSION' "$ROOT/crates/corvane-highlight/src/syntaxes.rs" | sed -E 's/.*"([^"]+)".*/\1/')"
RELEASE_BASE="${RELEASE_BASE:-https://github.com/wasi-master/corvane/releases/download/v$APP_VERSION}"

mkdir -p "$OUT"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

echo "writing the extended grammar dump…"
mkdir -p "$WORK/syntax-extended"
(cd "$ROOT" && cargo run -q -p corvane-highlight --features pack-builder --example dump-extended -- "$WORK/syntax-extended/syntaxes.packdump")

ZIP="$OUT/syntax-extended-$PACK_VERSION.zip"
rm -f "$ZIP"
(cd "$WORK/syntax-extended" && ditto -c -k . "$ZIP")
SHA="$(shasum -a 256 "$ZIP" | cut -d' ' -f1)"
SIZE="$(stat -f%z "$ZIP")"

cat > "$OUT/packs-manifest.json" <<EOF
{
  "schema": 1,
  "packs": [
    {
      "name": "syntax-extended",
      "version": "$PACK_VERSION",
      "min_app": "0.1.0",
      "url": "$RELEASE_BASE/syntax-extended-$PACK_VERSION.zip",
      "sha256": "$SHA",
      "size": $SIZE,
      "kind": "syntax-extended"
    }
  ]
}
EOF
echo "built $ZIP ($SIZE bytes, sha256 $SHA)"
echo "manifest: $OUT/packs-manifest.json"
