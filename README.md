<div align="center">

# DIOS Downloader

**A fast, honest download manager for Windows and Linux — written in Rust.**

Multi-connection downloads, browser integration, video/audio saving,
queues and scheduling — in one clean desktop app, in 8 languages.

[![Latest release](https://img.shields.io/badge/download-latest%20release-8A4FFF?style=flat-square&logo=github)](https://github.com/OussamaTabDev/DIOS-downloader/releases/latest)
![Windows](https://img.shields.io/badge/Windows-10%20%2C%2011-0078D6?style=flat-square&logo=windows11&logoColor=white)
![Linux](https://img.shields.io/badge/Linux-x64-FCC624?style=flat-square&logo=linux&logoColor=black)
![Rust](https://img.shields.io/badge/Made%20with-Rust-B7410E?style=flat-square&logo=rust&logoColor=white)
![License](https://img.shields.io/badge/license-MIT-green?style=flat-square)
![Languages](https://img.shields.io/badge/UI-8%20languages-success?style=flat-square)

**[Download](#-download--install)** · **[Features](#-features)** · **[Build from source](#-build-from-source)** · **[FAQ](#-faq--troubleshooting)** · **[Credits](#-origin--credits)**

</div>

---

## Table of contents

1. [About](#about)
2. [Origin & credits](#origin--credits)
3. [Features](#features)
4. [Download & install](#download--install)
5. [First steps](#first-steps)
6. [Browser add-on](#browser-add-on)
7. [Build from source](#build-from-source)
8. [Maintainer guide: builds & releases on GitHub](#maintainer-guide-builds--releases-on-github)
9. [How self-updates work](#how-self-updates-work)
10. [Behavior & privacy](#behavior--privacy)
11. [FAQ & troubleshooting](#faq--troubleshooting)
12. [Security](#security)
13. [Project structure](#project-structure)
14. [Changelog](#changelog)
15. [License](#license)
16. [Acknowledgements](#acknowledgements)

---

## About

**DIOS Downloader** is a desktop download manager written from scratch in **Rust** — a fast engine (`dios-engine`) plus a native **Slint** interface (`dios-app`). It downloads the files you point it at, splits each file across up to **32 connections**, and picks up exactly where it left off after an interruption, a pause, or even a full restart of the program.

It runs on **Windows 10/11** and on **64-bit Linux** (X11 and Wayland), saves video and audio from thousands of sites through **yt-dlp + FFmpeg**, integrates with **Chrome, Edge and Firefox** through a browser add-on, and speaks **eight languages**: العربية, English, Français, Español, Deutsch, Português, Türkçe, Русский.

It does **one job** — getting files onto your disk quickly and reliably — and does it without ads, accounts, tracking, or telemetry of any kind.

> [!IMPORTANT]
> You are responsible for respecting copyright and the terms of service of any content you download with the program. Please use it only with material you have the right to download.

---

## Origin & credits

> This project did not start here, and it is important to say so plainly.

**DIOS Downloader is a direct continuation of *Ezoo Download***, a download manager **originally created and developed by [ORIGINAL AUTHOR NAME → put the person's name here]** ([profile / original repository link → put the link here]).

- The original author designed, built and released **Ezoo Download versions 1.0.0 – 1.0.5** — the entire engine, the browser add-on, the update system and the initial interface. **All credit for the original work belongs to them.**
- This repository continues that work under a new name: at **v1.1.0** the project was renamed **DIOS Downloader**, received a redesigned interface, native Linux packages (`.deb` / `.rpm` / `.AppImage`), automated GitHub builds and releases, and a fresh release-signing key. The original codebase and its MIT license were carried over intact.
- The MIT license of the original project is preserved, including its original copyright notice — see [License](#license).
- **If you are the original author** and would like anything adjusted here — the credit wording, a link to your current project, or anything else — please open an issue. It will be handled promptly and in good faith.

Renaming an existing project without saying where it came from would be dishonest. This section exists so that anyone who finds DIOS Downloader knows exactly where it came from and whom to thank.

---

## Features

| | Feature | What it does |
|---|---|---|
| 🔀 | **Multi-connection downloads** | Every file is split across up to **32 connections** at once, with per-connection progress — big files finish dramatically faster |
| ⏯️ | **Pause & resume** | Pause any download and continue later — even after closing the program or rebooting |
| 🌐 | **Browser add-on** | Click a download in Chrome, Edge or Firefox and it is handed to DIOS Downloader, cookies included — links that need a login just work |
| 🎬 | **Video & audio** | Save video or audio from pages and whole **playlists** via **yt-dlp**, with **quality selection** (resolution, format), muxed together by **FFmpeg** |
| 📋 | **Clipboard & paste** | Copy a link anywhere and the program offers to download it; **Ctrl+V** fills the add dialog directly |
| 🖱️ | **Drag files out** | On Windows, drag a finished download straight from the list onto the desktop or into any folder |
| 🗂️ | **Auto-classify** | Files are filed automatically into the folder that matches their type (video, music, archives, programs, documents…) |
| ⏰ | **Queues & scheduler** | Line up many downloads as a queue, start the queue at a set time, and optionally **shut the PC down** when everything finishes |
| 🐢 | **Speed limits** | One global speed limit, or a separate limit per download |
| 🔎 | **Site grabber** | Point it at a page and collect **every file on it** in one pass, with size/type filters |
| 🗒️ | **Batch & lists** | Add many links at once; import and export download lists |
| 🖥️ | **Live progress window** | Compact status rows, per-connection bars, speeds and ETA at a glance |
| 🔔 | **Tray & autostart** | Minimizes to the tray icon, optional start with the session |
| 🔊 | **Completion sound** | A sound plays when downloads finish (optional) |
| 🌍 | **8 languages** | Full UI translation with RTL support for Arabic |
| 🔄 | **Signed self-updates** | The Windows build updates itself silently, verifying every update cryptographically before installing (see [How self-updates work](#how-self-updates-work)) |

<!-- Screenshots
## Screenshots

| Main window | Download info | Options |
|---|---|---|
| ![main](.github/assets/main.png) | ![info](.github/assets/info.png) | ![options](.github/assets/options.png) |
-->

---

## Download & install

All installers are published on the repository's **[Releases](https://github.com/OussamaTabDev/DIOS-downloader/releases/latest)** page — always the newest version, since only one release is kept at a time.

### Windows 10 / 11

1. Download **`DIOSDownloader-Setup.exe`** from the latest release.
2. Run it. The program installs **for your user only** — no administrator rights needed.
3. Start it from the Start menu.

> [!NOTE]
> The installer is not code-signed yet, so **Windows SmartScreen** may show a "unknown publisher" warning — click *More info → Run anyway*. This is expected for any new program without a certificate; see [Security](#security).

### Linux (x64, built for glibc 2.35+ — Ubuntu 22.04, Debian 12, Fedora 36 and newer)

| Package | Install | Remove |
|---|---|---|
| **`.deb`** (Ubuntu/Debian/Mint) | `sudo apt install ./dios-downloader_<version>_amd64.deb` | `sudo apt remove dios-downloader` |
| **`.rpm`** (Fedora/openSUSE) | `sudo dnf install ./dios-downloader-<version>-1.x86_64.rpm` | `sudo dnf remove dios-downloader` |
| **`.AppImage`** (any distro) | `chmod +x DIOSDownloader-<version>-x86_64.AppImage` then run it | delete the file |
| **`.tar.gz`** (portable, no root) | `tar -xzf DIOSDownloader-<version>-linux-x64.tar.gz && ./DIOSDownloader/install.sh` | `rm ~/.local/bin/DIOSDownloader` |

Details worth knowing:

- The `.deb` and `.rpm` install the program to **`/opt/DIOSDownloader`**, add a **`DIOSDownloader`** command to your `PATH`, a menu entry, and an icon — everything updates normally with your package manager.
- The **AppImage** runs anywhere without installation. It is built to run even without FUSE; if your system refuses to open it, either install `libfuse2` or run it with `./DIOSDownloader-….AppImage --appimage-extract-and-run`.
- The **tar.gz** contains an `install.sh` that installs to `~/.local` (menu entry + icon included) plus the browser extensions folder — no root required. You can also just run `./DIOSDownloader` straight from the extracted folder.
- On first use the program downloads **yt-dlp** and **FFmpeg** by itself from their official GitHub releases and verifies their **SHA-256** checksums; it keeps them current from then on. Nothing needs to be installed by hand.
- Standard desktop libraries are expected (libxkbcommon, libfontconfig, X11 or Wayland client libraries, xdg-desktop-portal for file dialogs). They are preinstalled on virtually every desktop distribution.

### System requirements

- **Windows**: Windows 10 or 11, 64-bit
- **Linux**: any 64-bit distribution with glibc ≥ 2.35 and a desktop session (X11 or Wayland)
- An internet connection 🙂

---

## First steps

1. **Add a link** — paste it with **Ctrl+V**, use the **Add** button, or just copy a link anywhere and let the program notice it.
2. **Pick the folder and quality** — the default folder is offered automatically; for videos, choose the resolution you want.
3. **Let it run** — progress appears in the main window with per-connection detail; finished files land in their type-matched folder.
4. **(Recommended) install the browser add-on** — from the program: **Help → Add DIOS Downloader to my browser** and follow the steps for your browser. Downloads started in the browser then arrive in the program automatically, with login cookies attached.
5. **Explore Options** — speed limits, queues, scheduler, auto-shutdown, language, clipboard monitoring, autostart, tray behavior.

---

## Browser add-on

The add-on (shipped for **Chrome/Edge** and **Firefox**) connects to the program over a local port on `127.0.0.1` (default **9614** — changeable in the settings). It is reachable **only from your machine**, and only from the add-on.

What it sends, and nothing more: the URL you clicked, the cookies and page URL of that specific download (so logged-in links work), and its own heartbeat. Everything stays on your machine — the program never uploads anything anywhere except the downloads you ask for.

Install it from inside the program (**Help → Add DIOS Downloader to my browser**):

- **Chrome / Edge**: load the unpacked `extension/` folder in `chrome://extensions` with developer mode on, or run the native-host installer the program offers.
- **Firefox**: install the bundled `.xpi` when present; on releases with Mozilla signing keys the add-on is **signed by Mozilla** and installs permanently, otherwise it loads as a temporary add-on via `about:debugging`.

---

## Build from source

The project is a Cargo workspace with two crates: `crates/engine` (the download engine, no UI) and `crates/app` (the Slint program).

### Prerequisites

- **Rust** (stable) — <https://rustup.rs>
- **Git** and a C toolchain (`build-essential` on Debian/Ubuntu, Xcode CLT / MSVC on Windows)
- Linux desktop dev libraries: `sudo apt install libxkbcommon-dev libfontconfig1-dev` (equivalents on other distros)
- **Windows only**: [Inno Setup 6](https://jrsoftware.org/isinfo.php) if you want to build the installer
- **Linux only**: `rpm` (`sudo apt install rpm`) if you want to build the `.rpm` on a Debian-based machine

### Linux — one command

```bash
./build-linux.sh
```

The script installs Rust if missing, runs `cargo build --release`, and produces **all four packages** in `dist/`:

```
dist/DIOSDownloader-<version>-linux-x64.tar.gz
dist/dios-downloader_<version>_amd64.deb
dist/dios-downloader-<version>-1.x86_64.rpm
dist/DIOSDownloader-<version>-x86_64.AppImage
```

The individual packagers live in `packaging/linux/` (`make-deb.sh`, `make-rpm.sh`, `make-appimage.sh`) and can also be run on their own after `cargo build --release`.

### Windows

```powershell
BUILD.bat                                        # quick build
powershell -ExecutionPolicy Bypass -File build-windows.ps1   # full build + installer
```

The setup appears as `dist/DIOSDownloader-Setup.exe`.

### Tests

```bash
cargo test -p dios-engine            # engine: downloads, HLS, resume, update verification…
node extension/test/yt-core.test.js  # browser add-on logic
node extension/test/background.sim.js
```

---

## Maintainer guide: builds & releases on GitHub

The workflow **`.github/workflows/build.yml`** does everything automatically, pinned to exact commit SHAs so a moved tag can never change a build.

**Jobs:** `windows` (tests, dependency audit, FFmpeg/yt-dlp gathering with checksum verification, Inno Setup installer, signed `version.json`, optional Mozilla add-on signing) → `linux` (Ubuntu 22.04 runner for glibc 2.35 compatibility; runs `./build-linux.sh` for all four packages) → `publish` (only for releases; gathers everything, creates the GitHub release, deletes older ones).

### 1. One-time setup — secrets

| Secret | Required | Purpose |
|---|---|---|
| `UPDATE_SIGNING_KEY` | **yes, for releases** | Ed25519 **seed, 64 hex chars**, whose public half is built into the program (`UPDATE_PUBLIC_KEY` in `crates/engine/src/selfupdate.rs`). The release build signs `version.json` with it. Without it a `[release]` push **fails on purpose** — installed copies would refuse an unsigned update. |
| `AMO_JWT_ISSUER` / `AMO_JWT_SECRET` | optional | Mozilla add-on API keys (addons.mozilla.org → Developer Hub → API keys). With them, the Firefox add-on is signed by Mozilla and installs permanently. |

```bash
gh secret set UPDATE_SIGNING_KEY --repo OussamaTabDev/DIOS-downloader
# paste the 64-hex seed, press Ctrl+D
gh secret list --repo OussamaTabDev/DIOS-downloader   # verify
```

> [!WARNING]
> The seed is the **private** half. Never commit it, never print it in issues — it lives only in the secret store. Losing it means rotating the key (see below).

**Rotating the signing key** (if it ever leaks, or on principle): generate a fresh Ed25519 keypair, replace `UPDATE_PUBLIC_KEY` in `crates/engine/src/selfupdate.rs`, release that version *without needing the old key*, then update the `UPDATE_SIGNING_KEY` secret to the new seed. Versions released with the old key can no longer be accepted once the new public key ships — so always ship the key change *first*, rotate the secret on the *next* release.

### 2. Everyday builds (no release)

Every push — and any manual run from the Actions tab (`workflow_dispatch`) — builds both platforms and keeps the results as artifacts:

```bash
git push
gh run list --limit 5                  # see recent runs
gh run watch                           # follow the newest run live
```

When the run is green, download **`DIOSDownloader-windows`** and **`DIOSDownloader-linux`** from the run's *Artifacts* section.

### 3. Publishing a release

A release is created **only** when a push to `main` carries **`[release]`** in the commit message:

```bash
# 1) bump the version once — every crate inherits it
sed -i 's/^version = ".*"/version = "1.2.0"/' Cargo.toml

# 2) commit and push; the word [release] is the trigger
git add -A
git commit -m "DIOS Downloader 1.2.0 [release]"
git push origin main
```

The `publish` job creates release **`v1.2.0`** with:

```
DIOSDownloader-Setup.exe                Windows installer
version.json                            signed update manifest
dios-downloader-firefox.xpi             (when AMO keys are set)
firefox-updates.json                    add-on self-update manifest
DIOSDownloader-<v>-linux-x64.tar.gz     Linux portable
dios-downloader_<v>_amd64.deb           Debian/Ubuntu
dios-downloader-<v>-1.x86_64.rpm        Fedora/openSUSE
DIOSDownloader-<v>-x86_64.AppImage      run-anywhere
SHA256SUMS                              checksums of every file
```

Check the result with `gh release view`. Notes:

- **Rolling latest**: only the newest release stays; older releases *and their tags* are deleted automatically, so `releases/latest` is always current.
- Re-releasing an existing version is skipped; pushing tag `vX.Y.Z` by hand is validated against `Cargo.toml` and fails on a mismatch.
- A commit that touches `.github/workflows/**` must be pushed with a token that has the **`workflow` scope** (`gh auth refresh -s workflow`), or GitHub rejects the push — see [the troubleshooting note](#faq--troubleshooting).

---

## How self-updates work

The Windows build keeps itself current, and every step is verified:

1. On start and every **6 hours**, the program reads `version.json` from the repository's latest release.
2. If that version is newer, it downloads the installer — **only from this repository**; any other URL is refused.
3. It verifies the installer's **SHA-256** against `version.json`.
4. It verifies `version.json`'s **Ed25519 signature** against the public key built into the program.
5. Only then does it run the installer silently and restart itself.

The signature is the core of the design: `version.json` is signed with a private key that exists **only** in the repository's secret store, while the matching public key is compiled into every installed copy. Even someone with full write access to the repository cannot push a malicious "update" — they could not produce a valid signature. Updates are mandatory (that is why they are verified so strictly); you can also check manually from **Help → Check for updates**.

The **Linux packages, tar.gz and AppImage do not self-update** — they are replaced the normal way: package-manager update, or downloading the new file. `version.json`'s `url`/`sha256` refer to the Windows installer.

---

## Behavior & privacy

For full transparency — here is everything the program does on its own:

| Function | Details |
|---|---|
| 🔄 **Self-update** | Reads `version.json` of the latest GitHub release on start and every 6 hours; installs only SHA-256 + Ed25519 verified installers from this repository |
| 🧰 **Helpers** | Downloads yt-dlp and FFmpeg on first use, refreshes them daily from their official GitHub releases, every file SHA-256 checked |
| 🔌 **Local port** | Listens on `127.0.0.1:9614` (changeable) for the browser add-on only; not reachable from the network |
| 🍪 **Cookies** | The add-on sends cookies only for the file you clicked, over the local connection; they are stored in the program's data folder in plain text |
| 📋 **Clipboard** | Watches for copied links and offers them; can be turned off in Options |
| 🚀 **Autostart** | Optional; `Run` registry value on Windows, an autostart `.desktop` entry on Linux |
| 💾 **Data location** | `%APPDATA%\DIOS\DIOSDownloader\data` (Windows) · `~/.local/share/dios/DIOSDownloader/data` (Linux) — settings, download lists, credentials, tools |
| 🤫 **Telemetry** | **None.** No analytics, no crash reporting, no statistics, no accounts, no requests to anything except the downloads you ask for and the update/tool sources above |

---

## FAQ & troubleshooting

**Q: Windows SmartScreen / antivirus warns about the installer.**
The installer is not code-signed yet. Click *More info → Run anyway*, or verify the file against `SHA256SUMS` on the release page. The entire source code is public for review.

**Q: `could not start yt-dlp: Permission denied (os error 13)`.**
The yt-dlp binary exists but cannot be executed. Usual causes and fixes:
- it lost its executable bit — restore it: `chmod +x ~/.local/share/dios/DIOSDownloader/data/tools/yt-dlp` (Linux);
- the bundled `tools` folder sits somewhere read-only or owned by root — the app automatically falls back to the data folder above and re-downloads there; deleting the stale `tools` folder forces a clean re-fetch;
- antivirus/EDR is blocking unsigned binaries — whitelist the file.

**Q: The AppImage does not open on my system.**
Your system lacks FUSE. Either install `libfuse2` (Debian/Ubuntu: `sudo apt install libfuse2`) or run it with `--appimage-extract-and-run`, which the shipped AppImage supports out of the box.

**Q: The browser add-on cannot connect.**
Make sure the program is running (check the tray). The add-on talks to `127.0.0.1:9614` — if you changed the port in settings, re-install the add-on from **Help → Add DIOS Downloader to my browser** so it picks up the new port. Corporate proxies/VPN tools that intercept localhost traffic can also break the link.

**Q: A download needs a login and fails in the program.**
Use the browser add-on: it forwards the site's cookies automatically. Or download while logged in from the browser and let the add-on catch it.

**Q: Where do finished files go?**
Each file lands in the sub-folder matching its type inside your download folder (video, music, archives…). The mapping and base folder are in Options.

**Q: How do I update on Linux?**
Install the new `.deb`/`.rpm` over the old one, replace the AppImage file, or re-run `install.sh` from a new tar.gz. Settings and download lists survive — they live in the data folder, not the install folder.

**Q: Something else?**
Open an [issue](https://github.com/OussamaTabDev/DIOS-downloader/issues) with your OS, the program version (Help → About) and what you did. Please never paste the `UPDATE_SIGNING_KEY` secret into an issue.

---

## Security

- **Reporting**: found a security problem? Open an issue (or contact the maintainer privately if the issue is sensitive). Please do not disclose publicly before there is a fix.
- **Design**: updates are pinned to this repository, checksummed (SHA-256) and signed (Ed25519); download tools are checksummed against their own projects' published sums; the local API port refuses anything that is not the add-on.
- **Scope**: the program downloads what you tell it to. It contains no telemetry and no hidden network activity — see [Behavior & privacy](#behavior--privacy).

---

## Project structure

```
├─ crates/
│  ├─ engine/             the engine: multi-connection downloads, resume, HLS,
│  │                      scheduling, grabber, yt-dlp integration, selfupdate
│  │                      (examples/sign_update.rs — the release signing tool)
│  └─ app/                the program (Slint UI)
│     ├─ ui/              window definitions
│     ├─ lang/            UI translations (8 languages)
│     └─ src/             windows logic, tray, add-on bridge, options
├─ extension/             browser add-on (Chrome/Edge + Firefox manifests, tests)
├─ assets/                logo, .desktop file
├─ packaging/linux/       make-deb.sh · make-rpm.sh · make-appimage.sh
├─ installer/             Inno Setup script for the Windows installer
├─ .github/workflows/     build.yml — full CI/CD: tests, builds, signed releases
├─ BUILD.bat              one-click Windows build
├─ build-windows.ps1      full Windows build + installer
└─ build-linux.sh         Linux build + all four packages
```

---

## Changelog

| Version | Date | Notes |
|---|---|---|
| **1.1.0** | 2026-10 | Renamed **Ezoo Download → DIOS Downloader** with a new logo; redesigned, IDM-style interface; Linux packaging (`.deb`/`.rpm`/`.AppImage`/tar.gz); automated GitHub builds & signed releases; fresh update-signing key |
| 1.0.5 | 2026-10 | *Ezoo Download* — original project, see the origin section above |
| 1.0.0 – 1.0.4 | 2026-10 | *Ezoo Download* — initial development by the original author |

---

## License

The project carries the **MIT license**, inherited unchanged from Ezoo Download:

- **Copyright (c) 2026 [ORIGINAL AUTHOR NAME]** — for Ezoo Download, the original project
- **Copyright (c) 2026 DIOS Downloader Team** — for the changes made since v1.1.0

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, subject to the following conditions: the above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED.

Open-source components the program ships with — full text in `THIRD-PARTY-NOTICES.txt`:

| Component | Role | License |
|---|---|---|
| [Slint](https://slint.dev) | User interface toolkit | Slint Royalty-free Desktop, Mobile & Web License 2.0 |
| [yt-dlp](https://github.com/yt-dlp/yt-dlp) | Video/audio extraction | The Unlicense (public domain) |
| [FFmpeg](https://github.com/FFmpeg/FFmpeg) | Muxing/transcoding | GPL-3.0 — unmodified build from [BtbN/FFmpeg-Builds](https://github.com/BtbN/FFmpeg-Builds); its license text ships next to the binary |
| Rust crates (tokio, reqwest, serde, ring, …) | Engine internals | MIT / Apache-2.0 |

---

## Acknowledgements

- **[ORIGINAL AUTHOR NAME]** — for creating Ezoo Download, the project this repository continues
- The **[yt-dlp](https://github.com/yt-dlp/yt-dlp)** team — the video-download backbone
- The **[FFmpeg](https://ffmpeg.org)** project and **[BtbN](https://github.com/BtbN/FFmpeg-Builds)** for the builds
- The **[Slint](https://slint.dev)** team for the UI toolkit
- The Rust community, whose crates do half the heavy lifting

<div align="center">

**DIOS Downloader Team** · © 2026 · [Download the latest release](https://github.com/OussamaTabDev/DIOS-downloader/releases/latest)

</div>
