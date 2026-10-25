#!/usr/bin/env bash
# DIOS Downloader — one-command Linux build + package
#
# Builds target/release/DIOSDownloader and packs
#   dist/DIOSDownloader-<version>-linux-x64.tar.gz
# containing the program, both browser extensions, a .desktop entry and
# an install helper. yt-dlp + ffmpeg are downloaded by the app itself
# on first start (from their GitHub releases, SHA-256 checked).
#
# Runtime libraries the program needs (almost every desktop has them):
#   libxkbcommon, libwayland-client (Wayland session), X11 client libs
#   (X11 session), fontconfig, and xdg-desktop-portal for file dialogs.
# Debian/Ubuntu:  sudo apt install libxkbcommon0 libwayland-client0 libfontconfig1 xdg-desktop-portal
# Fedora:         sudo dnf install libxkbcommon wayland fontconfig xdg-desktop-portal
set -euo pipefail
cd "$(dirname "$0")"

bold() { printf '\033[1m%s\033[0m\n' "$*"; }
fail() { printf '\033[31mERROR: %s\033[0m\n' "$*"; exit 1; }

# ---------------------------------------------------------------- 1. Rust
if ! command -v cargo >/dev/null 2>&1; then
    if [ -f "$HOME/.cargo/bin/cargo" ]; then
        export PATH="$HOME/.cargo/bin:$PATH"
    else
        bold "==> Installing Rust (rustup)"
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable --profile minimal
        export PATH="$HOME/.cargo/bin:$PATH"
    fi
fi
cargo --version
rustc --version

# ---------------------------------------------------------------- 2. build
# A running copy would lock the binary we replace
pkill -f DIOSDownloader 2>/dev/null || true
bold "==> Building DIOSDownloader (first time: 5-15 minutes)"
cargo build --release
BIN="target/release/DIOSDownloader"
[ -f "$BIN" ] || fail "the build did not produce $BIN"

# ---------------------------------------------------------------- 3. package
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
[ -n "$VERSION" ] || VERSION="0.0.0"
bold "==> Packaging DIOSDownloader $VERSION for Linux x64"

DIST="dist"
STAGE="build-cache/stage-linux"
rm -rf "$STAGE"
mkdir -p "$STAGE/DIOSDownloader" "$DIST"

cp "$BIN" "$STAGE/DIOSDownloader/"
cp THIRD-PARTY-NOTICES.txt "$STAGE/DIOSDownloader/"
cp assets/dios-downloader.desktop "$STAGE/DIOSDownloader/"
cp assets/dios-logo.png "$STAGE/DIOSDownloader/dios-downloader.png"

# browser extensions: Chrome/Edge (extension) + Firefox (own manifest)
cp -r extension "$STAGE/DIOSDownloader/extension"
rm -rf "$STAGE/DIOSDownloader/extension/test"
cp -r extension "$STAGE/DIOSDownloader/extension-firefox"
rm -rf "$STAGE/DIOSDownloader/extension-firefox/test"
cp "$STAGE/DIOSDownloader/extension-firefox/manifest.firefox.json" \
   "$STAGE/DIOSDownloader/extension-firefox/manifest.json"
rm -f "$STAGE/DIOSDownloader/extension-firefox/manifest.firefox.json"
rm -f "$STAGE/DIOSDownloader/extension/manifest.firefox.json"

# the signed Firefox add-on ships too when present
if ls extension-signed/dios-downloader-firefox-*.xpi >/dev/null 2>&1; then
    cp extension-signed/dios-downloader-firefox-*.xpi "$STAGE/DIOSDownloader/"
fi

cat > "$STAGE/DIOSDownloader/install.sh" <<'EOF'
#!/usr/bin/env bash
# DIOS Downloader — install helper for Linux
set -eu
cd "$(dirname "$0")"
PREFIX="${HOME}/.local"
mkdir -p "$PREFIX/bin" "$PREFIX/share/applications" "$PREFIX/share/icons/hicolor/256x256/apps"
install -m 755 DIOSDownloader "$PREFIX/bin/DIOSDownloader"
if [ -f dios-downloader.png ]; then
    cp -f dios-downloader.png "$PREFIX/share/icons/hicolor/256x256/apps/dios-downloader.png"
fi
sed "s|Exec=DIOSDownloader|Exec=$PREFIX/bin/DIOSDownloader|" dios-downloader.desktop \
    > "$PREFIX/share/applications/dios-downloader.desktop"
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$PREFIX/share/applications" 2>/dev/null || true
fi
echo "Installed. Start it from your applications menu or: $PREFIX/bin/DIOSDownloader"
echo "Browser extensions are in: $(cd .. >/dev/null && pwd)/DIOSDownloader/extension*"
EOF
chmod +x "$STAGE/DIOSDownloader/install.sh"

cat > "$STAGE/DIOSDownloader/README-linux.txt" <<'EOF'
DIOS Downloader for Linux (x64)
===============================

Quick start
  1. Run:  ./install.sh          (installs to ~/.local, no root needed)
  2. Start DIOS Downloader from your applications menu.

Or run it directly:  ./DIOSDownloader

Libraries the program needs (preinstalled on almost every desktop):
  libxkbcommon, libfontconfig, libwayland-client (Wayland) or
  X11 client libraries (X11), xdg-desktop-portal (file dialogs).
  Debian/Ubuntu: sudo apt install libxkbcommon0 libfontconfig1 \
                       libwayland-client0 xdg-desktop-portal

Browser extension
  Chrome/Edge : chrome://extensions > Developer mode > "Load unpacked" >
                choose the  extension/  folder.
  Firefox     : about:debugging#/runtime/this-firefox > "Load Temporary
                Add-on..." > pick manifest.json inside  extension-firefox/
                (or install the signed .xpi when one is shipped here).

Notes
  - yt-dlp and ffmpeg are fetched by the app itself on first use and
    kept up to date (SHA-256 verified).
  - Settings and download lists live in
    ~/.local/share/dios/DIOSDownloader/data
EOF

TARBALL="$DIST/DIOSDownloader-$VERSION-linux-x64.tar.gz"
tar -czf "$TARBALL" -C "$STAGE" DIOSDownloader

bold "==> DONE"
echo
echo "  >> $TARBALL"
echo
echo "  Install:  tar -xzf $(basename "$TARBALL") && ./DIOSDownloader/install.sh"
