//! "Download a playlist": every video of a playlist in one go, in the quality you
//! choose, into its own queue and folder.
use super::actions::{flag, get, modal, Params};
use super::ui::*;
use super::Shared;
use crate::i18n::{t, tf};
use crate::state::App;
use axum::extract::{Form, Query, State};
use axum::response::{IntoResponse, Response};
use axum::routing::{get as route_get, post};
use axum::Router;
use batiie_engine::{filename, NewDownload};
use maud::{html, Markup};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

const QUALITIES: [&str; 6] = ["best", "1080", "720", "480", "360", "audio"];

pub fn routes(r: Router<Shared>) -> Router<Shared> {
    r.route("/dlg/playlist", route_get(open_dialog))
        .route("/dlg/playlist/fetch", post(fetch))
        .route("/dlg/playlist/poll", post(poll))
        .route("/dlg/playlist/toggle", post(toggle))
        .route("/dlg/playlist/checkall", post(check_all))
        .route("/dlg/playlist/download", post(download))
}

#[derive(Clone)]
struct Entry {
    checked: bool,
    title: String,
    url: String,
    length: String,
}

#[derive(Default)]
pub struct PlaylistHub {
    entries: Vec<Entry>,
    title: String,
    busy: bool,
    status: String,
    answer: Arc<Mutex<Option<Result<batiie_engine::ytdlp::PlaylistInfo, String>>>>,
    folder: String,
}

fn hub(app: &App) -> std::sync::MutexGuard<'_, super::misc::MiscState> {
    app.misc.lock().unwrap_or_else(|e| e.into_inner())
}

fn fmt_len(secs: u64) -> String {
    if secs == 0 {
        String::new()
    } else if secs >= 3600 {
        format!("{}:{:02}:{:02}", secs / 3600, secs / 60 % 60, secs % 60)
    } else {
        format!("{}:{:02}", secs / 60, secs % 60)
    }
}

fn checked_text(h: &PlaylistHub) -> String {
    tf("{} of {} videos checked", &[&h.entries.iter().filter(|e| e.checked).count(), &h.entries.len()])
}

fn list_markup(h: &PlaylistHub, oob: bool) -> Markup {
    html! {
        div id="pl-list" class="tbl" hx-swap-oob=[oob.then_some("true")] style="--tcols:26px 36px 1fr 70px;height:280px" {
            div class="tr th" { span {} span { "#" } span { (t("Video")) } span class="td-r" { (t("Length")) } }
            @for (i, e) in h.entries.iter().enumerate() {
                label class="tr pick" {
                    input type="checkbox" checked[e.checked] hx-post=(format!("/dlg/playlist/toggle?i={i}")) hx-trigger="change" hx-swap="none";
                    span class="muted" { (i + 1) }
                    span class="ell" title=(e.title) { (e.title) }
                    span class="td-r muted" { (e.length) }
                }
            }
            @if h.entries.is_empty() { div class="tr muted" { span {} span {} span { (if h.busy { t("Getting the videos of the playlist…") } else { t("The videos of the playlist show up here.") }) } } }
        }
    }
}

fn status_markup(h: &PlaylistHub, oob: bool) -> Markup {
    html! {
        div id="pl-status" class="muted" hx-swap-oob=[oob.then_some("true")]
            hx-post=[h.busy.then_some("/dlg/playlist/poll")] hx-trigger=[h.busy.then_some("every 500ms")] hx-swap="none" {
            @if h.busy { span class="spin" {} " " }
            (h.status) " "
            span id="pl-count" { (if h.entries.is_empty() { String::new() } else { checked_text(h) }) }
        }
    }
}

async fn open_dialog(State(app): State<Shared>) -> Response {
    // a playlist address in the clipboard is the likely one
    let clip = arboard::Clipboard::new().ok().and_then(|mut c| c.get_text().ok()).unwrap_or_default();
    let clip = clip.trim().to_string();
    let url = if clip.contains("list=") || clip.contains("/playlist") || clip.contains("/@") { clip } else { String::new() };
    let m = hub(&app);
    let h = &m.playlist;
    let quality = opts(&["Best quality (MP4)", "1080p", "720p", "480p", "360p", "Audio only (M4A)"]);
    modal(dialog(
        "d-playlist",
        &t("Download a playlist"),
        "wide",
        None,
        html! {
            form id="pl-form" hx-post="/dlg/playlist/download" hx-swap="none" {
                div class="dlg-body" {
                    p style="margin:0" { (t("Paste the address of a playlist:")) }
                    div class="pathbox" {
                        input class="in" type="text" name="url" value=(url) placeholder=(t("https://…/playlist?list=…")) autocomplete="off" spellcheck="false" data-noselect;
                        button type="button" class="btn primary" data-enter hx-post="/dlg/playlist/fetch" hx-include="#pl-form" hx-swap="none" { (t("Get videos")) }
                    }
                    (status_markup(h, false))
                    div class="row-flex" {
                        button type="button" class="btn sm" hx-post="/dlg/playlist/checkall?on=1" hx-swap="none" { (t("Check all")) }
                        button type="button" class="btn sm" hx-post="/dlg/playlist/checkall?on=0" hx-swap="none" { (t("Uncheck all")) }
                    }
                    (list_markup(h, false))
                    div class="row-flex" {
                        (field(&t("Quality"), select("quality", &quality, "0")))
                        (field(&t("Files at the same time"), number("parallel", 2, 1, 8)))
                    }
                    div id="pl-folderbox" { (field(&t("Save to"), path("folder", &h.folder, "folder"))) }
                    div class="row-flex" {
                        (check("numbered", &t("Number the files"), true))
                        (check("start_now", &t("Start now"), true))
                    }
                    div id="pl-msg" {}
                }
                (footer(html! { span class="sp" {} (primary(&t("Download"))) (btn_close(&t("Close"))) }))
            }
        },
    ))
}

