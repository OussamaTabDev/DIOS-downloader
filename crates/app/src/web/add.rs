//! "Download File Info": the window that opens for a new address. It checks the
//! address in the background, fills in the name and folder, and (like the original)
//! can already start downloading while the window is open.
use super::actions::{closing, flag, get, modal, Params};
use super::ui::*;
use super::Shared;
use crate::i18n::{t, tf};
use crate::state::App;
use axum::extract::{Form, State};
use axum::response::{IntoResponse, Response};
use axum::routing::{get as route_get, post};
use axum::Router;
use batiie_engine::filename;
use batiie_engine::{fmt_size, DownloadId, DownloadKind, NewDownload, ProbeResult, RequestHeaders, Status};
use maud::{html, Markup};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub fn routes(r: Router<Shared>) -> Router<Shared> {
    r.route("/dlg/add", route_get(open_dialog))
        .route("/dlg/add/check", post(check_url))
        .route("/dlg/add/info", post(info_poll))
        .route("/dlg/add/category", post(category))
        .route("/dlg/add/ok", post(ok))
        .route("/dlg/add/browser", post(use_browser))
        .route("/dlg/add/cancel", post(cancel))
        .route("/dlg/dup", route_get(dup_dialog))
        .route("/dlg/dup/ok", post(dup_ok))
        .route("/dlg/dup/cancel", post(dup_cancel))
}

/// What the window is asked to show.
#[derive(Clone, Default)]
pub struct AddOpen {
    pub url: String,
    pub headers: RequestHeaders,
    pub file_name: Option<String>,
    pub audio_url: Option<String>,
    pub source_page: Option<String>,
    pub source_key: Option<String>,
}

impl AddOpen {
    pub fn url(u: String) -> Self {
        AddOpen { url: u, ..Default::default() }
    }
}

/// The answer of the background check.
#[derive(Default)]
struct ProbeSlot {
    seq: u64,
    /// None = still checking
    result: Option<Result<ProbeResult, String>>,
}

struct Session {
    open: AddOpen,
    path: String,
    auto_path: String,
    categories: Vec<String>,
    cat: String,
    early_id: Option<DownloadId>,
    probe_url: String,
    probe: Option<ProbeResult>,
    slot: Arc<Mutex<ProbeSlot>>,
    /// the probe result was already shown
    shown: bool,
    can_browser: bool,
    warning: String,
    use_login: bool,
    user: String,
    password: String,
}

/// A download waiting for the answer of the "Duplicate download link" window.
struct PendingAdd {
    req: NewDownload,
    existing: DownloadId,
}

#[derive(Default)]
pub struct AddHub {
    cur: Option<Session>,
    waiting: VecDeque<AddOpen>,
    dup: Option<PendingAdd>,
}

impl AddHub {
    /// The download started in the background for the open window, if any.
    pub fn early_id(&self) -> Option<DownloadId> {
        self.cur.as_ref().and_then(|s| s.early_id)
    }
}

fn hub(app: &App) -> std::sync::MutexGuard<'_, AddHub> {
    app.add.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn is_busy(app: &App) -> bool {
    hub(app).cur.is_some()
}

/// "Cancel": the download that started meanwhile goes away completely, with its
/// temporary files and the file itself.
pub fn cancel_early(app: &App) {
    let id = hub(app).cur.as_mut().and_then(|s| s.early_id.take());
    if let Some(id) = id {
        app.mgr.discard(id);
    }
}

/// A new address for the window. While it is busy with another one, it waits.
pub fn request(app: &App, o: AddOpen) {
    {
        let mut h = hub(app);
        if h.cur.is_some() {
            // the same link again (a page can send it twice): nothing new to show;
            // another link waits until this one is started or cancelled
            let same = h.cur.as_ref().map(|c| c.open.url == o.url).unwrap_or(false);
            if !o.url.is_empty() && !same && !h.waiting.iter().any(|p| p.url == o.url) {
                h.waiting.push_back(o);
            }
            drop(h);
            app.open("/dlg/add");
            return;
        }
    }
    start_session(app, o);
    app.open("/dlg/add");
}

