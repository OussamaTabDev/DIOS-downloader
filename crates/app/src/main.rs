// Batiie — desktop app.
// No console window in release builds on Windows.
#![cfg_attr(not(feature = "window"), allow(dead_code))]
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod api;
mod autostart;
mod browsers;
mod clipboard;
mod i18n;
mod shell;
mod sound;
mod state;
mod tray;
mod web;

use batiie_engine::{default_data_dir, Event, Manager, Settings};
use state::{App, Push};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn main() -> anyhow::Result<()> {
    // started by Windows at sign-in: stay next to the clock, no window
    let background = std::env::args().any(|a| a == "--background");
    let data_dir = std::env::var_os("BATIIE_DATA_DIR").or_else(|| std::env::var_os("DIOS_DATA_DIR")).map(std::path::PathBuf::from).unwrap_or_else(default_data_dir);
    let port = Settings::load(&data_dir.join("settings.json")).browser_port;

    // `Batiie magnet:?...` / `Batiie file.torrent` (what the system runs for
    // these links and files)
    let torrent_arg = std::env::args().skip(1).map(|a| a.trim().to_string()).find(|a| web::actions::torrent_source_of(a).is_some());

    // One instance only: if the port is taken by us, bring that one to front.
    let server = match api::bind(port) {
        Ok(s) => Some(s),
        Err(_) => {
            if api::notify_existing(port, torrent_arg.as_deref()) {
                return Ok(());
            }
            None
        }
    };

    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(4).enable_all().build()?;
    let mgr = Manager::new(rt.handle().clone(), data_dir.clone())?;
    // yt-dlp and ffmpeg: put back if missing, update silently once a day
    batiie_engine::tools::spawn_updater(rt.handle(), mgr.client(), data_dir.clone());

    // the language of the program
    i18n::apply(i18n::effective(&mgr.settings().language));

    // an installation of the old name ("DIOS Downloader") keeps its start-with-Windows entry
    autostart::migrate_old_name();

    // first run: start with Windows, like other download managers
    {
        let mut s = mgr.settings();
        if !s.autostart_set {
            // the installer already asked; a copy run without it starts with Windows
            if !autostart::installed_by_setup() {
                autostart::set(true);
            }
            s.autostart_set = true;
            let _ = mgr.set_settings(s);
        }
    }

    let tray = tray::Tray::new();
    let has_tray = tray.is_some();

    // ---------------------------------------------------------------- the interface server
    let (shell_tx, shell_rx) = std::sync::mpsc::channel();
    let (bus, _) = tokio::sync::broadcast::channel::<Push>(64);
    let token: String = {
        use rand::Rng;
        let mut r = rand::thread_rng();
        (0..32).map(|_| format!("{:x}", r.gen_range(0..16))).collect()
    };
    let app = Arc::new(App {
        mgr: mgr.clone(),
        rt: rt.handle().clone(),
        token,
        port: std::sync::atomic::AtomicU16::new(0),
        ui: Mutex::new(Default::default()),
        bus,
        shell: Mutex::new(shell_tx),
        add: Mutex::new(Default::default()),
        misc: Mutex::new(Default::default()),
        clip_last: Mutex::new(String::new()),
        started: std::time::Instant::now(),
        windowed: std::sync::atomic::AtomicBool::new(false),
        wins: Mutex::new(Default::default()),
        speed_hist: Mutex::new(Default::default()),
        total_hist: Mutex::new(Default::default()),
        win_seq: std::sync::atomic::AtomicU64::new(0),
        wake: tokio::sync::Notify::new(),
        me: Mutex::new(std::sync::Weak::new()),
        pending: Mutex::new(Vec::new()),
    });
    *app.me.lock().unwrap() = Arc::downgrade(&app);
    // test hook: dialogs are kept as window pages (but no window opens)
    if std::env::var("BATIIE_FAKE_WINDOWED").map(|v| v == "1").unwrap_or(false) {
        app.windowed.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    let listener = std::net::TcpListener::bind(("127.0.0.1", std::env::var("BATIIE_UI_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(0)))?;
    listener.set_nonblocking(true)?;
    app.port.store(listener.local_addr()?.port(), std::sync::atomic::Ordering::Relaxed);
    {
        let router = web::router(app.clone());
        rt.spawn(async move {
            if let Ok(l) = tokio::net::TcpListener::from_std(listener) {
                let _ = axum::serve(l, router).await;
            }
        });
    }
    if std::env::var_os("BATIIE_PRINT_URL").is_some() {
        println!("http://127.0.0.1:{}/?k={}", app.port.load(std::sync::atomic::Ordering::Relaxed), app.token);
    }

    // ---------------------------------------------------------------- browser add-in + clipboard
    if let Some(server) = server {
        let cfg_mgr = mgr.clone();
        let app2 = app.clone();
        api::serve(server, move || api::config_json(&cfg_mgr.settings()), move |event| web::actions::handle_api(&app2, event));
    }
    {
        let (enabled, app2) = (mgr.clone(), app.clone());
        clipboard::spawn(move || enabled.settings().clipboard_monitor, move |pickup| web::actions::clipboard_pickup(&app2, pickup));
    }

    // ---------------------------------------------------------------- the clock of the program
    web::misc::spawn_ticker(app.clone());
    web::update::spawn(app.clone());

    // opened with a .torrent file or a magnet link: the "Add torrent" window comes up
    // (not when the app starts quietly next to the clock)
    let quiet = background && has_tray;
    if !quiet {
        if let Some(source) = torrent_arg.as_deref().and_then(web::actions::torrent_source_of) {
            web::misc::open_torrent_add(&app, Some(source));
        }
        web::misc::first_run_dialogs(&app);
    }

    shell::run(app.clone(), shell_rx, tray, quiet);

    // save everything and stop downloads cleanly (they resume next time)
    web::add::cancel_early(&app);
    mgr.shutdown();
    rt.shutdown_timeout(Duration::from_secs(2));
    let _ = (Path::new(""), Event::QuotaReached);
    Ok(())
}