async fn fetch(State(app): State<Shared>, Form(p): Params) -> Response {
    let url = get(&p, "url");
    let msg = |m: String| html! { div id="pl-msg" hx-swap-oob="true" { (msg_err(&m)) } }.into_response();
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return msg(t("Paste the playlist address (https://…/playlist?list=…)"));
    }
    let base = app.mgr.settings().dir_for("Video");
    let answer = {
        let mut m = hub(&app);
        let h = &mut m.playlist;
        h.busy = true;
        h.entries.clear();
        h.status = t("Getting the list of videos… (a few seconds)");
        h.answer = Arc::new(Mutex::new(None));
        h.answer.clone()
    };
    let _ = base;
    std::thread::spawn(move || {
        let r = batiie_engine::ytdlp::playlist(&url).map_err(|e| format!("{e:#}"));
        *answer.lock().unwrap_or_else(|e| e.into_inner()) = Some(r);
    });
    let m = hub(&app);
    html! { div id="pl-msg" hx-swap-oob="true" {} (status_markup(&m.playlist, true)) (list_markup(&m.playlist, true)) }.into_response()
}

async fn poll(State(app): State<Shared>) -> Response {
    let base = app.mgr.settings().dir_for("Video");
    let mut m = hub(&app);
    let h = &mut m.playlist;
    let taken = h.answer.lock().unwrap_or_else(|e| e.into_inner()).take();
    let Some(result) = taken else { return axum::http::StatusCode::NO_CONTENT.into_response() };
    h.busy = false;
    let mut folder_html = String::new();
    match result {
        Ok(p) if p.entries.is_empty() => h.status = t("No videos were found at this address."),
        Ok(p) => {
            h.entries = p.entries.iter().map(|e| Entry { checked: true, title: e.title.clone(), url: e.url.clone(), length: fmt_len(e.duration) }).collect();
            h.title = p.title.clone();
            h.status = tf("{} videos", &[&p.entries.len()]);
            h.folder = base.join(filename::sanitize(&p.title)).display().to_string();
            folder_html = html! { div id="pl-folderbox" hx-swap-oob="true" { (field(&t("Save to"), path("folder", &h.folder, "folder"))) } }.into_string();
        }
        Err(e) => h.status = tf("Could not get the list: {}", &[&e]),
    }
    let out = html! { (status_markup(h, true)) (list_markup(h, true)) (maud::PreEscaped(folder_html)) };
    out.into_response()
}

async fn toggle(State(app): State<Shared>, Query(q): Query<HashMap<String, String>>) -> Response {
    let mut m = hub(&app);
    if let Some(e) = q.get("i").and_then(|i| i.parse::<usize>().ok()).and_then(|i| m.playlist.entries.get_mut(i)) {
        e.checked = !e.checked;
    }
    html! { span id="pl-count" hx-swap-oob="true" { (checked_text(&m.playlist)) } }.into_response()
}

async fn check_all(State(app): State<Shared>, Query(q): Query<HashMap<String, String>>) -> Response {
    let on = q.get("on").map(|v| v == "1").unwrap_or(false);
    let mut m = hub(&app);
    for e in m.playlist.entries.iter_mut() {
        e.checked = on;
    }
    html! { (list_markup(&m.playlist, true)) span id="pl-count" hx-swap-oob="true" { (checked_text(&m.playlist)) } }.into_response()
}

async fn download(State(app): State<Shared>, Form(p): Params) -> Response {
    let msg = |m: String| html! { div id="pl-msg" hx-swap-oob="true" { (msg_err(&m)) } }.into_response();
    let (chosen, title) = {
        let m = hub(&app);
        let c: Vec<(usize, Entry)> = m.playlist.entries.iter().cloned().enumerate().filter(|(_, e)| e.checked).collect();
        (c, m.playlist.title.clone())
    };
    if chosen.is_empty() {
        return msg(t("Check the videos you want first"));
    }
    let folder = get(&p, "folder");
    if folder.is_empty() {
        return msg(t("Choose where to save the videos"));
    }
    let folder = PathBuf::from(folder);
    let quality = QUALITIES[get(&p, "quality").parse::<usize>().unwrap_or(0).min(5)].to_string();
    let ext = if quality == "audio" { "m4a" } else { "mp4" };
    // its own queue: N videos at a time, all into the folder
    let name = if title.trim().is_empty() { "Playlist".to_string() } else { title.trim().chars().take(60).collect() };
    let parallel = get(&p, "parallel").parse::<u32>().unwrap_or(2).clamp(1, 8);
    let queue = match app.mgr.add_queue(&name, parallel, Some(folder.clone())) {
        Ok(q) => q,
        Err(e) => return msg(format!("{e:#}")),
    };
    let width = chosen.len().to_string().len().max(2);
    for (i, e) in &chosen {
        let mut file = filename::sanitize(&e.title);
        if flag(&p, "numbered") {
            file = format!("{:0width$} - {file}", i + 1);
        }
        app.mgr.add(NewDownload {
            url: e.url.clone(),
            file_name: Some(format!("{file}.{ext}")),
            save_dir: Some(folder.clone()),
            source_page: Some(e.url.clone()),
            pick_quality: Some(quality.clone()),
            queue: Some(queue),
            description: Some(title.clone()).filter(|t| !t.is_empty()),
            start: false,
            ..Default::default()
        });
    }
    if flag(&p, "start_now") {
        app.mgr.start_queue(queue);
    }
    app.ui().view = format!("queue:{queue}");
    let _ = std::fs::create_dir_all(&folder);
    app.wake.notify_waiters();
    super::actions::closing(&app, "d-playlist")
}
