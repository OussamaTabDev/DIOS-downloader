#!/usr/bin/env bash
# DIOS Downloader — build an AppImage from a staged Linux build.
#
# Usage:  ./packaging/linux/make-appimage.sh [stage-dir] [version]
#
#   stage-dir   folder with the staged files (default: build-cache/stage-linux/DIOSDownloader,
#               what build-linux.sh assembles) — same input as make-deb.sh.
#   version     version string for the file name (default: git tag, else Cargo.toml).
#
# Output: dist/DIOSDownloader-<version>-x86_64.AppImage
#
# The AppImage is self-contained: program, both browser extensions and the
# desktop entry. yt-dlp/ffmpeg are fetched by the app itself into the user's
# data dir on first use, so the read-only mount is never a problem.
#
# appimagetool (the official packing tool) is downloaded on first use into
# build-cache/ — set APPIMAGETOOL to a local copy to skip the download.
# It runs with --appimage-extract-and-run, so FUSE is not required (CI-friendly).
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
VERSION="${VERSION#v}"

echo "==> Building DIOSDownloader $VERSION AppImage (x86_64)"

# ---------------------------------------------------------------- appimagetool
APPIMAGETOOL_URL="${APPIMAGETOOL_URL:-https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage}"
TOOL="build-cache/appimagetool-x86_64.AppImage"
mkdir -p build-cache
if [ -z "${APPIMAGETOOL:-}" ]; then
    if [ ! -f "$TOOL" ]; then
        echo "==> Downloading appimagetool (one-time, into build-cache/)"
        if command -v curl >/dev/null 2>&1; then
            curl -fL --retry 3 -o "$TOOL" "$APPIMAGETOOL_URL"
        elif command -v wget >/dev/null 2>&1; then
            wget -q -O "$TOOL" "$APPIMAGETOOL_URL"
        else
            fail "neither curl nor wget is available to fetch appimagetool"
        fi
    fi
    chmod +x "$TOOL"
    APPIMAGETOOL="$TOOL"
fi
RUNMODE=()
# appimagetool is itself an AppImage. FUSE mounting is unreliable in
# containers and CI, so unpack-and-run is the default; opt back in with
# APPIMAGETOOL_USE_FUSE=1 (faster when you have working FUSE).
if [ "${APPIMAGETOOL_USE_FUSE:-0}" != "1" ] || { ! command -v fusermount >/dev/null 2>&1 && ! command -v fusermount3 >/dev/null 2>&1; }; then
    RUNMODE=(--appimage-extract-and-run)
fi

# ---------------------------------------------------------------- AppDir
WORK="build-cache/appimage"
rm -rf "$WORK"
APPDIR="$WORK/AppDir"
mkdir -p "$APPDIR/usr/bin"

# the program — the extensions sit NEXT TO the binary, which is where the
# app looks for them (Help > Add DIOS Downloader to my browser)
cp "$STAGE/DIOSDownloader" "$APPDIR/usr/bin/"
chmod 755 "$APPDIR/usr/bin/DIOSDownloader"
for ext in extension extension-firefox; do
    [ -d "$STAGE/$ext" ] && cp -r "$STAGE/$ext" "$APPDIR/usr/bin/$ext"
done
ls "$STAGE"/dios-downloader-firefox-*.xpi >/dev/null 2>&1 && \
    cp "$STAGE"/dios-downloader-firefox-*.xpi "$APPDIR/usr/bin/" || true
[ -f "$STAGE/THIRD-PARTY-NOTICES.txt" ] && cp "$STAGE/THIRD-PARTY-NOTICES.txt" "$APPDIR/usr/bin/"

# desktop entry + icons (appimagetool reads them from the AppDir root)
[ -f "$STAGE/dios-downloader.desktop" ] || fail "$STAGE/dios-downloader.desktop is missing"
cp "$STAGE/dios-downloader.desktop" "$APPDIR/"
[ -f "$STAGE/dios-downloader.png" ] || fail "$STAGE/dios-downloader.png is missing"
cp "$STAGE/dios-downloader.png" "$APPDIR/dios-downloader.png"
cp "$STAGE/dios-downloader.png" "$APPDIR/.DirIcon"
mkdir -p "$APPDIR/usr/share/icons/hicolor/256x256/apps"
cp "$STAGE/dios-downloader.png" "$APPDIR/usr/share/icons/hicolor/256x256/apps/dios-downloader.png"

# AppRun: exec the real binary (readlink resolves the AppImage path)
cat > "$APPDIR/AppRun" <<'EOF'
#!/bin/sh
# DIOS Downloader AppRun — start the program with all arguments passed through
HERE="$(dirname "$(readlink -f "$0")")"
exec "$HERE/usr/bin/DIOSDownloader" "$@"
EOF
chmod 755 "$APPDIR/AppRun"

# ---------------------------------------------------------------- build
mkdir -p dist
OUT="dist/DIOSDownloader-$VERSION-x86_64.AppImage"
"$APPIMAGETOOL" "${RUNMODE[@]}" --no-appstream "$APPDIR" "$OUT"
rm -rf "$WORK"

echo "==> DONE: $OUT"
echo "    Run it directly, or integrate it with an AppImage launcher of your choice."
