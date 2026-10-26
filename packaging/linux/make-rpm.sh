#!/usr/bin/env bash
# DIOS Downloader — build an RPM package (.rpm) from a staged Linux build.
#
# Usage:  ./packaging/linux/make-rpm.sh [stage-dir] [version]
#
#   stage-dir   folder with the staged files (default: build-cache/stage-linux/DIOSDownloader,
#               what build-linux.sh assembles) — same input as make-deb.sh.
#   version     package version (default: exact git tag, else the Cargo.toml version).
#
# Output: dist/dios-downloader-<version>-1.x86_64.rpm
#
# Needs the "rpm" tools only (no root):  Debian/Ubuntu:  sudo apt install rpm
#                                        Fedora:         preinstalled
#
# Layout (same idea as the .deb):
#   /opt/DIOSDownloader/DIOSDownloader        the program
#   /opt/DIOSDownloader/extension[-firefox]/  browser extensions
#   /usr/bin/DIOSDownloader                   symlink into /opt (on PATH)
#   /usr/share/applications/dios-downloader.desktop
#   /usr/share/icons/hicolor/256x256/apps/dios-downloader.png
set -euo pipefail
cd "$(dirname "$0")/../.."   # project root

fail() { printf '\033[31mERROR: %s\033[0m\n' "$*" >&2; exit 1; }
command -v rpmbuild >/dev/null 2>&1 || fail "rpmbuild not found — install it with: sudo apt install rpm  (Fedora/openSUSE have it)"

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
VERSION="${VERSION#v}"
# RPM versions may not contain '-': v1.1.0-beta -> 1.1.0~beta1
RPM_VERSION="$(printf '%s' "$VERSION" | sed -e 's/-\(alpha\|beta\|rc\|dev\)/~\1/' -e 's/-/_/g')"
RELEASE="${RPM_RELEASE:-1}"

echo "==> Building dios-downloader ${RPM_VERSION}-${RELEASE} (x86_64)"

# ---------------------------------------------------------------- sandbox
WORK="build-cache/rpm"
rm -rf "$WORK"
SPEC="$WORK/SPECS/dios-downloader.spec"
mkdir -p "$WORK"/{BUILD,RPMS,SOURCES,SPECS,SRPMS}

# ---------------------------------------------------------------- payload
# rpmbuild packs what the spec's %files list; the tree is staged once
PAYLOAD="$WORK/payload"
mkdir -p "$PAYLOAD/opt/DIOSDownloader" "$PAYLOAD/usr/bin" \
         "$PAYLOAD/usr/share/applications" "$PAYLOAD/usr/share/icons/hicolor/256x256/apps"
cp "$STAGE/DIOSDownloader" "$PAYLOAD/opt/DIOSDownloader/"
chmod 755 "$PAYLOAD/opt/DIOSDownloader/DIOSDownloader"
[ -f "$STAGE/THIRD-PARTY-NOTICES.txt" ] && cp "$STAGE/THIRD-PARTY-NOTICES.txt" "$PAYLOAD/opt/DIOSDownloader/"
[ -f "$STAGE/dios-downloader.png" ] && cp "$STAGE/dios-downloader.png" "$PAYLOAD/opt/DIOSDownloader/"
for ext in extension extension-firefox; do
    [ -d "$STAGE/$ext" ] && cp -r "$STAGE/$ext" "$PAYLOAD/opt/DIOSDownloader/$ext"
done
ls "$STAGE"/dios-downloader-firefox-*.xpi >/dev/null 2>&1 && \
    cp "$STAGE"/dios-downloader-firefox-*.xpi "$PAYLOAD/opt/DIOSDownloader/" || true
ln -s ../../opt/DIOSDownloader/DIOSDownloader "$PAYLOAD/usr/bin/DIOSDownloader"
if [ -f "$STAGE/dios-downloader.desktop" ]; then
    sed 's|^Exec=.*|Exec=/usr/bin/DIOSDownloader|' "$STAGE/dios-downloader.desktop" \
        > "$PAYLOAD/usr/share/applications/dios-downloader.desktop"
fi
[ -f "$STAGE/dios-downloader.png" ] && cp "$STAGE/dios-downloader.png" \
    "$PAYLOAD/usr/share/icons/hicolor/256x256/apps/dios-downloader.png"

# ---------------------------------------------------------------- spec
cat > "$SPEC" <<EOF
%define _binary_payload w9.zstdio
%define _topdir %(echo $WORK)
Name:           dios-downloader
Version:        ${RPM_VERSION}
Release:        ${RELEASE}%{?dist}
Summary:        Fast download manager with multi-connection downloads
License:        MIT
URL:            https://github.com/OussamaTabDev/DIOS-downloader
# payloads are already compiled: no BuildArch trickery, no build step
AutoReqProv:    yes

%description
DIOS Downloader downloads files over several connections at once and
resumes interrupted downloads. It stores videos and audio from web
pages and playlists (yt-dlp), imports .torrent files and magnet links,
sorts finished files into categories and offers queues, scheduling,
speed limits and a browser extension.

The browser extensions are under /opt/DIOSDownloader. Settings and
download lists live in ~/.local/share/dios/DIOSDownloader/data.

%prep
# nothing to unpack: the staged tree is copied over directly

%install
cp -a "$PAYLOAD/." "%{buildroot}/"

%files
/opt/DIOSDownloader
/usr/bin/DIOSDownloader
/usr/share/applications/dios-downloader.desktop
/usr/share/icons/hicolor/256x256/apps/dios-downloader.png

%post
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database /usr/share/applications 2>/dev/null || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -qf /usr/share/icons/hicolor 2>/dev/null || true
fi
exit 0

%changelog
* $(date '+%a %b %d %Y') DIOS Downloader Team <DIOSDownloader@users.noreply.github.com> - ${RPM_VERSION}-${RELEASE}
- See https://github.com/OussamaTabDev/DIOS-downloader/releases
EOF

# ---------------------------------------------------------------- build
rpmbuild --quiet -bb "$SPEC"
mkdir -p dist
found=0
for f in "$WORK"/RPMS/x86_64/*.rpm; do
    [ -f "$f" ] || continue
    mv "$f" dist/
    echo "==> DONE: dist/$(basename "$f")"
    found=1
done
[ "$found" = 1 ] || fail "rpmbuild produced no .rpm (see its output above)"
rm -rf "$WORK"
