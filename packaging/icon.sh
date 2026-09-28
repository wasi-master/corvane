#!/usr/bin/env bash
# Regenerates the app icon artefacts from the SVG sources in assets/icon/:
#   assets/Corvane.icns                  legacy icon (macOS 15, and 26 without Assets.car)
#   assets/icon/Corvane-1024.png         marketing / README
#   assets/icon/Corvane.icon/Assets/*.png  Icon Composer layers (macOS 26 Liquid Glass)
# Requires rsvg-convert (brew install librsvg) and iconutil (Xcode CLT).
# Sizes <= 32 px use Corvane-small.svg (thicker stroke, no shadow) per Apple HIG.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SRC="$ROOT/assets/icon"
OUT="$(mktemp -d)/Corvane.iconset"
mkdir -p "$OUT"
for spec in 16:1 16:2 32:1 32:2 128:1 128:2 256:1 256:2 512:1 512:2; do
  pt="${spec%%:*}"; scale="${spec##*:}"; px=$((pt * scale))
  svg="$SRC/Corvane.svg"; [[ $px -le 32 ]] && svg="$SRC/Corvane-small.svg"
  name="icon_${pt}x${pt}"; [[ $scale == 2 ]] && name="${name}@2x"
  rsvg-convert -w "$px" -h "$px" "$svg" -o "$OUT/$name.png"
done
cp "$OUT/icon_512x512@2x.png" "$SRC/Corvane-1024.png"
iconutil -c icns "$OUT" -o "$ROOT/assets/Corvane.icns"
echo "wrote assets/Corvane.icns"

# Icon Composer layers: white glyph on transparent, full 1024 canvas (Tahoe applies the mask).
for layer in "$SRC"/layers/*.svg; do
  name="$(basename "${layer%.svg}")"
  rsvg-convert -w 1024 -h 1024 "$layer" -o "$SRC/Corvane.icon/Assets/$name.png"
done
echo "wrote assets/icon/Corvane.icon/Assets/*.png"
