#!/usr/bin/env bash
# Build the on-demand pack archives and their manifest (PLAN.md §3.7).
#
#   packaging/packs.sh [out-dir]        # default: target/release-assets/packs
#   PACKS=tree-sitter-all packaging/packs.sh   # only some (space-separated)
#
# Produces in <out>:
# - syntax-extended-<v>.zip: two-face's grammar collection as a syntect dump;
# - tree-sitter-all-<v>.zip / tree-sitter-rest-<v>.zip: corvane-grammars as a
#   universal, ad-hoc signed dylib (every grammar / the ones for languages no
#   CodeMirror port covers) plus the grammars' and queries' licenses;
# - packs-manifest.json whose `url`s point at the GitHub release of the app
#   version in Cargo.toml. `release.sh` signs the manifest with minisign next
#   to the app assets.
#
# The dylibs are built for aarch64 and x86_64 when both Rust targets are
# installed (`rustup target add x86_64-apple-darwin`), else for this Mac only
# (with a warning: such a pack does not load on the other architecture).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="${1:-$ROOT/target/release-assets/packs}"
APP_VERSION="$(grep -m1 '^version' "$ROOT/Cargo.toml" | sed -E 's/.*"([^"]+)".*/\1/')"
# keep in sync with corvane_highlight::syntaxes::EXTENDED_PACK_VERSION
EXTENDED_VERSION="$(grep -m1 'EXTENDED_PACK_VERSION' "$ROOT/crates/corvane-highlight/src/syntaxes.rs" | sed -E 's/.*"([^"]+)".*/\1/')"
# corvane_grammars::PACK_VERSION (the `pack_version!` macro)
GRAMMARS_VERSION="$(grep -A3 'macro_rules! pack_version' "$ROOT/crates/corvane-grammars/src/lib.rs" | grep -m1 -oE '"[0-9][^"]*"' | tr -d '"')"
RELEASE_BASE="${RELEASE_BASE:-https://github.com/wasi-master/corvane/releases/download/v$APP_VERSION}"
PACKS="${PACKS:-syntax-extended tree-sitter-all tree-sitter-rest}"

mkdir -p "$OUT"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
ENTRIES="$WORK/entries.jsonl"
: > "$ENTRIES"

# name version kind zip [target]
add_entry() {
  local sha size
  sha="$(shasum -a 256 "$4" | cut -d' ' -f1)"
  size="$(stat -f%z "$4")"
  python3 - "$1" "$2" "$3" "$(basename "$4")" "$sha" "$size" "${5:-}" "$RELEASE_BASE" >> "$ENTRIES" <<'PY'
import json, sys
name, version, kind, file, sha, size, target, base = sys.argv[1:]
entry = {"name": name, "version": version, "min_app": "0.1.0", "url": f"{base}/{file}",
         "sha256": sha, "size": int(size), "kind": kind}
if target:
    entry["target"] = target
print(json.dumps(entry))
PY
  echo "built $4 ($size bytes, sha256 $sha)"
}

wants() { [[ " $PACKS " == *" $1 "* ]]; }

if wants syntax-extended; then
  echo "writing the extended grammar dump…"
  mkdir -p "$WORK/syntax-extended"
  (cd "$ROOT" && cargo run -q -p corvane-highlight --features pack-builder --example dump-extended -- "$WORK/syntax-extended/syntaxes.packdump")
  ZIP="$OUT/syntax-extended-$EXTENDED_VERSION.zip"
  rm -f "$ZIP"
  (cd "$WORK/syntax-extended" && ditto -c -k . "$ZIP")
  add_entry syntax-extended "$EXTENDED_VERSION" syntax-extended "$ZIP"
fi

TARGETS=()
for t in aarch64-apple-darwin x86_64-apple-darwin; do
  if rustup target list --installed 2>/dev/null | grep -qx "$t"; then
    TARGETS+=("$t")
  fi
done
if [[ ${#TARGETS[@]} -lt 2 ]]; then
  echo "warning: only ${TARGETS[*]:-the host target} installed; the tree-sitter packs will not be universal" >&2
fi

# variant (all | rest)
grammar_pack() {
  local variant="$1" dir="$WORK/tree-sitter-$1" slices=() t lib
  mkdir -p "$dir"
  echo "building the $variant tree-sitter grammars…"
  if [[ ${#TARGETS[@]} -eq 0 ]]; then
    (cd "$ROOT" && cargo build -q --release -p corvane-grammars --no-default-features --features "$variant")
    slices+=("$ROOT/target/release/libcorvane_grammars.dylib")
    cp "${slices[0]}" "$dir/libcorvane_grammars.dylib"
  else
    for t in "${TARGETS[@]}"; do
      (cd "$ROOT" && cargo build -q --release -p corvane-grammars --no-default-features --features "$variant" --target "$t")
      cp "$ROOT/target/$t/release/libcorvane_grammars.dylib" "$WORK/$t-$variant.dylib"
      slices+=("$WORK/$t-$variant.dylib")
    done
    lipo -create -output "$dir/libcorvane_grammars.dylib" "${slices[@]}"
  fi
  lib="$dir/libcorvane_grammars.dylib"
  # arm64 code must carry a signature; lipo keeps the linker's, re-sign to be sure
  codesign --force --sign - --timestamp=none "$lib"
  nm -gU "$lib" | grep -q '_corvane_grammars_v1$' || { echo "$lib does not export corvane_grammars_v1" >&2; exit 1; }
  cp "$ROOT/crates/corvane-grammars/THIRD_PARTY.md" "$dir/THIRD_PARTY.md"
  local zip="$OUT/tree-sitter-$variant-$GRAMMARS_VERSION.zip"
  rm -f "$zip"
  (cd "$dir" && ditto -c -k . "$zip")
  add_entry "tree-sitter-$variant" "$GRAMMARS_VERSION" "tree-sitter-$variant" "$zip" macos
}

wants tree-sitter-all && grammar_pack all
wants tree-sitter-rest && grammar_pack rest

python3 - "$ENTRIES" > "$OUT/packs-manifest.json" <<'PY'
import json, sys
packs = [json.loads(line) for line in open(sys.argv[1]) if line.strip()]
print(json.dumps({"schema": 1, "packs": packs}, indent=2))
PY
echo "manifest: $OUT/packs-manifest.json"
