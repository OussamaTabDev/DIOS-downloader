# DIOS Downloader – refinement notes

Performance
- Software renderer by default (windows open without creating a GPU context each). `DIOS_GPU=1` restores the GPU renderer.
- Windows: autostart is read/written through the registry API (no reg.exe on the UI thread).

Windows & tray
- All dialogs are always on top (native message boxes temporarily lift this). Download progress / complete windows are free.
- "Hide" button on the download window (like IDM), Ctrl+H / toolbar button on the main window.
- Linux tray (StatusNotifierItem via ksni). No tray host -> Hide minimizes, close quits.
- Tray click restores the main window and hidden download windows.

Browsers (Help > Add DIOS Downloader to my browser)
- Dynamic detection: PATH, Snap, Flatpak, /opt, .desktop web browsers, default browser first; Windows registry (StartMenuInternet, App Paths).
- "Other browser" card with manual steps. Extension folder also looked up in ~/.local/share/dios-downloader (install.sh fills it).

UI & shortcuts
- Menu shortcuts (Ctrl+N/B/J/Q/R/P/O, F2, F1, F9, Ctrl+K, Ctrl+, ...), list navigation (arrows, PgUp/PgDn, Home/End, Space), Esc closes dialogs.
- Responsive toolbar (icon-only < 1150 px, low-priority buttons drop out), category tree hides < 820 px, browser cards wrap.

Build note (Linux): needs libssl-dev (reqwest/native-tls).
Not verified: Windows-only code was not compiled; runtime behaviour (always-on-top, tray, Esc) was not run with a display.
