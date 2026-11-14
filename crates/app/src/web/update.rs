//! New versions of Batiie from GitHub. After the program opens it looks for a
//! newer release; when there is one it is downloaded, checked (signature of the release
//! and SHA-256) and installed silently, and the program starts again by itself.
//! Help > Check for updates does the same at once, in a window of its own.
use super::actions::{done, modal};
use super::ui::*;
use super::Shared;
use crate::i18n::{t, tf};
use crate::state::{App, ShellCmd};
use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum::routing::{get as route_get, post};
use axum::Router;
use batiie_engine::{selfupdate, Manager, Status};
use maud::{html, Markup};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// one check / download at a time (automatic or from the menu)
static BUSY: AtomicBool = AtomicBool::new(false);

pub fn routes(r: Router<Shared>) -> Router<Shared> {
    r.route("/dlg/update", route_get(open_dialog))
        .route("/dlg/update/state", post(state))
        .route("/dlg/update-ytdlp", route_get(update_tools))
}

fn update_dir() -> PathBuf {
    batiie_engine::default_data_dir().join("update")
}

fn downloading(mgr: &Manager) -> bool {
    mgr.snapshot().iter().any(|v| matches!(v.status, Status::Connecting | Status::Downloading | Status::Assembling))
}

/// Start the installer and close the program (downloads resume next time).
fn apply(app: &App, setup: PathBuf, sha256: String, visible: bool) {
    match selfupdate::run_installer(&setup, &sha256, visible) {
        Ok(()) => app.shell(ShellCmd::Quit),
        Err(e) => app.alert(tf("Could not start the update:\n\n{}", &[&format!("{e:#}")])),
    }
}

