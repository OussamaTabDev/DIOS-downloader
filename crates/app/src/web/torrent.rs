//! Torrents: the "Add torrent" window (a .torrent file or a magnet link, with the list
//! of files to choose from) and the torrent's own live window.
use super::actions::{closing, done, flag, get, is_info_hash, modal, reveal_in_folder, Params};
use super::page::state_of;
use super::ui::*;
use super::Shared;
use crate::i18n::{t, tf};
use crate::state::App;
use axum::body::Bytes;
use axum::extract::{Form, Path, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use axum::routing::{get as route_get, post};
use axum::Router;
use batiie_engine::model::TorrentRequest;
use batiie_engine::torrent::{TorrentLimits, TorrentMeta};
use batiie_engine::{fmt_duration, fmt_size, DownloadId, DownloadView, Status, TorrentSource};
use maud::{html, Markup};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub fn routes(r: Router<Shared>) -> Router<Shared> {
    r.route("/dlg/torrent", route_get(add_dialog))
        .route("/dlg/torrent/resolve", post(resolve))
        .route("/dlg/torrent/info", post(info_poll))
        .route("/dlg/torrent/summary", post(summary))
        .route("/dlg/torrent/all", post(all_files))
        .route("/dlg/torrent/paste", post(paste_magnet))
        .route("/dlg/torrent/ok", post(confirm))
        .route("/dlg/torrent/cancel", post(cancel))
        .route("/dlg/torrent/{id}/{op}", post(float_op))
        .route("/upload/torrent", post(upload))
}

/// A torrent whose contents were read and that waits in the window for Start / Later.
#[derive(Clone)]
pub struct PendingTorrent {
    pub source: TorrentSource,
    pub meta: TorrentMeta,
}

type Answer = Option<(u64, TorrentSource, Result<TorrentMeta, String>)>;

#[derive(Default)]
pub struct TorrentHub {
    preset: Option<TorrentSource>,
    pending: Option<PendingTorrent>,
    seq: u64,
    resolving: bool,
    error: Option<String>,
    answer: Arc<Mutex<Answer>>,
}

fn hub(app: &App) -> std::sync::MutexGuard<'_, super::misc::MiscState> {
    app.misc.lock().unwrap_or_else(|e| e.into_inner())
}

/// Open "Add torrent". `preset` comes from a .torrent opened with the app, a magnet link
/// (menu, browser, command line) or the clipboard.
pub fn open_add(app: &App, preset: Option<TorrentSource>) {
    hub(app).torrent.preset = preset;
    app.open("/dlg/torrent");
}

