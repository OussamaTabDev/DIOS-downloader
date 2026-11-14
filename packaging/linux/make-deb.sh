#!/usr/bin/env bash
# Batiie — build a Debian package (.deb) from a staged Linux build.
#
# Usage:  ./packaging/linux/make-deb.sh [stage-dir] [version]
#
#   stage-dir   folder that holds the staged files (default: build-cache/stage-linux/Batiie,
#               what build-linux.sh assembles): the binary, extension/, extension-firefox/,
#               batiie.desktop, batiie.png, THIRD-PARTY-NOTICES.txt
#               and (when present) the signed Firefox .xpi.
#   version     package version (default: exact git tag, else the Cargo.toml version).
#
# Output: dist/batiie_<version>_amd64.deb
#
# Layout inside the package (kept out of package-manager-managed program dirs
# so nothing else ever writes there):
#   /opt/Batiie/Batiie        the program
#   /opt/Batiie/extension[-firefox]/  browser extensions
#   /usr/bin/Batiie                   symlink into /opt (on PATH)
#   /usr/share/applications/batiie.desktop
#   /usr/share/icons/hicolor/256x256/apps/batiie.png
#
# yt-dlp and ffmpeg are NOT in the package: the app downloads them itself into
# ~/.local/share/batiie/Batiie/data/tools (SHA-256 checked), which stays
# writable no matter how the program was installed.
set -euo pipefail
cd "$(dirname "$0")/../.."   # project root

fail() { printf '\033[31mERROR: %s\033[0m\n' "$*" >&2; exit 1; }

STAGE="${1:-build-cache/stage-linux/Batiie}"
VERSION="${2:-}"

[ -d "$STAGE" ] || fail "stage dir not found: $STAGE (run build-linux.sh first, or pass the folder)"
[ -f "$STAGE/Batiie" ] || fail "$STAGE/Batiie is missing"

# ---------------------------------------------------------------- version
if [ -z "$VERSION" ]; then
    VERSION="$(git describe --tags --exact-match 2>/dev/null | sed 's/^v//')" || true
fi
if [ -z "$VERSION" ]; then
    VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
fi
[ -n "$VERSION" ] || VERSION="0.0.0"
# Debian wants [0-9.~+-]; a leading v or a -beta suffix stays readable: 1.1.0~beta1
DEB_VERSION="$(printf '%s' "$VERSION" | sed -e 's/^v//' -e 's/-\(alpha\|beta\|rc\|dev\)/~\1/')"

echo "==> Building batiie ${DEB_VERSION} (amd64)"

# ---------------------------------------------------------------- sandbox
WORK="build-cache/deb"
rm -rf "$WORK"
ROOT="$WORK/batiie_${DEB_VERSION}_amd64"
mkdir -p "$ROOT/DEBIAN" \
         "$ROOT/opt/Batiie" \
         "$ROOT/usr/bin" \
         "$ROOT/usr/share/applications" \
         "$ROOT/usr/share/icons/hicolor/256x256/apps"

# ---------------------------------------------------------------- payload
cp "$STAGE/Batiie" "$ROOT/opt/Batiie/"
chmod 755 "$ROOT/opt/Batiie/Batiie"
[ -f "$STAGE/THIRD-PARTY-NOTICES.txt" ] && cp "$STAGE/THIRD-PARTY-NOTICES.txt" "$ROOT/opt/Batiie/"
[ -f "$STAGE/batiie.png" ] && cp "$STAGE/batiie.png" "$ROOT/opt/Batiie/"
for ext in extension extension-firefox; do
    [ -d "$STAGE/$ext" ] && cp -r "$STAGE/$ext" "$ROOT/opt/Batiie/$ext"
done
ls "$STAGE"/batiie-firefox-*.xpi >/dev/null 2>&1 && \
    cp "$STAGE"/batiie-firefox-*.xpi "$ROOT/opt/Batiie/" || true

# launcher on PATH, desktop entry, icon
ln -s ../../opt/Batiie/Batiie "$ROOT/usr/bin/Batiie"
if [ -f "$STAGE/batiie.desktop" ]; then
    sed 's|^Exec=.*|Exec=/usr/bin/Batiie %U|' "$STAGE/batiie.desktop" \
        > "$ROOT/usr/share/applications/batiie.desktop"
fi
[ -f "$STAGE/batiie.png" ] && cp "$STAGE/batiie.png" \
    "$ROOT/usr/share/icons/hicolor/256x256/apps/batiie.png"

# ---------------------------------------------------------------- control
# Built with the GitHub Actions ubuntu-22.04 image the binary needs glibc 2.35;
# when you build on an older distro, lower this (or set DEB_LIBC_MIN).
LIBC_MIN="${DEB_LIBC_MIN:-2.35}"
cat > "$ROOT/DEBIAN/control" <<EOF
Package: batiie
Version: ${DEB_VERSION}
Section: net
Priority: optional
Architecture: amd64
Installed-Size: $(du -sk --exclude=DEBIAN "$ROOT" | cut -f1)
Maintainer: Batiie Team <Batiie@users.noreply.github.com>
Depends: libc6 (>= ${LIBC_MIN}), libwebkit2gtk-4.1-0, libgtk-3-0
Recommends: xdg-desktop-portal, libayatana-appindicator3-1
Suggests: ffmpeg
Homepage: https://github.com/OussamaTabDev/DIOS-downloader
Description: Fast download manager with multi-connection downloads
 Batiie downloads files over several connections at once and
 resumes interrupted downloads. It stores videos and audio from web
 pages and playlists (yt-dlp), imports .torrent files and magnet
 links, sorts finished files into categories and offers queues,
 scheduling, speed limits and a browser extension.
 .
 The browser extensions are under /opt/Batiie. Settings and
 download lists live in ~/.local/share/batiie/Batiie/data.
EOF

cat > "$ROOT/DEBIAN/postinst" <<'EOF'
#!/bin/sh
# refresh the caches so the menu entry and icon appear at once
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database /usr/share/applications 2>/dev/null || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -qf /usr/share/icons/hicolor 2>/dev/null || true
fi
exit 0
EOF
# dpkg requires the maintainer script to be executable, control files not
chmod 755 "$ROOT/DEBIAN/postinst"
chmod 644 "$ROOT/DEBIAN/control"

# ---------------------------------------------------------------- build
mkdir -p dist
OUT="dist/batiie_${DEB_VERSION}_amd64.deb"
dpkg-deb --build --root-owner-group "$ROOT" "$OUT"
rm -rf "$WORK"

echo "==> DONE: $OUT"
