# The htmx interface (from 1.1.3 "refined")

The whole interface was rebuilt. The engine (`crates/engine`), the browser add-ons and the
add-on API (`api.rs`) did not change; the settings files are the same, so an existing
installation keeps its downloads, queues, categories and options.

## How it works now

* `Batiie` starts a small web server (axum) on `127.0.0.1` and shows it in a native
  window: WebView2 on Windows, WebKitGTK on Linux. If the web view can't start, the interface
  opens in the default browser (`BATIIE_BROWSER=1` forces that).
* Every window is a server-rendered HTML fragment (maud) that htmx puts on the page; forms and
  buttons are htmx requests. The list, sidebar, toolbar, status bar and the progress windows
  are kept up to date by **one server-sent-events stream**: the server compares what each page
  already has and sends only the rows that changed (idiomorph keeps hover, focus and
  transitions).
* Two small files of script: htmx (+ SSE and idiomorph extensions) and `app.js` (dialogs, menus,
  keyboard, column resizing, drag and drop). No `eval`; the page runs under a strict
  Content-Security-Policy.
* The 8 languages come from the existing `.po` / `.json` files (read at run time, Arabic is
  right-to-left).

## Windows of their own

Every dialog of the program is a real top-level window now, not a panel inside the main
window. They can sit on other workspaces or monitors, the main window can be minimized or
sent to the tray without taking them along, and each can be resized (with a minimum size).

* **Download File Info**, **Download complete**, the shutdown countdown, the duplicate-link
  question and plain messages open **under the mouse pointer** (on the screen it is on) and
  stay **above other windows**, so a new download is never missed.
* The **progress window** is a window of its own too, with a live speed graph, size / time /
  resume cards and a connections bar. Torrents get the same window with their file list.
* The other dialogs (Options, Scheduler, Properties, Batch, Torrent, Grabber, Playlist …) open in
  the middle of the screen, sized for their content. Short yes/no questions stay inside the
  window they come from.
* The main window remembers its size, position and maximized state.
* Implementation: a dialog is still written once; when the native window layer is running, the
  answer of a dialog route is shown by a window of its own (`/w/dlg/<id>`). Windows are made
  when the program starts and reused (WebKitGTK does not paint web views created later).
  Without the native layer (browser mode) dialogs stay in-page.
* Wayland compositors ignore requested window positions and "always on top"; there the windows
  open where the compositor puts them.

## Other improvements

Quick action buttons on the hovered list line (pause / resume / open / folder), a small speed
graph in the status bar, a real empty state (Add URL / Paste), keyboard-shortcut sheet (F1),
"Copy file name" / "Copy full path", a toast when a download fails, category add / edit buttons in
the sidebar, an idle list that costs almost nothing (rows are fingerprinted, the live stream
slows down when nothing changes).

## Safety of the local server

It controls a program that opens files and deletes entries, so: it only listens on loopback; a
random secret per run (given to the window in its first address, kept in a `HttpOnly`,
`SameSite=Strict` cookie); every request checks the `Host` header (DNS-rebinding) and every
change needs the custom `X-Batiie` header plus a matching `Origin`; links the interface opens
must be `http(s)` web addresses; replies carry `Content-Security-Policy`, `nosniff` and
`no-referrer`.

## Windows of the program

Main window (menus, toolbar, categories, queues, list with sortable / resizable / movable
columns, status bar, keyboard, context menu, drag and drop of links, text and `.torrent` files),
Add download (checks the address, starts while open, duplicates), Add batch (list or pattern),
progress window (live, segments, speed limit, when-done), Download complete, shutdown countdown,
Properties, Columns, Find, Category, Scheduler / queues, Options (10 tabs, live theme preview),
Torrent (add + live window with files), Site grabber / "Download all links", Playlist, Browser
add-in setup, Updates, Tip of the day, About, Refresh download address, Export / Import.

## Known differences from the Slint version

* The *Window opacity* option has no effect (a web view can't make its window translucent);
  the stored value is kept.
* The progress windows are floating panels inside the main window, not separate OS windows.
* Dialogs show the texts that exist in the translation files; a few new texts fall back to
  English until they are translated.
* The Linux AppImage needs WebKitGTK 4.1 on the host (it is not bundled).