/// A dropped .torrent file (the page sends its bytes): it is kept in the data folder.
async fn upload(State(app): State<Shared>, headers: HeaderMap, body: Bytes) -> Response {
    if body.is_empty() || body.len() > 20_000_000 {
        return axum::http::StatusCode::BAD_REQUEST.into_response();
    }
    let name = headers
        .get("x-filename")
        .and_then(|v| v.to_str().ok())
        .map(|v| url::form_urlencoded::parse(format!("n={v}").as_bytes()).next().map(|(_, v)| v.into_owned()).unwrap_or_default())
        .unwrap_or_default();
    let name = batiie_engine::filename::sanitize(&name);
    let name = if name.to_ascii_lowercase().ends_with(".torrent") { name } else { "dropped.torrent".to_string() };
    let dir = batiie_engine::default_data_dir().join("dropped");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join(name);
    if std::fs::write(&path, &body).is_err() {
        return axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    hub(&app).torrent.preset = Some(TorrentSource::Torrent(path));
    axum::http::StatusCode::NO_CONTENT.into_response()
}

fn source_of(p: &std::collections::HashMap<String, String>) -> Option<TorrentSource> {
    match get(p, "mode").as_str() {
        "1" => {
            let m = get(p, "magnet");
            if m.starts_with("magnet:") {
                Some(TorrentSource::Magnet(m))
            } else if is_info_hash(&m) {
                Some(TorrentSource::Magnet(format!("magnet:?xt=urn:btih:{m}")))
            } else {
                None
            }
        }
        _ => {
            let path = PathBuf::from(get(p, "torrent_path"));
            path.is_file().then_some(TorrentSource::Torrent(path))
        }
    }
}

/// Forget the torrent that was read for the window. It leaves the session, unless the
/// download list already has it (its torrent must stay there).
fn drop_pending(app: &App) {
    let p = {
        let mut h = hub(app);
        h.torrent.seq += 1;
        h.torrent.resolving = false;
        h.torrent.error = None;
        h.torrent.pending.take()
    };
    if let Some(p) = p {
        if app.mgr.find_url(&p.source.display()).is_none() {
            let hash = p.meta.info_hash.clone();
            let engine = app.mgr.torrent_engine();
            app.rt.spawn(async move { engine.remove(&hash, false).await });
        }
    }
}

/// Read what is inside the torrent (its metadata) in the background. A .torrent file has
/// it at once; a magnet link asks the DHT / trackers and can take a minute.
fn start_resolve(app: &App, source: TorrentSource, folder: PathBuf) {
    drop_pending(app);
    let (seq, answer) = {
        let mut h = hub(app);
        h.torrent.seq += 1;
        h.torrent.resolving = true;
        (h.torrent.seq, h.torrent.answer.clone())
    };
    let _ = std::fs::create_dir_all(&folder);
    let engine = app.mgr.torrent_engine();
    let s = app.mgr.settings();
    let limits = TorrentLimits { download_kb: s.torrent_dl_limit_kb, upload_kb: s.torrent_ul_limit_kb };
    app.rt.spawn(async move {
        let result = engine
            .add_paused(&source, &folder, Vec::new(), limits)
            .await
            .and_then(|handle| batiie_engine::torrent::meta_of(&handle))
            .map_err(|e| format!("{e:#}"));
        *answer.lock().unwrap_or_else(|e| e.into_inner()) = Some((seq, source, result));
    });
}

/// Take the answer of the check, if it is the newest one.
fn take_answer(app: &App) {
    let mut h = hub(app);
    let taken = h.torrent.answer.lock().unwrap_or_else(|e| e.into_inner()).take();
    let Some((seq, source, result)) = taken else { return };
    if seq != h.torrent.seq {
        return;
    }
    h.torrent.resolving = false;
    match result {
        Ok(meta) => {
            h.torrent.error = None;
            h.torrent.pending = Some(PendingTorrent { source, meta });
        }
        Err(e) => {
            h.torrent.error = Some(e);
            h.torrent.pending = None;
        }
    }
}

fn files_markup(meta: &TorrentMeta) -> Markup {
    html! {
        div id="tor-files" {
            div class="row-flex" style="margin-bottom:6px" {
                button type="button" class="btn sm" data-checkall="#tor-files input[name=file]" data-state="1" { (t("Select all")) }
                button type="button" class="btn sm" data-checkall="#tor-files input[name=file]" data-state="0" { (t("Select none")) }
                span id="tor-sum" class="muted" {}
            }
            div class="tbl" style="--tcols:26px 1fr 90px;max-height:230px" hx-post="/dlg/torrent/summary" hx-trigger="change" hx-include="#tor-files input" hx-target="#tor-sum" hx-swap="innerHTML" {
                @for f in &meta.files {
                    label class="tr pick" {
                        input type="checkbox" name="file" value=(f.index) checked;
                        span class="ell" title=(f.path) { (f.path) }
                        span class="td-r" { (fmt_size(f.size)) }
                    }
                }
            }
        }
    }
}

fn info_panel(app: &App, oob: bool) -> Markup {
    let h = hub(app);
    let (resolving, error, pending) = (h.torrent.resolving, h.torrent.error.clone(), h.torrent.pending.clone());
    drop(h);
    html! {
        div id="tor-info" class="addinfo" hx-swap-oob=[oob.then_some("true")]
            hx-post=[resolving.then_some("/dlg/torrent/info")] hx-trigger=[resolving.then_some("every 500ms")] hx-swap="none" {
            @if resolving { div class="ai-top" { span class="spin" {} span { (t("Fetching the torrent info…")) } } }
            @else if let Some(e) = error { div class="err-t sel-text" { (e) } }
            @else if let Some(p) = &pending {
                div class="ai-top" {
                    span class="ai-icon k-torrent" { (icon("torrent")) }
                    div class="ai-txt sel-text" { b { (p.meta.name) } }
                    div class="ai-size" { (fmt_size(p.meta.total_size)) }
                }
                div class="muted" { (tf("{} files  \u{b7}  {}", &[&p.meta.files.len(), &fmt_size(p.meta.total_size)])) }
            } @else { span class="muted" { (t("Choose a torrent file, or paste a magnet link.")) } }
        }
    }
}

async fn add_dialog(State(app): State<Shared>) -> Response {
    let s = app.mgr.settings();
    let preset = hub(&app).torrent.preset.take();
    let (mode, magnet, tpath) = match &preset {
        Some(TorrentSource::Magnet(m)) => ("1", m.clone(), String::new()),
        Some(TorrentSource::Torrent(p)) => ("0", String::new(), p.display().to_string()),
        None => {
            // a magnet link waiting in the clipboard? start with it
            let clip = arboard::Clipboard::new().ok().and_then(|mut c| c.get_text().ok()).unwrap_or_default();
            let clip = clip.trim().to_string();
            if clip.starts_with("magnet:") { ("1", clip, String::new()) } else { ("0", String::new(), String::new()) }
        }
    };
    drop_pending(&app);
    if let Some(src) = preset {
        start_resolve(&app, src, s.download_dir.clone());
    }
    let queues: Vec<(String, String)> = s.queues.iter().map(|q| (q.id.to_string(), q.name.clone())).collect();
    let panel = info_panel(&app, false);
    modal(dialog(
        "d-torrent",
        &t("Add torrent"),
        "wide",
        Some("/dlg/torrent/cancel"),
        html! {
            form id="tor-form" hx-post="/dlg/torrent/ok" hx-swap="none" {
                div class="dlg-body modes" {
                    div class="row-flex" {
                        label class="chk" { input type="radio" name="mode" value="0" checked[mode == "0"] hx-post="/dlg/torrent/resolve" hx-trigger="change" hx-include="#tor-form" hx-target="#tor-info" hx-swap="outerHTML"; span class="box" { (icon("check")) } span class="chk-l" { (t("Torrent file")) } }
                        label class="chk" { input type="radio" name="mode" value="1" checked[mode == "1"] hx-post="/dlg/torrent/resolve" hx-trigger="change" hx-include="#tor-form" hx-target="#tor-info" hx-swap="outerHTML"; span class="box" { (icon("check")) } span class="chk-l" { (t("Magnet link")) } }
                    }
                    div data-mode="0" class="col" {
                        div class="pathbox" id="p-torrent_path" {
                            input class="in" type="text" name="torrent_path" value=(tpath) placeholder=(t("Choose a torrent file")) autocomplete="off" spellcheck="false"
                                hx-post="/dlg/torrent/resolve" hx-trigger="change" hx-include="#tor-form" hx-target="#tor-info" hx-swap="outerHTML";
                            button type="button" class="btn" hx-post="/pick/torrent" hx-include="#p-torrent_path input" hx-target="#p-torrent_path input" hx-swap="outerHTML" hx-vals=r#"{"name":"torrent_path"}"# { (icon("folder")) (t("Browse…")) }
                        }
                    }
                    div data-mode="1" class="col" {
                        div class="pathbox" {
                            input class="in" id="tor-magnet" type="text" name="magnet" value=(magnet) placeholder="magnet:?xt=urn:btih:…" autocomplete="off" spellcheck="false"
                                hx-post="/dlg/torrent/resolve" hx-trigger="change" hx-include="#tor-form" hx-target="#tor-info" hx-swap="outerHTML";
                            button type="button" class="btn" hx-post="/dlg/torrent/paste" hx-target="#tor-magnet" hx-swap="outerHTML" { (icon("copy")) (t("Paste")) }
                        }
                    }
                    (panel)
                    div id="tor-list" {}
                    (field(&t("Save To"), path("folder", &s.download_dir.display().to_string(), "folder")))
                    div class="row-flex" {
                        (field(&t("Queue"), select("queue", &queues, &queues.first().map(|q| q.0.clone()).unwrap_or_default())))
                        (field(&t("Description"), html! { input class="in" type="text" name="description" placeholder=(t("optional note")) autocomplete="off"; }))
                    }
                    div id="tor-msg" {}
                }
                (footer(html! {
                    span class="sp" {}
                    button type="submit" name="act" value="later" class="btn" { (t("Download Later")) }
                    button type="submit" name="act" value="start" class="btn primary" { (t("Start Download")) }
                    (btn_close(&t("Cancel")))
                }))
            }
        },
    ))
}

async fn paste_magnet() -> Markup {
    let mut value = String::new();
    if let Some(text) = arboard::Clipboard::new().ok().and_then(|mut c| c.get_text().ok()) {
        let text = text.trim().to_string();
        if text.starts_with("magnet:") || is_info_hash(&text) {
            value = text;
        }
    }
    html! { input class="in" id="tor-magnet" type="text" name="magnet" value=(value) placeholder="magnet:?xt=urn:btih:…" autocomplete="off" spellcheck="false"
        hx-post="/dlg/torrent/resolve" hx-trigger="load, change" hx-include="#tor-form" hx-target="#tor-info" hx-swap="outerHTML"; }
}

async fn resolve(State(app): State<Shared>, Form(p): Params) -> Response {
    let folder = PathBuf::from(get(&p, "folder"));
    match source_of(&p) {
        Some(src) if !folder.as_os_str().is_empty() => start_resolve(&app, src, folder),
        _ => {
            drop_pending(&app);
        }
    }
    let mut r = html! { (info_panel(&app, false)) div id="tor-list" hx-swap-oob="true" {} }.into_response();
    r.headers_mut().insert("hx-reswap", "none".parse().unwrap());
    // the panel is an out-of-band swap, so the visible one is replaced by id
    let body = html! { (info_panel(&app, true)) div id="tor-list" hx-swap-oob="true" {} };
    let mut r2 = body.into_response();
    r2.headers_mut().insert("hx-reswap", "none".parse().unwrap());
    let _ = r;
    r2
}

/// Polled while the check runs.
async fn info_poll(State(app): State<Shared>) -> Response {
    let before = hub(&app).torrent.resolving;
    take_answer(&app);
    let (now, meta) = {
        let h = hub(&app);
        (h.torrent.resolving, h.torrent.pending.as_ref().map(|p| p.meta.clone()))
    };
    if before && now {
        return done(&app);
    }
    let list = match &meta {
        Some(m) => html! { div id="tor-list" hx-swap-oob="true" { (files_markup(m)) } },
        None => html! { div id="tor-list" hx-swap-oob="true" {} },
    };
    html! { (info_panel(&app, true)) (list) }.into_response()
}

/// "3 of 5 files · 1.2 GB of 4 GB" for the ticked files.
async fn summary(State(app): State<Shared>, body: Bytes) -> Response {
    let ticked: Vec<usize> = url::form_urlencoded::parse(&body).filter(|(k, _)| k == "file").filter_map(|(_, v)| v.parse().ok()).collect();
    let Some(p) = hub(&app).torrent.pending.clone() else { return done(&app) };
    let selected = p.meta.size_of(&ticked);
    let text = tf("{} of {} files  ·  {} of {}", &[&ticked.len(), &p.meta.files.len(), &fmt_size(selected), &fmt_size(p.meta.total_size)]);
    maud::PreEscaped(html! { (text) }.into_string()).into_response()
}

async fn all_files() -> Response {
    axum::http::StatusCode::NO_CONTENT.into_response()
}

async fn cancel(State(app): State<Shared>) -> Response {
    drop_pending(&app);
    done(&app)
}

/// "Start Download" / "Download Later": the torrent becomes a list entry.
async fn confirm(State(app): State<Shared>, body: Bytes) -> Response {
    let pairs: Vec<(String, String)> = url::form_urlencoded::parse(&body).into_owned().collect();
    let p: std::collections::HashMap<String, String> = pairs.iter().cloned().collect();
    let start = get(&p, "act") != "later";
    let msg = |m: String| html! { div id="tor-msg" hx-swap-oob="true" { (msg_err(&m)) } }.into_response();
    take_answer(&app);
    let Some(pend) = hub(&app).torrent.pending.clone() else {
        return msg(t("Choose a torrent file, or paste a magnet link."));
    };
    let folder = PathBuf::from(get(&p, "folder"));
    if folder.as_os_str().is_empty() {
        return msg(t("Choose a folder to save the torrent into."));
    }
    let mut files: Vec<usize> = pairs.iter().filter(|(k, _)| k == "file").filter_map(|(_, v)| v.parse().ok()).collect();
    if files.is_empty() {
        return msg(t("Tick at least one file."));
    }
    // everything ticked = no filter
    if files.len() == pend.meta.files.len() {
        files = (0..pend.meta.files.len()).collect();
    }
    let s = app.mgr.settings();
    let description = get(&p, "description");
    let queue = get(&p, "queue").parse::<u64>().ok().filter(|q| s.queue(*q).is_some());
    let req = TorrentRequest {
        source: pend.source.clone(),
        meta: pend.meta.clone(),
        files,
        save_dir: folder,
        queue: if start { None } else { queue },
        description: (!description.is_empty()).then_some(description),
        start,
    };
    match app.mgr.add_torrent(req) {
        Ok(id) => {
            // the torrent stays in the session; only the window's copy goes
            hub(&app).torrent.pending = None;
            if start {
                app.mgr.start(id);
                super::progress::open(&app, id);
            }
            closing(&app, "d-torrent")
        }
        Err(e) => msg(format!("{e:#}")),
    }
}

// ---------------------------------------------------------------- the torrent's own window

/// The live part of a torrent's window: numbers, bar and the file list.
pub fn float_inner(app: &App, v: &DownloadView) -> Markup {
    let pct = v.progress();
    let st = state_of(v.status);
    let status = if v.checking {
        t("Checking existing files…")
    } else {
        match v.status {
            Status::Downloading => t("Downloading from peers…"),
            Status::Assembling => t("Joining the parts…"),
            Status::Failed => t("Error"),
            other => t(other.label()),
        }
    };
    let rate = |s: f64| if s > 0.0 { format!("{}/s", fmt_size(s as u64)) } else { "—".to_string() };
    let have = app.mgr.torrent_have(v.id);
    let id = v.id;
    let running = v.status.is_running() || v.status == Status::Queued;
    html! {
        div class="pg-top" {
            span class="fi big k-torrent" { (icon("torrent")) }
            div class="pg-name" { div class="big-name" title=(v.file_name) { (v.file_name) } div class="muted ell sel-text" title=(v.url) { (v.url) } }
            div class="pg-pct" { (format!("{:.0}%", pct * 100.0)) }
        }
        div class={"bar bigbar s" (st)} { i style=(format!("width:{:.2}%", pct * 100.0)) {} }
        dl class="stats" {
            dt { (t("Status")) } dd class={"s" (st)} { (status) }
            dt { (t("File size")) } dd { (fmt_size(v.torrent.as_ref().map(|t| t.total_size).unwrap_or(0))) }
            dt { (t("Downloaded")) } dd { (format!("{} ({:.1} %)", fmt_size(v.downloaded), pct * 100.0)) }
            dt { (t("Uploaded")) } dd { (fmt_size(v.uploaded)) }
            dt { (t("Transfer rate")) } dd { (rate(v.speed)) }
            dt { (t("Upload rate")) } dd { (rate(v.up_speed)) }
            dt { (t("Time left")) } dd { (v.eta_secs.map(fmt_duration).unwrap_or_else(|| "—".into())) }
            dt { (t("Peers")) } dd { (tf("{} connected  ·  {} seen", &[&v.peers, &v.seen])) }
        }
        @if let Some(e) = &v.error { @if v.status == Status::Failed { (msg_err(e)) } }
        @if let Some(tor) = &v.torrent {
            @let count = if tor.files.is_empty() { tor.file_list.len() } else { tor.files.len() };
            @let got: u64 = if tor.files.is_empty() { v.downloaded.min(tor.total_size) } else { tor.files.iter().filter_map(|&fi| have.get(fi).copied()).sum::<u64>().min(tor.total_size) };
            div class="row-flex" {
                button type="button" class="btn sm" hx-post=(format!("/dlg/torrent/{id}/all")) hx-vals=r#"{"on":1}"# hx-swap="none" { (t("Select all")) }
                button type="button" class="btn sm" hx-post=(format!("/dlg/torrent/{id}/all")) hx-vals=r#"{"on":0}"# hx-swap="none" { (t("Select none")) }
                span class="muted" { (tf("{} of {} files  ·  {} of {}", &[&count, &tor.file_list.len(), &fmt_size(got), &fmt_size(tor.total_size)])) }
            }
            div class="tbl tfiles" style="--tcols:26px 1fr 80px 90px;max-height:210px" {
                @for (i, f) in tor.file_list.iter().enumerate() {
                    @let on = tor.files.is_empty() || tor.files.contains(&i);
                    @let fp = if f.size > 0 { have.get(i).copied().unwrap_or(0) as f64 / f.size as f64 } else { 1.0 };
                    div class="tr" {
                        label class="chk" { input type="checkbox" checked[on] hx-post=(format!("/dlg/torrent/{id}/file")) hx-vals=(format!(r#"{{"index":{i}}}"#)) hx-swap="none" hx-trigger="change"; span class="box" { (icon("check")) } }
                        span class="ell" title=(f.path) { (f.path) }
                        span class="td-r" { (fmt_size(f.size)) }
                        div class="bar sm s1" { i style=(format!("width:{:.1}%", fp.clamp(0.0, 1.0) * 100.0)) {} }
                    }
                }
            }
        }
        div class="pg-btns" {
            @if running {
                button type="button" class="btn" hx-post=(format!("/dlg/torrent/{id}/pause")) hx-swap="none" { (icon("pause")) (t("Pause")) }
            } @else if v.status != Status::Completed {
                button type="button" class="btn primary" hx-post=(format!("/dlg/torrent/{id}/resume")) hx-swap="none" { (icon("play")) (t("Resume")) }
            }
            button type="button" class="btn" hx-post=(format!("/dlg/torrent/{id}/folder")) hx-swap="none" { (icon("folder")) (t("Open folder")) }
            span class="sp" {}
            button type="button" class="btn" hx-post=(format!("/w/progress/{id}/close")) hx-swap="none" { (t("Hide")) }
            button type="button" class="btn" hx-post=(format!("/w/progress/{id}/cancel")) hx-swap="none" { (t("Cancel")) }
        }
    }
}

async fn float_op(State(app): State<Shared>, Path((id, op)): Path<(DownloadId, String)>, Form(p): Params) -> Response {
    match op.as_str() {
        "pause" => app.mgr.pause(id),
        "resume" => app.mgr.start(id),
        "folder" => {
            if let Some(v) = app.mgr.get(id) {
                reveal_in_folder(&v);
            }
        }
        // flip that file in the record's selection
        "file" => {
            let index = get(&p, "index").parse::<usize>().unwrap_or(usize::MAX);
            if let Some(tor) = app.mgr.get(id).and_then(|v| v.torrent) {
                let mut files = if tor.files.is_empty() { (0..tor.file_list.len()).collect() } else { tor.files.clone() };
                if files.contains(&index) {
                    files.retain(|&f| f != index);
                } else if index < tor.file_list.len() {
                    files.push(index);
                }
                app.mgr.set_torrent_files(id, files);
            }
        }
        "all" => {
            let count = app.mgr.get(id).and_then(|v| v.torrent).map(|t| t.file_list.len()).unwrap_or(0);
            let files: Vec<usize> = if flag(&p, "on") { (0..count).collect() } else { Vec::new() };
            app.mgr.set_torrent_files(id, files);
        }
        _ => return axum::http::StatusCode::NOT_FOUND.into_response(),
    }
    done(&app)
}
