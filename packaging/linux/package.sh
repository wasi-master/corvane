#!/bin/bash
# Build Corvane's Linux packages from a release build, for this machine's
# architecture (amd64 / x86_64 or arm64 / aarch64):
#   target/linux/corvane_<version>_<amd64|arm64>.deb
#   target/linux/Corvane-<version>-<x86_64|aarch64>.AppImage   (when appimagetool is found)
#
# Layout (both):
#   usr/lib/corvane/corvane          the app
#   usr/lib/corvane/bin/corvane      the command line tool (corvane.sh)
#   usr/bin/corvane -> ../lib/corvane/bin/corvane   (.deb only)
#   usr/share/applications/com.wasimaster.corvane.desktop
#   usr/share/icons/hicolor/{256x256,scalable}/apps/com.wasimaster.corvane.*
#
# Env: CORVANE_UPDATE_PUBLIC_KEY (else packaging/corvane-release.pub),
# SKIP_BUILD=1 (reuse target/release/corvane), PACKAGE_BIN=<binary> (package
# that binary instead, e.g. CI's debug build), APPIMAGETOOL=<path>.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
VERSION="$(cargo metadata --no-deps --format-version 1 |
  python3 -c 'import json,sys; print(next(p["version"] for p in json.load(sys.stdin)["packages"] if p["name"] == "corvane"))')"
# the build machine's architecture (release.yml builds each natively)
case "$(uname -m)" in
  x86_64) ARCH_DEB=amd64 ARCH_APPIMAGE=x86_64 ;;
  aarch64 | arm64) ARCH_DEB=arm64 ARCH_APPIMAGE=aarch64 ;;
  *) echo "unsupported architecture $(uname -m)" >&2; exit 1 ;;
esac
OUT="$ROOT/target/linux"
ID=com.wasimaster.corvane

if [ -z "${CORVANE_UPDATE_PUBLIC_KEY:-}" ] && [ -f packaging/corvane-release.pub ]; then
  CORVANE_UPDATE_PUBLIC_KEY="$(tail -n 1 packaging/corvane-release.pub)"
  export CORVANE_UPDATE_PUBLIC_KEY
fi
if [ -z "${SKIP_BUILD:-}" ] && [ -z "${PACKAGE_BIN:-}" ]; then
  cargo build --release -p corvane
fi
BIN="${PACKAGE_BIN:-$ROOT/target/release/corvane}"
[ -x "$BIN" ] || { echo "no $BIN" >&2; exit 1; }

rm -rf "$OUT"
mkdir -p "$OUT"

# stage the shared tree into $1
stage() {
  local root="$1"
  install -Dm755 "$BIN" "$root/usr/lib/corvane/corvane"
  install -Dm755 packaging/linux/corvane.sh "$root/usr/lib/corvane/bin/corvane"
  install -Dm644 packaging/linux/$ID.desktop "$root/usr/share/applications/$ID.desktop"
  install -Dm644 assets/icon/Corvane-256.png "$root/usr/share/icons/hicolor/256x256/apps/$ID.png"
  install -Dm644 assets/icon/Corvane.svg "$root/usr/share/icons/hicolor/scalable/apps/$ID.svg"
}

# ---- .deb -------------------------------------------------------------
DEB="$OUT/deb"
stage "$DEB"
mkdir -p "$DEB/usr/bin"
ln -s ../lib/corvane/bin/corvane "$DEB/usr/bin/corvane"
SIZE_KB="$(du -sk "$DEB/usr" | cut -f1)"
mkdir -p "$DEB/DEBIAN"
cat > "$DEB/DEBIAN/control" <<EOF
Package: corvane
Version: $VERSION
Architecture: $ARCH_DEB
Maintainer: Corvane <corvane@wasimaster.com>
Installed-Size: $SIZE_KB
Depends: git (>= 2.35), libc6 (>= 2.35), libxcb1, libxkbcommon0, libxkbcommon-x11-0, libfontconfig1, libfreetype6, libvulkan1, xdg-utils, perl
Recommends: mesa-vulkan-drivers, gnome-keyring | kwalletmanager | keepassxc, hunspell-en-us, fonts-noto-core
Section: devel
Priority: optional
Homepage: https://github.com/wasi-master/corvane
Description: Native Git client that looks and works like GitHub Desktop
 Corvane is a native reimplementation of GitHub Desktop 3.6.6: the same
 changes, history, branches and pull request workflow, without Electron.
EOF
cat > "$DEB/DEBIAN/postinst" <<'EOF'
#!/bin/sh
set -e
if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database -q /usr/share/applications || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache -q -t -f /usr/share/icons/hicolor || true
fi
EOF
cp "$DEB/DEBIAN/postinst" "$DEB/DEBIAN/postrm"
chmod 755 "$DEB/DEBIAN/postinst" "$DEB/DEBIAN/postrm"
dpkg-deb --root-owner-group --build "$DEB" "$OUT/corvane_${VERSION}_${ARCH_DEB}.deb" >/dev/null
rm -rf "$DEB"
echo "built $OUT/corvane_${VERSION}_${ARCH_DEB}.deb"

# ---- AppImage -----------------------------------------------------------
TOOL="${APPIMAGETOOL:-$(command -v appimagetool || true)}"
if [ -z "$TOOL" ]; then
  echo "appimagetool not found: skipping the AppImage" >&2
  exit 0
fi
APPDIR="$OUT/Corvane.AppDir"
stage "$APPDIR"
# AppImage desktop entries name the executable without a path
sed -e 's|^Exec=.*|Exec=corvane %U|' -e 's|^TryExec=.*||' \
  packaging/linux/$ID.desktop > "$APPDIR/$ID.desktop"
cp assets/icon/Corvane-256.png "$APPDIR/$ID.png"
install -Dm755 packaging/linux/AppRun "$APPDIR/AppRun"
ARCH=$ARCH_APPIMAGE "$TOOL" --no-appstream "$APPDIR" "$OUT/Corvane-${VERSION}-${ARCH_APPIMAGE}.AppImage" >/dev/null
rm -rf "$APPDIR"
echo "built $OUT/Corvane-${VERSION}-${ARCH_APPIMAGE}.AppImage"
