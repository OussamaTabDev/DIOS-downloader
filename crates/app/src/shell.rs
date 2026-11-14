//! The window around the interface. With the `window` feature the page is shown
//! in a native web view (WebView2 on Windows, WebKitGTK on Linux); without it,
//! or when the web view can't start, the page opens in the default browser.
use crate::state::{App, ShellCmd};
use crate::tray;
use std::sync::mpsc::Receiver;
use std::sync::Arc;

/// Runs until the program quits. Must be called on the main thread.
pub fn run(app: Arc<App>, rx: Receiver<ShellCmd>, tray: Option<tray::Tray>, start_hidden: bool) {
    #[cfg(feature = "window")]
    {
        if std::env::var_os("BATIIE_BROWSER").is_none() {
            match window::run(app.clone(), rx, tray, start_hidden) {
                Ok(()) => return,
                Err((rx, tray)) => {
                    eprintln!("the native window could not start, opening the browser instead");
                    return browser_loop(app, rx, tray, start_hidden);
                }
            }
        }
    }
    browser_loop(app, rx, tray, start_hidden)
}

fn url(app: &App) -> String {
    format!(
        "http://127.0.0.1:{}/?k={}",
        app.port.load(std::sync::atomic::Ordering::Relaxed),
        app.token
    )
}

/// No native window: the page lives in the browser, the tray (if any) still works.
fn browser_loop(app: Arc<App>, rx: Receiver<ShellCmd>, tray: Option<tray::Tray>, start_hidden: bool) {
    use std::time::Duration;
    if !start_hidden {
        let _ = open::that(url(&app));
    }
    let mut limiter = false;
    loop {
        match rx.recv_timeout(Duration::from_millis(150)) {
            Ok(ShellCmd::Show) => {
                let _ = open::that(url(&app));
            }
            // no native windows here: pages that wanted one are opened in the browser
            Ok(ShellCmd::Open(spec)) if !spec.path.starts_with("/w/dlg/") => {
                let _ = open::that(crate::web::windows::url(&app, &spec.path));
            }
            Ok(ShellCmd::Quit) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return,
            Ok(_) => {}
            Err(_) => {}
        }
        if let Some(t) = &tray {
            for a in t.poll() {
                if crate::web::tray_action(&app, a) {
                    return;
                }
            }
        }
        let on = app.mgr.peek(|s| s.speed_limit_on);
        if on != limiter {
            limiter = on;
            if let Some(t) = &tray {
                t.set_limiter(on);
            }
        }
    }
}

