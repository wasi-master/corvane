#!/usr/bin/env bash
# Assemble Corvane.app from a cargo build. Ad-hoc signed (no Developer ID).
#
#   packaging/bundle.sh            # debug build  -> target/bundle/Corvane.app
#   packaging/bundle.sh release    # release build -> target/bundle/Corvane.app
#   OPEN=1 packaging/bundle.sh     # also launch it
set -euo pipefail

PROFILE="${1:-debug}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="$ROOT/target/$PROFILE/corvane"
OUT="$ROOT/target/bundle"
APP="$OUT/Corvane.app"
VERSION="$(grep -m1 '^version' "$ROOT/Cargo.toml" | sed -E 's/.*"([^"]+)".*/\1/')"
BUILD="$(git -C "$ROOT" rev-list --count HEAD 2>/dev/null || echo 0)"

if [[ ! -x "$BIN" ]]; then
  echo "binary not found: $BIN (run cargo build${PROFILE:+ --$PROFILE} first)" >&2
  exit 1
fi

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
sed -e "s/__VERSION__/$VERSION/" -e "s/__BUILD__/$BUILD/" \
  "$ROOT/packaging/Info.plist" > "$APP/Contents/Info.plist"
cp "$BIN" "$APP/Contents/MacOS/corvane"
if [[ -f "$ROOT/assets/Corvane.icns" ]]; then
  cp "$ROOT/assets/Corvane.icns" "$APP/Contents/Resources/Corvane.icns"
fi
printf 'APPL????' > "$APP/Contents/PkgInfo"

# Ad-hoc signature so the bundle launches locally; Gatekeeper still quarantines downloads.
codesign --force --sign - --timestamp=none "$APP" >/dev/null

echo "built $APP (v$VERSION build $BUILD)"
if [[ "${OPEN:-0}" == "1" ]]; then
  open -n "$APP"
fi
