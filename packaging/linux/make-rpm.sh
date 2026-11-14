#!/usr/bin/env bash
# Batiie — build an RPM package (.rpm) from a staged Linux build.
#
# Usage:  ./packaging/linux/make-rpm.sh [stage-dir] [version]
#
#   stage-dir   folder with the staged files (default: build-cache/stage-linux/Batiie,
#               what build-linux.sh assembles) — same input as make-deb.sh.
#   version     package version (default: exact git tag, else the Cargo.toml version).
#
# Output: dist/batiie-<version>-1.x86_64.rpm
#
# Needs the "rpm" tools only (no root):  Debian/Ubuntu:  sudo apt install rpm
#                                        Fedora:         preinstalled
#
# Layout (same idea as the .deb):
#   /opt/Batiie/Batiie        the program
#   /opt/Batiie/extension[-firefox]/  browser extensions
#   /usr/bin/Batiie                   symlink into /opt (on PATH)
#   /usr/share/applications/batiie.desktop
#   /usr/share/icons/hicolor/256x256/apps/batiie.png
set -euo pipefail
cd "$(dirname "$0")/../.."   # project root

fail() { printf '\033[31mERROR: %s\033[0m\n' "$*" >&2; exit 1; }
command -v rpmbuild >/dev/null 2>&1 || fail "rpmbuild not found — install it with: sudo apt install rpm  (Fedora/openSUSE have it)"

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
VERSION="${VERSION#v}"
# RPM versions may not contain '-': v1.1.0-beta -> 1.1.0~beta1
RPM_VERSION="$(printf '%s' "$VERSION" | sed -e 's/-\(alpha\|beta\|rc\|dev\)/~\1/' -e 's/-/_/g')"
RELEASE="${RPM_RELEASE:-1}"

echo "==> Building batiie ${RPM_VERSION}-${RELEASE} (x86_64)"

# ---------------------------------------------------------------- sandbox
# Absolute paths everywhere: rpmbuild runs the %install scriptlet from its
# own BUILD directory, so a relative payload path would point nowhere
# (this is exactly the 'cannot stat build-cache/rpm/payload/.' failure).
WORK="$PWD/build-cache/rpm"
rm -rf "$WORK"
SPEC="$WORK/SPECS/batiie.spec"
mkdir -p "$WORK"/{BUILD,RPMS,SOURCES,SPECS,SRPMS}

# ---------------------------------------------------------------- payload
# rpmbuild packs what the spec's %files list; the tree is staged once
PAYLOAD="$WORK/payload"
mkdir -p "$PAYLOAD/opt/Batiie" "$PAYLOAD/usr/bin" \
         "$PAYLOAD/usr/share/applications" "$PAYLOAD/usr/share/icons/hicolor/256x256/apps"
cp "$STAGE/Batiie" "$PAYLOAD/opt/Batiie/"
chmod 755 "$PAYLOAD/opt/Batiie/Batiie"
[ -f "$STAGE/THIRD-PARTY-NOTICES.txt" ] && cp "$STAGE/THIRD-PARTY-NOTICES.txt" "$PAYLOAD/opt/Batiie/"
[ -f "$STAGE/batiie.png" ] && cp "$STAGE/batiie.png" "$PAYLOAD/opt/Batiie/"
for ext in extension extension-firefox; do
    [ -d "$STAGE/$ext" ] && cp -r "$STAGE/$ext" "$PAYLOAD/opt/Batiie/$ext"
done
ls "$STAGE"/batiie-firefox-*.xpi >/dev/null 2>&1 && \
    cp "$STAGE"/batiie-firefox-*.xpi "$PAYLOAD/opt/Batiie/" || true
ln -s ../../opt/Batiie/Batiie "$PAYLOAD/usr/bin/Batiie"
if [ -f "$STAGE/batiie.desktop" ]; then
    sed 's|^Exec=.*|Exec=/usr/bin/Batiie %U|' "$STAGE/batiie.desktop" \
        > "$PAYLOAD/usr/share/applications/batiie.desktop"
fi
[ -f "$STAGE/batiie.png" ] && cp "$STAGE/batiie.png" \
    "$PAYLOAD/usr/share/icons/hicolor/256x256/apps/batiie.png"

# ---------------------------------------------------------------- spec
cat > "$SPEC" <<EOF
%define _topdir $WORK
Name:           batiie
Version:        ${RPM_VERSION}
Release:        ${RELEASE}%{?dist}
Summary:        Fast download manager with multi-connection downloads
License:        MIT
URL:            https://github.com/OussamaTabDev/DIOS-downloader
# payloads are already compiled: no BuildArch trickery, no build step
# (no _binary_payload override: the distro default always works, while a
#  hard-coded zstd setting breaks on rpm builds without zstd support)
AutoReqProv:    yes
# the window is a web view
Requires:       (webkit2gtk4.1 or libwebkit2gtk-4_1-0)
Requires:       (gtk3 or libgtk-3-0)

%description
Batiie downloads files over several connections at once and
resumes interrupted downloads. It stores videos and audio from web
pages and playlists (yt-dlp), imports .torrent files and magnet links,
sorts finished files into categories and offers queues, scheduling,
speed limits and a browser extension.

The browser extensions are under /opt/Batiie. Settings and
download lists live in ~/.local/share/batiie/Batiie/data.

%prep
# nothing to unpack: the staged tree is copied over directly

%install
cp -a "$PAYLOAD/." "%{buildroot}/"

%files
/opt/Batiie
/usr/bin/Batiie
/usr/share/applications/batiie.desktop
/usr/share/icons/hicolor/256x256/apps/batiie.png

%post
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database /usr/share/applications 2>/dev/null || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -qf /usr/share/icons/hicolor 2>/dev/null || true
fi
exit 0

%changelog
* $(date '+%a %b %d %Y') Batiie Team <Batiie@users.noreply.github.com> - ${RPM_VERSION}-${RELEASE}
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