#[cfg(feature = "window")]
mod window {
    use super::*;
    use crate::state::WinSpec;
    use std::sync::Mutex;
    use std::time::{Duration, Instant};
    use tao::dpi::{LogicalSize, PhysicalPosition};
    use tao::event::{Event, WindowEvent};
    use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy, EventLoopWindowTarget};
    use tao::window::{Icon, Window, WindowBuilder, WindowId};
    use wry::{WebContext, WebView, WebViewBuilder};

    type Back = (Receiver<ShellCmd>, Option<tray::Tray>);

    /// How many windows of their own are made when the program starts. WebKitGTK does not
    /// paint a web view that is made after the event loop has started (it stays black), so the
    /// windows are made first, kept hidden, and reused for every dialog.
    const POOL: usize = 8;

    /// A window of its own (hidden while it is free) with its web view.
    struct Slot {
        window: Window,
        webview: WebView,
        /// which dialog shows in it now (the page's messages carry it)
        label: Arc<Mutex<Option<String>>>,
    }

    impl Slot {
        fn current(&self) -> Option<String> {
            self.label.lock().ok().and_then(|l| l.clone())
        }
    }

    #[derive(serde::Serialize, serde::Deserialize, Clone, Default)]
    struct Geometry {
        x: i32,
        y: i32,
        w: f64,
        h: f64,
        maximized: bool,
    }

    fn geometry_file() -> std::path::PathBuf {
        std::env::var_os("BATIIE_DATA_DIR").or_else(|| std::env::var_os("DIOS_DATA_DIR")).map(std::path::PathBuf::from).unwrap_or_else(batiie_engine::default_data_dir).join("window.json")
    }

    fn load_geometry() -> Option<Geometry> {
        let g: Geometry = serde_json::from_slice(&std::fs::read(geometry_file()).ok()?).ok()?;
        (g.w >= 400.0 && g.h >= 300.0 && g.w < 10_000.0 && g.h < 10_000.0).then_some(g)
    }

    pub fn run(app: Arc<App>, rx: Receiver<ShellCmd>, tray: Option<tray::Tray>, start_hidden: bool) -> Result<(), Back> {
        // the tao event loop wants user events; our channel is forwarded into it
        let event_loop = EventLoopBuilder::<ShellCmd>::with_user_event().build();
        let proxy = event_loop.create_proxy();
        let saved = load_geometry();
        let mut builder = WindowBuilder::new()
            .with_title("Batiie")
            .with_inner_size(LogicalSize::new(saved.as_ref().map(|g| g.w).unwrap_or(1180.0), saved.as_ref().map(|g| g.h).unwrap_or(740.0)))
            .with_min_inner_size(LogicalSize::new(760.0, 480.0))
            .with_window_icon(icon())
            .with_visible(!start_hidden);
        if let Some(g) = &saved {
            // only where a screen still is (a monitor may have been unplugged)
            let on_screen = event_loop.available_monitors().any(|m| {
                let (p, s) = (m.position(), m.size());
                g.x + 60 >= p.x && g.x + 60 < p.x + s.width as i32 && g.y + 30 >= p.y && g.y + 30 < p.y + s.height as i32
            });
            if on_screen {
                builder = builder.with_position(PhysicalPosition::new(g.x, g.y));
            }
            builder = builder.with_maximized(g.maximized);
        }
        let window = match builder.build(&event_loop) {
            Ok(w) => w,
            Err(_) => return Err((rx, tray)),
        };
        let main_id = window.id();
        // one web context for every window: one web process, one cookie jar, one cache
        let mut ctx = WebContext::default();
        let main_view = {
            let b = WebViewBuilder::new_with_web_context(&mut ctx).with_url(url(&app)).with_new_window_req_handler(|u| {
                // links to other sites go to the real browser, never into this window
                if u.starts_with("http://") || u.starts_with("https://") {
                    let _ = open::that(u);
                }
                false
            });
            match build_main_view(b, &window) {
                Ok(w) => w,
                Err(_) => return Err((rx, tray)),
            }
        };
        app.windowed.store(true, std::sync::atomic::Ordering::Relaxed);
        // the windows of the dialogs, ready and hidden
        let mut slots: Vec<Slot> = (0..POOL).filter_map(|_| make_slot(&event_loop, &mut ctx, &proxy)).collect();

        // forward the commands of the program (other threads) into the loop
        {
            let proxy = proxy.clone();
            std::thread::spawn(move || {
                while let Ok(c) = rx.recv() {
                    if proxy.send_event(c).is_err() {
                        break;
                    }
                }
            });
        }

        let can_hide = tray.is_some();
        let mut limiter = false;
        let mut geom = saved.unwrap_or_default();
        let mut dirty: Option<Instant> = None;
        let _keep_main = main_view;
        event_loop.run(move |event, target, flow| {
            *flow = ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(120));
            match event {
                Event::UserEvent(ShellCmd::Show) => {
                    window.set_visible(true);
                    window.set_minimized(false);
                    window.set_focus();
                }
                Event::UserEvent(ShellCmd::Hide) => window.set_visible(false),
                Event::UserEvent(ShellCmd::Title(t)) => window.set_title(&t),
                Event::UserEvent(ShellCmd::Quit) => *flow = ControlFlow::Exit,
                Event::UserEvent(ShellCmd::Open(spec)) => {
                    // already open: it comes to the front
                    if let Some(s) = slots.iter().find(|s| s.current().as_deref() == Some(spec.label.as_str())) {
                        s.window.set_visible(true);
                        s.window.set_minimized(false);
                        s.window.set_focus();
                    } else {
                        let free = slots.iter().position(|s| s.current().is_none());
                        let idx = match free {
                            Some(i) => Some(i),
                            // more dialogs than ready windows: one more is made now
                            None => make_slot(target, &mut ctx, &proxy).map(|s| {
                                slots.push(s);
                                slots.len() - 1
                            }),
                        };
                        if let Some(i) = idx {
                            show_in(&slots[i], &app, target, &spec);
                        }
                    }
                }
                Event::UserEvent(ShellCmd::Close(label)) | Event::UserEvent(ShellCmd::ForceClose(label)) => {
                    release(&slots, &app, &label);
                }
                Event::UserEvent(ShellCmd::Ipc(label, msg)) => {
                    if msg == "close" {
                        release(&slots, &app, &label);
                    } else if let Some(t) = msg.strip_prefix("title:") {
                        if let Some(s) = slots.iter().find(|s| s.current().as_deref() == Some(label.as_str())) {
                            s.window.set_title(t);
                        }
                    }
                }
                Event::WindowEvent { window_id, event: WindowEvent::CloseRequested, .. } if window_id != main_id => {
                    // the system's close button: the page is asked first (a new download is
                    // cancelled, a running one is stopped); if it does not answer the window
                    // goes anyway
                    if let Some(s) = slots.iter().find(|s| s.window.id() == window_id) {
                        if let Some(label) = s.current() {
                            let _ = s.webview.evaluate_script("window.batiieBeforeClose ? window.batiieBeforeClose() : window.ipc.postMessage('close')");
                            let p = proxy.clone();
                            std::thread::spawn(move || {
                                std::thread::sleep(Duration::from_millis(2500));
                                let _ = p.send_event(ShellCmd::ForceClose(label));
                            });
                        } else {
                            s.window.set_visible(false);
                        }
                    }
                }
                Event::WindowEvent { window_id, event: WindowEvent::CloseRequested, .. } if window_id == main_id => {
                    // the red cross sends the program to the tray when there is one
                    if can_hide && app.mgr.peek(|s| s.close_to_tray) {
                        window.set_visible(false);
                        app.ui().hidden = true;
                    } else {
                        save_geometry(&window, &geom);
                        *flow = ControlFlow::Exit;
                    }
                }
                Event::WindowEvent { window_id, event: WindowEvent::Resized(size), .. } if window_id == main_id => {
                    if !window.is_maximized() && !window.is_minimized() && size.width > 0 {
                        let l = size.to_logical::<f64>(window.scale_factor());
                        geom.w = l.width;
                        geom.h = l.height;
                        dirty = Some(Instant::now());
                    }
                }
                Event::WindowEvent { window_id, event: WindowEvent::Moved(pos), .. } if window_id == main_id => {
                    if !window.is_maximized() && !window.is_minimized() {
                        geom.x = pos.x;
                        geom.y = pos.y;
                        dirty = Some(Instant::now());
                    }
                }
                Event::MainEventsCleared => {
                    if let Some(t) = &tray {
                        for a in t.poll() {
                            if crate::web::tray_action(&app, a) {
                                *flow = ControlFlow::Exit;
                            }
                        }
                        let on = app.mgr.peek(|s| s.speed_limit_on);
                        if on != limiter {
                            limiter = on;
                            t.set_limiter(on);
                        }
                    }
                    if dirty.map(|d| d.elapsed() > Duration::from_millis(700)).unwrap_or(false) {
                        dirty = None;
                        save_geometry(&window, &geom);
                    }
                }
                Event::LoopDestroyed => save_geometry(&window, &geom),
                _ => {}
            }
        });
    }

    fn save_geometry(window: &Window, g: &Geometry) {
        let mut g = g.clone();
        g.maximized = window.is_maximized();
        if g.w >= 400.0 {
            let _ = std::fs::write(geometry_file(), serde_json::to_vec(&g).unwrap_or_default());
        }
    }

    fn build_main_view(builder: WebViewBuilder, window: &Window) -> wry::Result<WebView> {
        #[cfg(target_os = "linux")]
        {
            use tao::platform::unix::WindowExtUnix;
            use wry::WebViewBuilderExtUnix;
            let vbox = window.default_vbox().expect("gtk box");
            builder.build_gtk(vbox)
        }
        #[cfg(not(target_os = "linux"))]
        {
            builder.build(window)
        }
    }

    /// A hidden window with an empty page: ready to show a dialog.
    fn make_slot(target: &EventLoopWindowTarget<ShellCmd>, ctx: &mut WebContext, proxy: &EventLoopProxy<ShellCmd>) -> Option<Slot> {
        let window = WindowBuilder::new()
            .with_title("Batiie")
            .with_inner_size(LogicalSize::new(640.0, 600.0))
            .with_window_icon(icon())
            .with_visible(false)
            .build(target)
            .ok()?;
        let label: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let (p, l) = (proxy.clone(), label.clone());
        let b = WebViewBuilder::new_with_web_context(ctx)
            .with_url("about:blank")
            .with_ipc_handler(move |req| {
                if let Some(label) = l.lock().ok().and_then(|l| l.clone()) {
                    let _ = p.send_event(ShellCmd::Ipc(label, req.body().to_string()));
                }
            })
            .with_new_window_req_handler(|u| {
                if u.starts_with("http://") || u.starts_with("https://") {
                    let _ = open::that(u);
                }
                false
            });
        let webview = build_main_view(b, &window).ok()?;
        Some(Slot { window, webview, label })
    }

    /// Show a dialog in a ready window: its size, where it opens, above others or not.
    fn show_in(slot: &Slot, app: &Arc<App>, target: &EventLoopWindowTarget<ShellCmd>, spec: &WinSpec) {
        if let Ok(mut l) = slot.label.lock() {
            *l = Some(spec.label.clone());
        }
        let w = &slot.window;
        w.set_title(&spec.title);
        w.set_min_inner_size(Some(LogicalSize::new(spec.min_w, spec.min_h)));
        w.set_always_on_top(spec.on_top);
        match placement(target, spec) {
            Some((lw, lh, x, y)) => {
                w.set_inner_size(LogicalSize::new(lw, lh));
                w.set_outer_position(PhysicalPosition::new(x, y));
            }
            None => w.set_inner_size(LogicalSize::new(spec.w, spec.h)),
        }
        let _ = slot.webview.load_url(&crate::web::windows::url(app, &spec.path));
        w.set_visible(true);
        w.set_minimized(false);
        // some window managers place a window that was shown before by their own rules: the
        // position is asked again once it is on the screen
        if let Some((_, _, x, y)) = placement(target, spec) {
            w.set_outer_position(PhysicalPosition::new(x, y));
        }
        w.set_focus();
    }

    /// A dialog is done: its window is hidden and kept for the next one.
    fn release(slots: &[Slot], app: &Arc<App>, label: &str) {
        if let Some(s) = slots.iter().find(|s| s.current().as_deref() == Some(label)) {
            if let Ok(mut l) = s.label.lock() {
                *l = None;
            }
            s.window.set_visible(false);
            s.window.set_always_on_top(false);
            let _ = s.webview.load_url("about:blank");
        }
        crate::web::windows::forget(app, label);
    }

    /// Where a window opens (physical pixels): under the pointer (questions) or in the middle
    /// of the screen the pointer is on; never bigger than that screen.
    fn placement(target: &EventLoopWindowTarget<ShellCmd>, spec: &WinSpec) -> Option<(f64, f64, i32, i32)> {
        let cursor = target.cursor_position().ok();
        let monitor = cursor.and_then(|c| target.monitor_from_point(c.x, c.y)).or_else(|| target.primary_monitor())?;
        let scale = monitor.scale_factor();
        let (mp, ms) = (monitor.position(), monitor.size());
        let (pw, ph) = ((spec.w * scale).min(ms.width as f64 * 0.96), (spec.h * scale).min(ms.height as f64 * 0.94));
        let (cx, cy) = match (spec.at_cursor, cursor) {
            (true, Some(c)) => (c.x, c.y - ph * 0.25),
            _ => (mp.x as f64 + ms.width as f64 / 2.0, mp.y as f64 + ms.height as f64 / 2.0 - ph * 0.04),
        };
        let x = (cx - pw / 2.0).clamp(mp.x as f64, (mp.x as f64 + ms.width as f64 - pw).max(mp.x as f64));
        let y = (cy - ph / 2.0).clamp(mp.y as f64, (mp.y as f64 + ms.height as f64 - ph).max(mp.y as f64));
        Some((pw / scale, ph / scale, x as i32, y as i32))
    }

    fn icon() -> Option<Icon> {
        let img = include_bytes!("../assets/img/icon-window.rgba");
        Icon::from_rgba(img.to_vec(), 64, 64).ok()
    }

    #[allow(dead_code)]
    fn _unused(_: WindowId) {}
}