fn open_next(app: &App) {
    let next = hub(app).waiting.pop_front();
    if let Some(o) = next {
        // the window closes and opens again with the next link
        let app2 = app.bus.clone();
        let _ = app2;
        start_session(app, o);
        app.open("/dlg/add");
    }
}

fn start_session(app: &App, o: AddOpen) {
    let s = app.mgr.settings();
    // nothing may be left over from an earlier window
    cancel_early(app);
    // General first, like the classic list
    let mut cats = vec!["General".to_string()];
    cats.extend(s.categories.iter().map(|c| c.name.clone()));
    let name = o
        .file_name
        .clone()
        .filter(|n| !n.trim().is_empty())
        .map(|n| filename::sanitize(&n))
        .or_else(|| filename::from_url(&o.url).map(|n| filename::sanitize(&n)))
        .unwrap_or_else(|| "download".into());
    let cat = s.category_of(&name, &o.url);
    let path = s.dir_for(&cat).join(&name).display().to_string();
    let login = o.headers.auth.clone().or_else(|| s.login_for(&o.url));
    let warning = if !o.url.is_empty() && app.mgr.find_url(&o.url).is_some() {
        t("This address is already in the download list.")
    } else {
        String::new()
    };
    let mut sess = Session {
        auto_path: if o.file_name.as_deref().map(|n| !n.trim().is_empty()).unwrap_or(false) { String::new() } else { path.clone() },
        path,
        categories: cats,
        cat,
        early_id: None,
        probe_url: String::new(),
        probe: None,
        slot: Arc::new(Mutex::new(ProbeSlot::default())),
        shown: false,
        can_browser: false,
        warning,
        use_login: login.is_some(),
        user: login.as_ref().map(|l| l.0.clone()).unwrap_or_default(),
        password: login.map(|l| l.1).unwrap_or_default(),
        open: o.clone(),
    };
    // it already downloads while the window is open
    if !o.url.is_empty() && s.start_while_info && app.mgr.find_url(&o.url).is_none() {
        let p = PathBuf::from(&sess.path);
        let id = app.mgr.add(NewDownload {
            url: o.url.clone(),
            file_name: p.file_name().map(|n| n.to_string_lossy().into_owned()),
            save_dir: p.parent().map(Path::to_path_buf),
            headers: o.headers.clone(),
            audio_url: o.audio_url.clone(),
            source_page: o.source_page.clone(),
            source_key: o.source_key.clone(),
            early: true,
            start: true,
            ..Default::default()
        });
        sess.early_id = Some(id);
    }
    if !o.url.is_empty() {
        spawn_probe(app, &sess.slot, o.url.clone(), sess.open.headers.clone(), None);
    }
    hub(app).cur = Some(sess);
}

/// Ask the server about the file in the background.
fn spawn_probe(app: &App, slot: &Arc<Mutex<ProbeSlot>>, url: String, mut headers: RequestHeaders, auth: Option<(String, String)>) {
    if auth.is_some() {
        headers.auth = auth;
    }
    let seq = {
        let mut s = slot.lock().unwrap_or_else(|e| e.into_inner());
        s.seq += 1;
        s.result = None;
        s.seq
    };
    let (mgr, slot) = (app.mgr.clone(), slot.clone());
    app.rt.spawn(async move {
        let r = mgr.probe(&url, &headers).await.map_err(|e| format!("{e:#}"));
        let mut s = slot.lock().unwrap_or_else(|e| e.into_inner());
        if s.seq == seq {
            s.result = Some(r);
        }
    });
}

// ---------------------------------------------------------------- the window

fn category_select(sess: &Session) -> Markup {
    let opts: Vec<(String, String)> = sess.categories.iter().map(|c| (c.clone(), t(c))).collect();
    html! {
        select class="in sel" id="add-cat" name="category" hx-post="/dlg/add/category" hx-include="#d-add form" hx-swap="none" {
            @for (v, l) in &opts { option value=(v) selected[*v == sess.cat] { (l) } }
        }
    }
}

