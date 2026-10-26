#!/usr/bin/env bash
# DIOS Downloader — build a Debian package (.deb) from a staged Linux build.
#
# Usage:  ./packaging/linux/make-deb.sh [stage-dir] [version]
#
#   stage-dir   folder that holds the staged files (default: build-cache/stage-linux/DIOSDownloader,
#               what build-linux.sh assembles): the binary, extension/, extension-firefox/,
#               dios-downloader.desktop, dios-downloader.png, THIRD-PARTY-NOTICES.txt
#               and (when present) the signed Firefox .xpi.
#   version     package version (default: exact git tag, else the Cargo.toml version).
#
# Output: dist/dios-downloader_<version>_amd64.deb
#
# Layout inside the package (kept out of package-manager-managed program dirs
# so nothing else ever writes there):
#   /opt/DIOSDownloader/DIOSDownloader        the program
#   /opt/DIOSDownloader/extension[-firefox]/  browser extensions
#   /usr/bin/DIOSDownloader                   symlink into /opt (on PATH)
#   /usr/share/applications/dios-downloader.desktop
#   /usr/share/icons/hicolor/256x256/apps/dios-downloader.png
#
# yt-dlp and ffmpeg are NOT in the package: the app downloads them itself into
# ~/.local/share/dios/DIOSDownloader/data/tools (SHA-256 checked), which stays
# writable no matter how the program was installed.
set -euo pipefail
cd "$(dirname "$0")/../.."   # project root

fail() { printf '\033[31mERROR: %s\033[0m\n' "$*" >&2; exit 1; }

STAGE="${1:-build-cache/stage-linux/DIOSDownloader}"
VERSION="${2:-}"

[ -d "$STAGE" ] || fail "stage dir not found: $STAGE (run build-linux.sh first, or pass the folder)"
[ -f "$STAGE/DIOSDownloader" ] || fail "$STAGE/DIOSDownloader is missing"

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

echo "==> Building dios-downloader ${DEB_VERSION} (amd64)"

# ---------------------------------------------------------------- sandbox
WORK="build-cache/deb"
rm -rf "$WORK"
ROOT="$WORK/dios-downloader_${DEB_VERSION}_amd64"
mkdir -p "$ROOT/DEBIAN" \
         "$ROOT/opt/DIOSDownloader" \
         "$ROOT/usr/bin" \
         "$ROOT/usr/share/applications" \
         "$ROOT/usr/share/icons/hicolor/256x256/apps"

# ---------------------------------------------------------------- payload
cp "$STAGE/DIOSDownloader" "$ROOT/opt/DIOSDownloader/"
chmod 755 "$ROOT/opt/DIOSDownloader/DIOSDownloader"
[ -f "$STAGE/THIRD-PARTY-NOTICES.txt" ] && cp "$STAGE/THIRD-PARTY-NOTICES.txt" "$ROOT/opt/DIOSDownloader/"
[ -f "$STAGE/dios-downloader.png" ] && cp "$STAGE/dios-downloader.png" "$ROOT/opt/DIOSDownloader/"
for ext in extension extension-firefox; do
    [ -d "$STAGE/$ext" ] && cp -r "$STAGE/$ext" "$ROOT/opt/DIOSDownloader/$ext"
done
ls "$STAGE"/dios-downloader-firefox-*.xpi >/dev/null 2>&1 && \
    cp "$STAGE"/dios-downloader-firefox-*.xpi "$ROOT/opt/DIOSDownloader/" || true

# launcher on PATH, desktop entry, icon
ln -s ../../opt/DIOSDownloader/DIOSDownloader "$ROOT/usr/bin/DIOSDownloader"
if [ -f "$STAGE/dios-downloader.desktop" ]; then
    sed 's|^Exec=.*|Exec=/usr/bin/DIOSDownloader|' "$STAGE/dios-downloader.desktop" \
        > "$ROOT/usr/share/applications/dios-downloader.desktop"
fi
[ -f "$STAGE/dios-downloader.png" ] && cp "$STAGE/dios-downloader.png" \
    "$ROOT/usr/share/icons/hicolor/256x256/apps/dios-downloader.png"

# ---------------------------------------------------------------- control
# Built with the GitHub Actions ubuntu-22.04 image the binary needs glibc 2.35;
# when you build on an older distro, lower this (or set DEB_LIBC_MIN).
LIBC_MIN="${DEB_LIBC_MIN:-2.35}"
cat > "$ROOT/DEBIAN/control" <<EOF
Package: dios-downloader
Version: ${DEB_VERSION}
Section: net
Priority: optional
Architecture: amd64
Installed-Size: $(du -sk --exclude=DEBIAN "$ROOT" | cut -f1)
Maintainer: DIOS Downloader Team <DIOSDownloader@users.noreply.github.com>
Depends: libc6 (>= ${LIBC_MIN}), libfontconfig1, libxkbcommon0
Recommends: libwayland-client0, xdg-desktop-portal
Suggests: ffmpeg
Homepage: https://github.com/OussamaTabDev/DIOS-downloader
Description: Fast download manager with multi-connection downloads
 DIOS Downloader downloads files over several connections at once and
 resumes interrupted downloads. It stores videos and audio from web
 pages and playlists (yt-dlp), imports .torrent files and magnet
 links, sorts finished files into categories and offers queues,
 scheduling, speed limits and a browser extension.
 .
 The browser extensions are under /opt/DIOSDownloader. Settings and
 download lists live in ~/.local/share/dios/DIOSDownloader/data.
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
OUT="dist/dios-downloader_${DEB_VERSION}_amd64.deb"
dpkg-deb --build --root-owner-group "$ROOT" "$OUT"
rm -rf "$WORK"

echo "==> DONE: $OUT"
