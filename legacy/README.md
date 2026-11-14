# Legacy Slint interface

This is the original DIOS Downloader 1.1.3 desktop interface, written with Slint.
It is kept for reference and as a fallback; the program in `crates/app` replaced it
with an htmx interface (a small local web server + a native web view window).

Build it on its own:

    cd legacy/slint-app
    cargo build --release

It uses the same engine (`crates/engine`) and the same settings files.
