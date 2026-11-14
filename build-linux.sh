#!/usr/bin/env bash
# Batiie — one-command Linux build + package
#
# Builds target/release/Batiie and packs
#   dist/Batiie-<version>-linux-x64.tar.gz
# containing the program, both browser extensions, a .desktop entry and
# an install helper. yt-dlp + ffmpeg are downloaded by the app itself
# on first start (from their GitHub releases, SHA-256 checked).
#
# The window is a web view: it needs WebKitGTK 4.1 (most desktops have it).
# Build:    sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libxdo-dev libssl-dev pkg-config
# Run:      sudo apt install libwebkit2gtk-4.1-0 libgtk-3-0 xdg-desktop-portal
# Fedora:   sudo dnf install webkit2gtk4.1 gtk3 xdg-desktop-portal   (build: webkit2gtk4.1-devel gtk3-devel openssl-devel)
# Without WebKitGTK the interface opens in your default browser instead.
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
pkill -f Batiie 2>/dev/null || true
bold "==> Building Batiie (first time: 5-15 minutes)"
cargo build --release
BIN="target/release/Batiie"
[ -f "$BIN" ] || fail "the build did not produce $BIN"

# ---------------------------------------------------------------- 3. package
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
[ -n "$VERSION" ] || VERSION="0.0.0"
bold "==> Packaging Batiie $VERSION for Linux x64"

DIST="dist"
STAGE="build-cache/stage-linux"
rm -rf "$STAGE"
mkdir -p "$STAGE/Batiie" "$DIST"

cp "$BIN" "$STAGE/Batiie/"
cp THIRD-PARTY-NOTICES.txt "$STAGE/Batiie/"
cp assets/batiie.desktop "$STAGE/Batiie/"
cp assets/batiie-logo.png "$STAGE/Batiie/batiie.png"

# browser extensions: Chrome/Edge (extension) + Firefox (own manifest)
cp -r extension "$STAGE/Batiie/extension"
rm -rf "$STAGE/Batiie/extension/test"
cp -r extension "$STAGE/Batiie/extension-firefox"
rm -rf "$STAGE/Batiie/extension-firefox/test"
cp "$STAGE/Batiie/extension-firefox/manifest.firefox.json" \
   "$STAGE/Batiie/extension-firefox/manifest.json"
rm -f "$STAGE/Batiie/extension-firefox/manifest.firefox.json"
rm -f "$STAGE/Batiie/extension/manifest.firefox.json"

# the signed Firefox add-on ships too when present
if ls extension-signed/dios-downloader-firefox-*.xpi >/dev/null 2>&1; then
    cp extension-signed/dios-downloader-firefox-*.xpi "$STAGE/Batiie/"
fi

cat > "$STAGE/Batiie/install.sh" <<'EOF'
#!/usr/bin/env bash
# Batiie — install helper for Linux
set -eu
cd "$(dirname "$0")"
PREFIX="${HOME}/.local"
mkdir -p "$PREFIX/bin" "$PREFIX/share/applications" "$PREFIX/share/icons/hicolor/256x256/apps"
install -m 755 Batiie "$PREFIX/bin/Batiie"
if [ -f batiie.png ]; then
    cp -f batiie.png "$PREFIX/share/icons/hicolor/256x256/apps/batiie.png"
fi
sed "s|Exec=Batiie|Exec=$PREFIX/bin/Batiie|" batiie.desktop \
    > "$PREFIX/share/applications/batiie.desktop"
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$PREFIX/share/applications" 2>/dev/null || true
fi
# the browser extensions: where the program looks for them (Help > Add Batiie
# Downloader to my browser), so the folder stays after this folder is deleted
DATA="${XDG_DATA_HOME:-$HOME/.local/share}/batiie"
mkdir -p "$DATA"
rm -rf "$DATA/extension" "$DATA/extension-firefox"
cp -r extension "$DATA/extension"
cp -r extension-firefox "$DATA/extension-firefox"
cp -f batiie-firefox*.xpi "$DATA/batiie-firefox.xpi" 2>/dev/null || true
echo "Installed. Start it from your applications menu or: $PREFIX/bin/Batiie"
echo "Browser extensions are in: $DATA"
EOF
chmod +x "$STAGE/Batiie/install.sh"

cat > "$STAGE/Batiie/README-linux.txt" <<'EOF'
Batiie for Linux (x64)
===============================

Quick start
  1. Run:  ./install.sh          (installs to ~/.local, no root needed)
  2. Start Batiie from your applications menu.

Or run it directly:  ./Batiie

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
    ~/.local/share/batiie/Batiie/data
EOF

TARBALL="$DIST/Batiie-$VERSION-linux-x64.tar.gz"
tar -czf "$TARBALL" -C "$STAGE" Batiie

# ------------------------------------------------- 4. native packages (.deb .rpm .AppImage)
# Best effort: each one is only built when its tool is present. Skip with
# SKIP_PACKAGES=1. Same staged folder feeds all three, see packaging/linux/.
if [ "${SKIP_PACKAGES:-0}" = "1" ]; then
    bold "==> Skipping native packages (SKIP_PACKAGES=1)"
else
    bold "==> Building native packages (deb / rpm / AppImage)"
    if command -v dpkg-deb >/dev/null 2>&1; then
        ./packaging/linux/make-deb.sh "$STAGE/Batiie" "$VERSION" || \
            printf '\033[33mNOTE: the .deb failed (see above); the tarball is still complete.\033[0m\n'
    else
        echo "NOTE: dpkg-deb not found — no .deb (Debian/Ubuntu: 'apt install dpkg' provides it)."
    fi
    if command -v rpmbuild >/dev/null 2>&1; then
        ./packaging/linux/make-rpm.sh "$STAGE/Batiie" "$VERSION" || \
            printf '\033[33mNOTE: the .rpm failed (see above); the tarball is still complete.\033[0m\n'
    else
        echo "NOTE: rpmbuild not found — no .rpm (Debian/Ubuntu: 'sudo apt install rpm')."
    fi
    ./packaging/linux/make-appimage.sh "$STAGE/Batiie" "$VERSION" || \
        printf '\033[33mNOTE: the AppImage failed (see above); the tarball is still complete.\033[0m\n'
fi

bold "==> DONE"
echo
echo "  >> $TARBALL"
ls -1 dist/Batiie-* dist/batiie-* 2>/dev/null | while read -r f; do echo "     $f"; done
echo
echo "  Install (tarball):  tar -xzf $(basename "$TARBALL") && ./Batiie/install.sh"
{ [ -f dist/batiie_"$VERSION"_amd64.deb ] && echo "  Install (.deb):     sudo apt install ./dist/batiie_${VERSION}_amd64.deb"; } || true
{ command -v rpm >/dev/null 2>&1 && rpm dist/batiie-"$VERSION"-1.x86_64.rpm >/dev/null 2>&1; } && echo "  Install (.rpm):     sudo dnf install ./dist/batiie-${VERSION}-1.x86_64.rpm" || true
{ [ -f "dist/Batiie-$VERSION-x86_64.AppImage" ] && echo "  Run (.AppImage):    chmod +x dist/Batiie-$VERSION-x86_64.AppImage && ./dist/Batiie-$VERSION-x86_64.AppImage"; } || true
