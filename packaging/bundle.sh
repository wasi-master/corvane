#!/usr/bin/env bash
# Assemble Corvane.app from a cargo build. Signed with the self-signed
# "Corvane Self-Signed" certificate when the keychain has it
# (packaging/signing-cert.sh), else ad-hoc; never with a Developer ID.
#
#   packaging/bundle.sh            # debug build  -> target/bundle/Corvane.app
#   packaging/bundle.sh release    # release build -> target/bundle/Corvane.app
#   OPEN=1 packaging/bundle.sh     # also launch it
#   CORVANE_SIGN_IDENTITY=- …      # ad-hoc even when the certificate is there
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

# macOS 26 Liquid Glass icon: compile the Icon Composer package (assets/icon/Corvane.icon)
# into Assets.car. Needs actool from Xcode 26+; the Command Line Tools alone do not ship it.
# Without it the bundle falls back to the legacy .icns below (Finder ignores
# CFBundleIconName when Assets.car is absent, so the key is only added on success).
if ACTOOL="$(xcrun --find actool 2>/dev/null)"; then
  "$ACTOOL" "$ROOT/assets/icon/Corvane.icon" \
    --compile "$APP/Contents/Resources" \
    --platform macosx --minimum-deployment-target 26.0 \
    --app-icon Corvane \
    --output-format human-readable-text --errors --warnings \
    --output-partial-info-plist "$OUT/Corvane-icon.plist" >/dev/null
  if [[ ! -f "$APP/Contents/Resources/Assets.car" ]]; then
    echo "actool did not produce Assets.car" >&2
    exit 1
  fi
  /usr/libexec/PlistBuddy -c "Add :CFBundleIconName string Corvane" "$APP/Contents/Info.plist"
else
  echo "actool not found (Xcode 26+); skipping Liquid Glass icon, using Corvane.icns only" >&2
fi
# Hand-tuned legacy icon (16/32 px variants) wins over the flattened one actool emits.
if [[ -f "$ROOT/assets/Corvane.icns" ]]; then
  cp "$ROOT/assets/Corvane.icns" "$APP/Contents/Resources/Corvane.icns"
fi
printf 'APPL????' > "$APP/Contents/PkgInfo"
# Command line tool (Install Command Line Tool… symlinks it into /usr/local/bin)
install -m 755 "$ROOT/packaging/corvane.sh" "$APP/Contents/Resources/corvane"

# A certificate keeps the designated requirement (identifier + certificate)
# the same from build to build, so Keychain items and privacy permissions
# granted to one build carry over; an ad-hoc one is the binary's hash and
# changes with every build. Either way Gatekeeper still quarantines downloads.
IDENTITY="${CORVANE_SIGN_IDENTITY:-}"
if [[ -z "$IDENTITY" ]]; then
  IDENTITY="-"
  if security find-identity -p codesigning 2>/dev/null | grep -q '"Corvane Self-Signed"'; then
    IDENTITY="Corvane Self-Signed"
  fi
fi
codesign --force --sign "$IDENTITY" --timestamp=none "$APP" >/dev/null

echo "built $APP (v$VERSION build $BUILD, signed: ${IDENTITY/#-/ad-hoc})"
if [[ "${OPEN:-0}" == "1" ]]; then
  open -n "$APP"
fi