fn saveas_box(path: &str, oob: bool) -> Markup {
    html! {
        div class="pathbox" id="p-saveas" hx-swap-oob=[oob.then_some("true")] {
            input class="in" type="text" name="saveas" value=(path) autocomplete="off" spellcheck="false";
            button type="button" class="btn" title=(t("Browse…")) hx-post="/pick/save" hx-include="#p-saveas input" hx-target="#p-saveas input" hx-swap="outerHTML" hx-vals=r#"{"name":"saveas"}"# { (icon("folder")) }
        }
    }
}

fn remember_label(cat: &str, oob: bool) -> Markup {
    html! { span class="chk-l" id="add-rem-l" hx-swap-oob=[oob.then_some("true")] { (tf("Remember this path for \"{}\" category", &[&t(cat)])) } }
}

/// The part under the address: kind icon, size, what the check found.
fn info_panel(app: &App, sess: &mut Session, oob: bool) -> Markup {
    let mut probing = false;
    let mut error: Option<String> = None;
    let mut size_text = String::new();
    let mut info = String::new();
    let url = sess.open.url.clone();
    let has_check = !sess.open.url.is_empty();
    if has_check {
        let res = {
            let slot = sess.slot.lock().unwrap_or_else(|e| e.into_inner());
            slot.result.clone()
        };
        match res {
            None => probing = true,
            Some(Ok(p)) => {
                let has_audio = sess.open.audio_url.is_some();
                let size = p.total_size.map(fmt_size).unwrap_or_else(|| t("Unknown size"));
                size_text = if has_audio { format!("{size}\n{}", t("+ audio")) } else { size };
                let kind = match (p.kind, p.resumable) {
                    _ if has_audio => "Video + audio, joined after download",
                    (DownloadKind::Hls, _) => "Video stream (HLS)",
                    (_, true) => "Resume supported",
                    (_, false) => "No resume support",
                };
                info = format!("{}  ·  {}", t(kind), p.content_type.clone().unwrap_or_default());
                if !sess.shown {
                    sess.shown = true;
                    sess.can_browser = false;
                    if p.is_web_page() {
                        sess.warning = t("This address opens a web page, not a file. The link may have expired or need a login.");
                    }
                    sess.probe_url = url.clone();
                    sess.probe = Some(p);
                }
            }
            Some(Err(e)) => {
                if !sess.shown {
                    sess.shown = true;
                    // refused (often a site that only serves its own browser session):
                    // the browser can still take it
                    sess.can_browser = ["HTTP 403", "HTTP 406", "HTTP 429"].iter().any(|c| e.contains(c));
                }
                error = Some(e);
            }
        }
    } else {
        info = t("Paste a link and press Check");
    }
    let _ = app;
    let ext = filename::extension(Path::new(&sess.path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default().as_str()).to_uppercase();
    let kind = super::page::kind_for(&sess.path);
    html! {
        div id="add-info" class="addinfo" hx-swap-oob=[oob.then_some("true")]
            hx-post=[probing.then_some("/dlg/add/info")] hx-trigger=[probing.then_some("every 350ms")] hx-include=[probing.then_some("#p-saveas input")] hx-swap=[probing.then_some("none")] {
            div class="ai-top" {
                span class={"ai-icon k-" (kind)} { (icon(super::page::file_icon(kind))) @if !ext.is_empty() { b { (ext) } } }
                div class="ai-txt" {
                    @if probing { span class="spin" {} " " (t("Checking the address…")) }
                    @else if let Some(e) = &error { span class="err-t sel-text" { (e) } }
                    @else { span class="sel-text" { (info) } }
                }
                div class="ai-size" { @for l in size_text.lines() { div { (l) } } }
            }
            @if !sess.warning.is_empty() { (msg_err(&sess.warning)) }
            @if sess.can_browser && error.is_some() {
                div { button type="button" class="btn" hx-post="/dlg/add/browser" hx-swap="none" { (icon("globe")) (t("Download with the browser")) } }
            }
        }
    }
}

async fn open_dialog(State(app): State<Shared>) -> Response {
    // opened from the toolbar with nothing in it
    if !is_busy(&app) {
        start_session(&app, AddOpen::url(String::new()));
    }
    let s = app.mgr.settings();
    let queues: Vec<(String, String)> = s.queues.iter().map(|q| (q.id.to_string(), q.name.clone())).collect();
    let (body, ) = {
        let mut h = hub(&app);
        let Some(sess) = h.cur.as_mut() else { return super::actions::done(&app) };
        let panel = info_panel(&app, sess, false);
        let sess = &*sess;
        (html! {
            form hx-post="/dlg/add/ok" hx-swap="none" autocomplete="off" {
                div class="dlg-body" {
                    div class="fld urlrow" {
                        span class="fld-l" { (t("URL")) }
                        div class="pathbox" {
                            input class="in" id="add-url" type="text" name="url" value=(sess.open.url) placeholder=(t("https://example.com/file")) autocomplete="off" spellcheck="false" data-noselect;
                            button type="button" class="btn" data-enter hx-post="/dlg/add/check" hx-include="#d-add form" hx-target="#add-info" hx-swap="outerHTML" { (icon("refresh")) (t("Check")) }
                        }
                    }
                    (panel)
                    div class="addgrid" {
                        (field(&t("Category"), category_select(sess)))
                        (field(&t("Queue"), select("queue", &queues, &queues.first().map(|q| q.0.clone()).unwrap_or_default())))
                    }
                    (field(&t("Save As"), saveas_box(&sess.path, false)))
                    label class="chk" { input type="checkbox" name="remember" value="1"; span class="box" { (icon("check")) } (remember_label(&sess.cat, false)) }
                    (field(&t("Description"), html! { input class="in" type="text" name="description" placeholder=(t("optional note")) autocomplete="off"; }))
                    div class="addgrid" {
                        (field(&t("Connections"), number("connections", s.connections(), 1, 64)))
                    }
                    div class="grp-lite" {
                        label class="chk" { input type="checkbox" id="add-uselogin" name="uselogin" value="1" checked[sess.use_login]; span class="box" { (icon("check")) } span class="chk-l" { (t("Use authorization")) } }
                        div class="addgrid" data-enable="#add-uselogin" {
                            (field(&t("Login"), text("user", &sess.user)))
                            (field(&t("Password"), password("password", &sess.password)))
                        }
                    }
                }
                (footer(html! {
                    span class="sp" {}
                    button type="submit" name="act" value="later" class="btn" { (t("Download Later")) }
                    button type="submit" name="act" value="start" class="btn primary" { (t("Start Download")) }
                    (btn_close(&t("Cancel")))
                }))
            }
        },)
    };
    modal(dialog("d-add", &t("Download File Info"), "", Some("/dlg/add/cancel"), body)).tap_keep()
}

trait Keep {
    fn tap_keep(self) -> Self;
}
impl Keep for Response {
    fn tap_keep(self) -> Self {
        self
    }
}

/// Check the address in the box (again).
async fn check_url(State(app): State<Shared>, Form(p): Params) -> Response {
    let url = get(&p, "url");
    let (slot, headers, was) = {
        let mut h = hub(&app);
        let Some(sess) = h.cur.as_mut() else { return super::actions::done(&app) };
        let was = sess.open.url.clone();
        // another address: the download started for the old one stops
        if url != sess.open.url {
            sess.probe = None;
            sess.probe_url.clear();
            sess.shown = false;
            sess.can_browser = false;
            sess.warning = if !url.is_empty() && app.mgr.find_url(&url).is_some() { t("This address is already in the download list.") } else { String::new() };
            sess.open.url = url.clone();
            // the extras (audio, source page) belong to the original address only
            sess.open.audio_url = None;
            sess.open.source_page = None;
            sess.open.source_key = None;
        }
        sess.shown = false;
        (sess.slot.clone(), sess.open.headers.clone(), was)
    };
    if url != was {
        cancel_early(&app);
    }
    if url.is_empty() {
        let mut h = hub(&app);
        let sess = h.cur.as_mut().unwrap();
        return info_panel(&app, sess, false).into_response();
    }
    let auth = (flag(&p, "uselogin") && !get(&p, "user").is_empty()).then(|| (get(&p, "user"), p.get("password").cloned().unwrap_or_default()));
    spawn_probe(&app, &slot, url, headers, auth);
    let mut h = hub(&app);
    let sess = h.cur.as_mut().unwrap();
    info_panel(&app, sess, false).into_response()
}

/// Polled while the check runs: the result, and the name / folder it found.
async fn info_poll(State(app): State<Shared>, Form(p): Params) -> Response {
    let mut h = hub(&app);
    let Some(sess) = h.cur.as_mut() else { return super::actions::done(&app) };
    // the answer arrived since the last look: name and folder follow it
    let mut extra = String::new();
    let had = sess.shown;
    let arrived = sess.slot.lock().unwrap_or_else(|e| e.into_inner()).result.is_some();
    if arrived && !had {
        // let info_panel take the result into the session first
        let _ = info_panel(&app, sess, true);
        if let Some(probe) = sess.probe.clone() {
            let current = p.get("saveas").cloned().unwrap_or_default();
            if !sess.auto_path.is_empty() && current == sess.auto_path {
                let s = app.mgr.settings();
                let cat = s.category_of(&probe.file_name, &sess.open.url);
                let path = s.dir_for(&cat).join(&probe.file_name).display().to_string();
                sess.cat = cat.clone();
                sess.auto_path = path.clone();
                sess.path = path.clone();
                extra.push_str(&saveas_box(&path, true).into_string());
                extra.push_str(&category_select(sess).into_string().replacen("<select ", r#"<select hx-swap-oob="true" "#, 1));
                extra.push_str(&remember_label(&cat, true).into_string());
            }
        }
    }
    let panel = info_panel(&app, sess, true).into_string();
    maud::PreEscaped(format!("{panel}{extra}")).into_response()
}

/// Another category: the file goes to that category's folder.
async fn category(State(app): State<Shared>, Form(p): Params) -> Response {
    let cat = get(&p, "category");
    let s = app.mgr.settings();
    let current = PathBuf::from(get(&p, "saveas"));
    let name = current.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "download".into());
    let path = s.dir_for(&cat).join(&name).display().to_string();
    if let Some(sess) = hub(&app).cur.as_mut() {
        sess.cat = cat.clone();
        sess.auto_path = path.clone();
        sess.path = path.clone();
    }
    let mut r = html! { (saveas_box(&path, true)) (remember_label(&cat, true)) }.into_response();
    r.headers_mut().insert("hx-reswap", "none".parse().unwrap());
    r
}

/// Add the download from the window.
async fn ok(State(app): State<Shared>, Form(p): Params) -> Response {
    let start = get(&p, "act") != "later";
    let url = get(&p, "url");
    let err = |msg: String| -> Response {
        html! { div id="add-info" hx-swap-oob="true" class="addinfo" { (msg_err(&msg)) } }.into_response()
    };
    if !(url.starts_with("http://") || url.starts_with("https://")) || url::Url::parse(&url).is_err() {
        return err(t("Please enter a valid http:// or https:// address"));
    }
    let save_as = PathBuf::from(get(&p, "saveas"));
    let name = save_as.file_name().map(|n| n.to_string_lossy().into_owned()).filter(|n| !n.trim().is_empty());
    let dir = save_as.parent().map(Path::to_path_buf).filter(|p| !p.as_os_str().is_empty());
    let settings = app.mgr.settings();
    let cats = hub(&app).cur.as_ref().map(|s| s.categories.clone()).unwrap_or_default();
    if flag(&p, "remember") {
        if let Some(dir) = &dir {
            let mut s = settings.clone();
            let key = cats.iter().find(|c| **c == get(&p, "category")).cloned().unwrap_or_else(|| "General".into());
            s.category_dirs.insert(key, dir.clone());
            let _ = app.mgr.set_settings(s);
        }
    }
    let req = {
        let mut h = hub(&app);
        let Some(sess) = h.cur.as_mut() else { return closing(&app, "d-add") };
        let probe = if sess.probe_url == url { sess.probe.clone() } else { None };
        // the video extras belong to the original link only
        let same = sess.open.url == url;
        let extra_ok = same;
        // "Download Later" puts it in the chosen queue
        let queue = if start { None } else { get(&p, "queue").parse::<u64>().ok().filter(|q| settings.queue(*q).is_some()) };
        let description = get(&p, "description");
        let mut headers = sess.open.headers.clone();
        headers.auth = (flag(&p, "uselogin") && !get(&p, "user").is_empty()).then(|| (get(&p, "user"), p.get("password").cloned().unwrap_or_default()));
        NewDownload {
            url: url.clone(),
            file_name: name,
            save_dir: dir,
            headers,
            connections: Some(get(&p, "connections").parse::<u32>().unwrap_or(1).clamp(1, 64)),
            probe,
            audio_url: if extra_ok { sess.open.audio_url.clone() } else { None },
            source_page: if extra_ok { sess.open.source_page.clone() } else { None },
            source_key: if extra_ok { sess.open.source_key.clone() } else { None },
            queue,
            description: (!description.is_empty()).then_some(description),
            pick_quality: None,
            early: false,
            start,
        }
    };
    // the window is done
    let (early, origin) = {
        let mut h = hub(&app);
        let sess = h.cur.take();
        match sess {
            Some(mut s) => (s.early_id.take(), s.open.url.clone()),
            None => (None, String::new()),
        }
    };
    let _ = origin;
    let resp = closing(&app, "d-add");
    // the next waiting link (later, the window has to close first)
    {
        let a = app_arc(&app);
        a.rt.spawn({
            let a = a.clone();
            async move {
                tokio::time::sleep(std::time::Duration::from_millis(350)).await;
                open_next(&a);
            }
        });
    }
    if let Some(id) = early {
        let same = app.mgr.get(id).map(|v| v.url == url || v.parent_page.as_deref() == Some(url.as_str())).unwrap_or(false);
        if same {
            let dir = req.save_dir.clone().unwrap_or_else(|| settings.dir_for("General"));
            let name = req.file_name.clone().unwrap_or_default();
            if let Err(e) = app.mgr.retarget(id, &dir, &name, req.description.clone(), req.headers.auth.clone()) {
                app.alert(tf("Could not change where it is saved: {}", &[&format!("{e:#}")]));
            }
            if start {
                if app.mgr.get(id).map(|v| v.status == Status::Failed || v.status == Status::Paused).unwrap_or(false) {
                    app.mgr.start(id);
                }
                // finished while the window was open: "Download complete" now;
                // still running: its progress window
                if app.mgr.get(id).map(|v| v.status == Status::Completed).unwrap_or(false) {
                    if settings.sounds_on {
                        crate::sound::play(&settings.sound_complete);
                    }
                    if settings.show_complete_dialog {
                        super::progress::show_complete(&app, id);
                    }
                } else {
                    super::actions::show_progress_for_start(&app, id);
                }
            } else {
                // Download Later: stop it and put it in the queue
                app.mgr.pause(id);
                if let Some(q) = req.queue {
                    app.mgr.set_queue(id, Some(q));
                }
            }
            return resp;
        }
        app.mgr.remove(id, false);
    }
    match app.mgr.find_url(&url) {
        Some(existing) => {
            let choice = settings.duplicate_action;
            if choice == 0 {
                ask_duplicate(&app, PendingAdd { req, existing });
            } else {
                apply_duplicate(&app, PendingAdd { req, existing }, choice);
            }
        }
        None => {
            finish_add(&app, req);
        }
    }
    resp
}

/// The shared handle to the app (the window closes first, the next link opens later).
fn app_arc(app: &App) -> Arc<App> {
    app.me.lock().unwrap_or_else(|e| e.into_inner()).upgrade().expect("app alive")
}

/// Really add it; open its progress window when it starts.
fn finish_add(app: &App, req: NewDownload) -> DownloadId {
    let start = req.start;
    let id = app.mgr.add(req);
    if start {
        super::actions::show_progress_for_start(app, id);
    }
    app.wake.notify_waiters();
    id
}

/// What to do with a link that is already in the list: 1 add numbered, 2 replace the
/// old one, 3 show / resume the old one.
fn apply_duplicate(app: &App, p: PendingAdd, choice: u32) {
    match choice {
        2 => {
            app.mgr.remove(p.existing, true);
            // the old file goes away a moment later; wait for it
            let a = app_arc(app);
            a.rt.spawn({
                let a = a.clone();
                async move {
                    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
                    finish_add(&a, p.req);
                }
            });
        }
        3 => match app.mgr.get(p.existing) {
            Some(v) if v.status == Status::Completed => super::progress::show_complete(app, v.id),
            Some(v) => {
                app.mgr.start(v.id);
                super::progress::open(app, v.id);
            }
            None => {
                finish_add(app, p.req);
            }
        },
        _ => {
            finish_add(app, p.req);
        }
    }
}

fn ask_duplicate(app: &App, p: PendingAdd) {
    hub(app).dup = Some(p);
    app.open("/dlg/dup");
}

async fn dup_dialog(State(app): State<Shared>) -> Response {
    let existing = hub(&app).dup.as_ref().map(|d| d.existing);
    let Some(v) = existing.and_then(|id| app.mgr.get(id)) else { return super::actions::done(&app) };
    modal(dialog(
        "d-dup",
        &t("Duplicate download link"),
        "small",
        Some("/dlg/dup/cancel"),
        html! {
            form hx-post="/dlg/dup/ok" hx-swap="none" {
                div class="dlg-body" {
                    p class="sel-text" style="margin:0" { (t("This address is already in the download list:")) }
                    div class="note" { (icon("file")) span class="sel-text" { b { (v.file_name) } " — " (t(v.status.label())) } }
                    @for (i, label) in [(1, "Add it with a new name (numbered)"), (2, "Add it and replace the old download (overwrite the file)"), (3, "If it is finished show it, if not resume it")] {
                        label class="chk" { input type="radio" name="choice" value=(i) checked[i == 1]; span class="box" { (icon("check")) } span class="chk-l" { (t(label)) } }
                    }
                    (check("remember", &t("Remember my choice (Options > Downloads)"), false))
                }
                (footer(html! { (primary(&t("OK"))) (btn_close(&t("Cancel"))) }))
            }
        },
    ))
}

async fn dup_ok(State(app): State<Shared>, Form(p): Params) -> Response {
    let choice = get(&p, "choice").parse::<u32>().unwrap_or(1).clamp(1, 3);
    if flag(&p, "remember") {
        let mut s = app.mgr.settings();
        s.duplicate_action = choice;
        let _ = app.mgr.set_settings(s);
    }
    let pending = hub(&app).dup.take();
    let r = closing(&app, "d-dup");
    if let Some(pn) = pending {
        apply_duplicate(&app, pn, choice);
    }
    r
}

async fn dup_cancel(State(app): State<Shared>) -> Response {
    hub(&app).dup.take();
    super::actions::done(&app)
}

/// The server refused us: give the address back to the browser.
async fn use_browser(State(app): State<Shared>) -> Response {
    let url = hub(&app).cur.as_ref().map(|s| s.open.url.clone()).unwrap_or_default();
    cancel_early(&app);
    hub(&app).cur = None;
    if url.starts_with("http://") || url.starts_with("https://") {
        crate::api::leave_to_browser(&url);
        let _ = open::that(&url);
    }
    let a = app_arc(&app);
    a.rt.spawn({
        let a = a.clone();
        async move {
            tokio::time::sleep(std::time::Duration::from_millis(350)).await;
            open_next(&a);
        }
    });
    closing(&app, "d-add")
}

/// Cancel, the cross, Escape.
async fn cancel(State(app): State<Shared>) -> Response {
    cancel_early(&app);
    hub(&app).cur = None;
    let a = app_arc(&app);
    a.rt.spawn({
        let a = a.clone();
        async move {
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            open_next(&a);
        }
    });
    super::actions::done(&app)
}
