#!/usr/bin/env bash
# Build the release assets (PLAN.md §3.8, packaging/release.md):
#
#   packaging/release.sh                 # release build → target/release-assets/
#   FULL=1 packaging/release.sh          # the "full" variant (packs compiled in)
#   SKIP_BUILD=1 packaging/release.sh    # reuse target/release/corvane
#   UPDATE_CASK=1 packaging/release.sh   # also rewrite packaging/homebrew/Casks/corvane.rb
#   SECRET_KEY=~/.minisign/corvane-release.key packaging/release.sh   # sign here
#
# Output: Corvane[-Full]-<version>-macos-<universal|arch>.zip (+ .dmg), a
# .minisig for every asset when a secret key is given, the packs manifest and
# archives, and the cask's sha256 on stdout. Creates no git tags or remotes.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$ROOT/target/release-assets"
VERSION="$(grep -m1 '^version' "$ROOT/Cargo.toml" | sed -E 's/.*"([^"]+)".*/\1/')"
VARIANT="Corvane"
FEATURES=()
if [[ "${FULL:-0}" == "1" ]]; then
  VARIANT="Corvane-Full"
  FEATURES=(--features full)
fi

# --- signing key -----------------------------------------------------------
PUB_FILE="$ROOT/packaging/corvane-release.pub"
if [[ -z "${CORVANE_UPDATE_PUBLIC_KEY:-}" ]]; then
  if [[ -f "$PUB_FILE" ]]; then
    export CORVANE_UPDATE_PUBLIC_KEY="$(tail -n1 "$PUB_FILE")"
  else
    echo "warning: $PUB_FILE is missing; the self-updater will ship a placeholder key (packaging/release.md)" >&2
  fi
fi
SIGNER=""
if command -v minisign >/dev/null 2>&1; then
  SIGNER="minisign"
elif command -v rsign >/dev/null 2>&1; then
  SIGNER="rsign"
fi

# --- build ------------------------------------------------------------------
if [[ "${SKIP_BUILD:-0}" != "1" ]]; then
  echo "building $VARIANT $VERSION (release)…"
  (cd "$ROOT" && cargo build --release -p corvane "${FEATURES[@]}")
fi

# a universal binary when both per-target builds exist (CI: two `cargo build
# --target` runs), else the native one
ARCH_TAG="$(uname -m)"
[[ "$ARCH_TAG" == "arm64" ]] && ARCH_TAG="arm64" || ARCH_TAG="x86_64"
BIN="$ROOT/target/release/corvane"
if [[ -x "$ROOT/target/aarch64-apple-darwin/release/corvane" && -x "$ROOT/target/x86_64-apple-darwin/release/corvane" ]]; then
  mkdir -p "$ROOT/target/release"
  lipo -create -output "$BIN" \
    "$ROOT/target/aarch64-apple-darwin/release/corvane" \
    "$ROOT/target/x86_64-apple-darwin/release/corvane"
  ARCH_TAG="universal"
fi
[[ -x "$BIN" ]] || { echo "no release binary at $BIN" >&2; exit 1; }

"$ROOT/packaging/bundle.sh" release
APP="$ROOT/target/bundle/Corvane.app"
mkdir -p "$OUT"

# --- assets -----------------------------------------------------------------
BASE="$VARIANT-$VERSION-macos-$ARCH_TAG"
ZIP="$OUT/$BASE.zip"
DMG="$OUT/$BASE.dmg"
rm -f "$ZIP" "$DMG"
(cd "$(dirname "$APP")" && ditto -c -k --sequesterRsrc --keepParent "$(basename "$APP")" "$ZIP")
hdiutil create -quiet -volname Corvane -srcfolder "$APP" -ov -format UDZO "$DMG"
echo "zip: $ZIP ($(stat -f%z "$ZIP") bytes)"
echo "dmg: $DMG"

if [[ "${FULL:-0}" != "1" ]]; then
  "$ROOT/packaging/packs.sh" "$OUT/packs"
  cp "$OUT/packs/packs-manifest.json" "$OUT/packs-manifest.json"
  cp "$OUT"/packs/*.zip "$OUT/"
fi

# --- signatures -------------------------------------------------------------
sign() {
  local asset="$1"
  rm -f "$asset.minisig"
  case "$SIGNER" in
    minisign) minisign -S -s "$SECRET_KEY" -t "corvane v$VERSION $(basename "$asset")" -x "$asset.minisig" -m "$asset" ;;
    rsign) rsign sign -s "$SECRET_KEY" -t "corvane v$VERSION $(basename "$asset")" -x "$asset.minisig" "$asset" ;;
  esac
}
if [[ -n "${SECRET_KEY:-}" && -n "$SIGNER" ]]; then
  for asset in "$OUT"/*.zip "$OUT"/*.dmg "$OUT"/packs-manifest.json; do
    [[ -f "$asset" ]] && sign "$asset" && echo "signed $(basename "$asset")"
  done
else
  echo "not signed: set SECRET_KEY=<minisign secret key> (and install minisign or rsign) or sign in CI" >&2
fi

# --- cask -------------------------------------------------------------------
SHA="$(shasum -a 256 "$ZIP" | cut -d' ' -f1)"
echo "cask sha256: $SHA"
if [[ "${UPDATE_CASK:-0}" == "1" && "${FULL:-0}" != "1" ]]; then
  CASK="$ROOT/packaging/homebrew/Casks/corvane.rb"
  sed -i '' -E "s/^  version \"[^\"]+\"/  version \"$VERSION\"/; s/^  sha256 \"[^\"]+\"/  sha256 \"$SHA\"/" "$CASK"
  echo "updated $CASK"
fi
echo "assets in $OUT"
