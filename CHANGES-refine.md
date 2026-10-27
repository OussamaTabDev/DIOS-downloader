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

---------------------------------------------------------------------
Round 3 (on top of 1.1.3): hand-over of magnets/.torrent files, less idle work
---------------------------------------------------------------------

Found in 1.1.3 and kept as it is: everything from rounds 1-2 that 1.1.3 already had (yt-dlp execute-bit fix, software
renderer, always-on-top dialogs, Hide / tray on Linux, dynamic browser detection, shortcuts, registry autostart) and its
own, more complete torrent engine (librqbit 9, file windows, drag and drop, paste).

Fixed / added
- "Open with" while the program is running: a second start used to bring the window forward and DROP the magnet link or
  .torrent file. It is now forwarded to the running instance (POST /torrent on the local API) and opens "Add torrent".
- Linux: the .desktop file gets %U and MimeType (application/x-bittorrent, x-scheme-handler/magnet); the deb and rpm
  scripts no longer strip %U when they rewrite Exec=.
- Windows installer: optional task "Open .torrent files and magnet links with DIOS Downloader".

Performance
- A download's view no longer carries the login cookies (copied for every download 4 times a second);
  Manager::headers_of(id) gives them to the two places that need them (Export list, Refresh address).
- A torrent's file list is shared (Arc) instead of copied on every list refresh: big torrents have thousands of files.
- Manager::peek(|settings| ...) reads one setting without cloning all of them (the 100 ms timer cloned the whole
  Settings ten times a second just to read "speed limiter on").
- The list is not rebuilt while the main window is hidden to the tray or minimized.
- The 100 ms timer runs every third tick when nothing visible moves (no main window, no download/torrent window);
  the API poll is 100 ms instead of 50 ms (the API thread wakes the loop itself).

Checked without building: rustfmt parse of every .rs file, brace balance of every .slint file.
NOT verified: the project has not been compiled; Windows-only code and runtime behaviour were not run.