/// The silent checks (installed copies only).
pub fn spawn(app: Arc<App>) {
    // leftovers of an update that is installed now
    if let Ok(rd) = std::fs::read_dir(update_dir()) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.contains(&format!("-{}.", selfupdate::current())) || name.ends_with(".download") {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    if !selfupdate::installed() {
        return; // a portable copy: there is no installation to update
    }
    let rt = app.rt.clone();
    rt.spawn(async move {
        // first check as soon as the program is open
        tokio::time::sleep(Duration::from_secs(3)).await;
        let mut first = true;
        loop {
            if !BUSY.swap(true, Ordering::SeqCst) {
                let started = round(&app, first).await;
                BUSY.store(false, Ordering::SeqCst);
                if started {
                    return;
                }
            }
            first = false;
            tokio::time::sleep(Duration::from_secs(6 * 3600)).await;
        }
    });
}

/// One check. true = the installer was started.
async fn round(app: &Arc<App>, at_start: bool) -> bool {
    let client = app.mgr.client();
    let Ok(Some(rel)) = selfupdate::check(&client).await else { return false };
    let Ok(setup) = selfupdate::download(&client, &rel, &update_dir()).await else { return false };
    // later checks never cut a running download short; at start the downloads simply go on
    if !at_start {
        while downloading(&app.mgr) {
            tokio::time::sleep(Duration::from_secs(30)).await;
        }
    }
    apply(app, setup, rel.sha256.clone(), false);
    true
}

/// What the window shows: written by the check (async side), read by the window.
#[derive(Clone, Default)]
pub struct Shown {
    state: i32,
    heading: String,
    detail: String,
    progress: f32,
    /// downloaded and checked: run this setup (path, sha256)
    install: Option<(PathBuf, String)>,
}

fn size_text(n: u64) -> String {
    format!("{:.1} MB", n as f64 / (1024.0 * 1024.0))
}

fn tell(shared: &Mutex<Shown>, state: i32, heading: String, detail: String, progress: f32) {
    let mut s = shared.lock().unwrap_or_else(|e| e.into_inner());
    s.state = state;
    s.heading = heading;
    s.detail = detail;
    s.progress = progress;
}

fn body(s: &Shown, live: bool) -> Markup {
    let (ico, class) = match s.state {
        1 => ("check", "ok"),
        4 => ("warn", "err"),
        _ => ("download", ""),
    };
    let busy = s.state == 0 || s.state == 2 || s.state == 3;
    html! {
        div id="up-body" class="dlg-body" hx-post=[live.then_some("/dlg/update/state")] hx-trigger=[(live && busy).then_some("every 300ms")] hx-swap="outerHTML" {
            div class="up-head" {
                span class={"up-ic " (class)} { (icon(ico)) }
                div { div class="big-name" style="white-space:normal" { (s.heading) } div class="muted sel-text" { (s.detail) } }
            }
            @if busy {
                div class="bar bigbar s1" { i style=(format!("width:{:.1}%", if s.progress >= 0.0 { s.progress * 100.0 } else { 100.0 })) class={@if s.progress < 0.0 { "indet" }} {} }
            }
        }
    }
}

async fn open_dialog(State(app): State<Shared>) -> Response {
    let shared = app.misc.lock().unwrap_or_else(|e| e.into_inner()).update.clone();
    if BUSY.swap(true, Ordering::SeqCst) {
        tell(&shared, 0, t("A new version is being prepared. It is installed by itself when it is ready."), String::new(), -1.0);
    } else {
        tell(&shared, 0, t("Checking for updates…"), tf("This is version {}.", &[&selfupdate::current()]), -1.0);
        shared.lock().unwrap_or_else(|e| e.into_inner()).install = None;
        let client = app.mgr.client();
        let sh = shared.clone();
        app.rt.spawn(async move {
            let finish = || BUSY.store(false, Ordering::SeqCst);
            let rel = match selfupdate::check(&client).await {
                Err(e) => {
                    tell(&sh, 4, t("Could not check for updates"), format!("{e:#}"), -1.0);
                    return finish();
                }
                Ok(None) => {
                    tell(&sh, 1, t("Batiie is up to date"), tf("Version {} is the newest version.", &[&selfupdate::current()]), -1.0);
                    return finish();
                }
                Ok(Some(rel)) => rel,
            };
            if !selfupdate::installed() {
                tell(&sh, 4, tf("Version {} is available", &[&rel.version]), t("This copy was not installed with the setup, so it cannot update itself."), -1.0);
                return finish();
            }
            let heading = tf("Downloading version {}…", &[&rel.version]);
            tell(&sh, 2, heading.clone(), String::new(), -1.0);
            let (s2, h2) = (sh.clone(), heading.clone());
            let progress = move |done: u64, total: Option<u64>| {
                let (detail, p) = match total {
                    Some(t) if t > 0 => (format!("{} / {}", size_text(done), size_text(t)), done as f32 / t as f32),
                    _ => (size_text(done), -1.0),
                };
                tell(&s2, 2, h2.clone(), detail, p);
            };
            match selfupdate::download_with_progress(&client, &rel, &update_dir(), &progress).await {
                Ok(setup) => {
                    tell(&sh, 3, tf("Installing version {}…", &[&rel.version]), t("Batiie closes now and opens again when it is installed."), -1.0);
                    // a moment to read it, then the setup runs in front of the user
                    tokio::time::sleep(Duration::from_millis(1200)).await;
                    sh.lock().unwrap_or_else(|e| e.into_inner()).install = Some((setup, rel.sha256.clone()));
                    finish();
                }
                Err(e) => {
                    tell(&sh, 4, t("Could not get the update"), format!("{e:#}"), -1.0);
                    finish();
                }
            }
        });
    }
    let s = shared.lock().unwrap_or_else(|e| e.into_inner()).clone();
    modal(dialog(
        "d-update",
        &t("Check for updates"),
        "small",
        None,
        html! {
            (body(&s, true))
            (footer(html! { span class="sp" {} (btn_close(&t("Close"))) }))
        },
    ))
}

async fn state(State(app): State<Shared>) -> Response {
    let shared = app.misc.lock().unwrap_or_else(|e| e.into_inner()).update.clone();
    let (s, install) = {
        let mut g = shared.lock().unwrap_or_else(|e| e.into_inner());
        (g.clone(), g.install.take())
    };
    if let Some((setup, sha)) = install {
        apply(&app, setup, sha, true);
    }
    body(&s, true).into_response()
}

/// Help > Check for updates (video downloader): yt-dlp and ffmpeg now.
async fn update_tools(State(app): State<Shared>) -> Response {
    let dir = batiie_engine::default_data_dir();
    // ffmpeg too, right now (removing the stamp makes it check)
    let _ = std::fs::remove_file(dir.join("ffmpeg-checked"));
    app.rt.spawn(batiie_engine::tools::update_all(app.mgr.client(), dir));
    let bus = app.bus.clone();
    std::thread::spawn(move || {
        let text = match batiie_engine::ytdlp::update_now() {
            Ok(out) => tf("Batiie {}\n\nThe video downloader (yt-dlp) is up to date.\n\n{}", &[&env!("CARGO_PKG_VERSION"), &out]),
            Err(e) => tf("Could not update yt-dlp:\n\n{}", &[&format!("{e:#}")]),
        };
        let _ = bus.send(crate::state::Push::Alert(text));
    });
    app.toast(t("Checking the video downloader…"));
    done(&app)
}
