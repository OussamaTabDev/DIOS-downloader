//! What the buttons, menus and keys of the main window do.
use super::ui::*;
use super::Shared;
use crate::clipboard;
use crate::i18n::{t, tf};
use crate::state::{App, ShellCmd};
use crate::tray::TrayAction;
use axum::extract::{Form, Path, State};
use axum::http::{HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::Router;
use batiie_engine::config::{AfterDone, MAIN_QUEUE};
use batiie_engine::{DownloadId, DownloadKind, DownloadView, NewDownload, RequestHeaders, Status};
use maud::{html, Markup};
use std::collections::HashMap;

pub type Params = Form<HashMap<String, String>>;

pub fn routes(r: Router<Shared>) -> Router<Shared> {
    r.route("/view", post(view))
        .route("/tree", post(tree))
        .route("/search", post(search))
        .route("/sort", post(sort))
        .route("/sel/click", post(sel_click))
        .route("/sel/all", post(sel_all))
        .route("/sel/key", post(sel_key))
        .route("/ctx", post(context_menu))
        .route("/act/queue/{op}", post(act_queue))
        .route("/act/to-queue", post(to_queue))
        .route("/act/alldone/{n}", post(all_done))
        .route("/act/theme/{n}", post(theme))
        .route("/act/lang/{code}", post(language))
        .route("/act/toolbar/{size}", post(toolbar_size))
        .route("/act/sortby/{key}", post(sort_by))
        .route("/act/sortdir/{dir}", post(sort_dir))
        .route("/act/{name}", post(act))
        .route("/do/{name}", post(do_it))
        .route("/pick/{kind}", post(pick))
        .route("/drop", post(drop_text))
}

// ---------------------------------------------------------------- helpers

pub fn done(app: &App) -> Response {
    app.wake.notify_waiters();
    StatusCode::NO_CONTENT.into_response()
}

fn hdr(r: &mut Response, k: &'static str, v: &str) {
    if let Ok(v) = HeaderValue::from_str(v) {
        r.headers_mut().insert(HeaderName::from_static(k), v);
    }
}

/// A dialog answer: it is added to the dialog area of the page, whatever
/// element asked for it.
pub fn modal(m: Markup) -> Response {
    let mut r = m.into_response();
    // the server may turn this answer into a window of its own (see windows.rs)
    hdr(&mut r, "x-batiie-dialog", "1");
    hdr(&mut r, "hx-retarget", "#modal-host");
    hdr(&mut r, "hx-reswap", "beforeend");
    r
}

pub fn closing(app: &App, id: &str) -> Response {
    app.wake.notify_waiters();
    let mut r = StatusCode::NO_CONTENT.into_response();
    hdr(&mut r, "hx-trigger", &format!(r#"{{"dlgClose":"{id}"}}"#));
    r
}

pub fn get(p: &HashMap<String, String>, k: &str) -> String {
    p.get(k).map(|s| s.trim().to_string()).unwrap_or_default()
}

pub fn flag(p: &HashMap<String, String>, k: &str) -> bool {
    p.get(k).map(|v| !v.is_empty() && v != "0" && v != "false").unwrap_or(false)
}

pub fn id_of(p: &HashMap<String, String>) -> Option<DownloadId> {
    p.get("id").and_then(|s| s.parse().ok())
}

/// A question with two buttons.
pub fn confirm(id: &str, title: &str, text: &str, ok: &str, post: &str, vals: serde_json::Value) -> Markup {
    dialog(
        id,
        title,
        "small",
        None,
        html! {
            div class="dlg-body ask" {
                span class="ask-i" { (icon("help")) }
                div class="ask-t" { @for l in text.lines() { @if l.is_empty() { br; } @else { p { (l) } } } }
            }
            (footer(html! {
                button type="button" class="btn primary" autofocus hx-post=(post) hx-vals=(vals.to_string()) hx-swap="none" { (ok) }
                (btn_close(&t("Cancel")))
            }))
        },
    )
}

/// The downloads an action works on: the whole selection when the clicked one is
/// part of it, otherwise only that one. In list order.
pub fn targets(app: &App, id: Option<DownloadId>) -> Vec<DownloadId> {
    let snap = super::page::snap(app);
    let sel = app.ui().sel.clone();
    match id {
        Some(id) if sel.contains(&id) && sel.len() > 1 => {
            let v: Vec<DownloadId> = snap.all.iter().map(|v| v.id).filter(|x| sel.contains(x)).collect();
            if v.is_empty() { sel } else { v }
        }
        Some(id) => vec![id],
        None => snap.all.iter().map(|v| v.id).filter(|x| sel.contains(x)).collect(),
    }
}

// ---------------------------------------------------------------- selection and view

async fn view(State(app): State<Shared>, Form(p): Params) -> Response {
    let v = get(&p, "v");
    if !v.is_empty() {
        let mut ui = app.ui();
        ui.view = v;
        ui.sel.clear();
        ui.focus = None;
    }
    done(&app)
}

async fn tree(State(app): State<Shared>, Form(p): Params) -> Response {
    {
        let mut ui = app.ui();
        match get(&p, "k").as_str() {
            "all" => ui.open_all = !ui.open_all,
            "unf" => ui.open_unf = !ui.open_unf,
            "fin" => ui.open_fin = !ui.open_fin,
            "queues" => ui.open_queues = !ui.open_queues,
            _ => {}
        }
    }
    done(&app)
}

async fn search(State(app): State<Shared>, Form(p): Params) -> Response {
    app.ui().search = p.get("q").cloned().unwrap_or_default();
    done(&app)
}

async fn sort(State(app): State<Shared>, Form(p): Params) -> Response {
    let k = get(&p, "k");
    let mut s = app.mgr.settings();
    if s.sort_by == k {
        s.sort_desc = !s.sort_desc;
    } else {
        s.sort_by = k;
        s.sort_desc = false;
    }
    let _ = app.mgr.set_settings(s);
    done(&app)
}

async fn sort_by(State(app): State<Shared>, Path(key): Path<String>) -> Response {
    let mut s = app.mgr.settings();
    s.sort_by = key;
    let _ = app.mgr.set_settings(s);
    done(&app)
}

async fn sort_dir(State(app): State<Shared>, Path(dir): Path<String>) -> Response {
    let mut s = app.mgr.settings();
    s.sort_desc = dir == "desc";
    let _ = app.mgr.set_settings(s);
    done(&app)
}

/// Click on a row: alone, Ctrl = add / remove, Shift = range from the last click.
async fn sel_click(State(app): State<Shared>, Form(p): Params) -> Response {
    let Some(id) = id_of(&p) else { return done(&app) };
    let (ctrl, shift) = (flag(&p, "ctrl"), flag(&p, "shift"));
    let order: Vec<u64> = super::page::snap(&app).shown.iter().map(|v| v.id).collect();
    {
        let mut ui = app.ui();
        ui.focus = Some(id);
        if shift {
            let anchor = ui.anchor.filter(|a| order.contains(a)).unwrap_or(id);
            let a = order.iter().position(|x| *x == anchor).unwrap_or(0);
            let b = order.iter().position(|x| *x == id).unwrap_or(0);
            let range: Vec<u64> = order[a.min(b)..=a.max(b)].to_vec();
            if ctrl {
                for r in range {
                    if !ui.sel.contains(&r) {
                        ui.sel.push(r);
                    }
                }
            } else {
                ui.sel = range;
            }
        } else if ctrl {
            if let Some(i) = ui.sel.iter().position(|x| *x == id) {
                ui.sel.remove(i);
            } else {
                ui.sel.push(id);
            }
            ui.anchor = Some(id);
        } else {
            ui.sel = vec![id];
            ui.anchor = Some(id);
        }
    }
    done(&app)
}

async fn sel_all(State(app): State<Shared>) -> Response {
    select_all(&app);
    done(&app)
}

pub fn select_all(app: &App) {
    let ids: Vec<u64> = super::page::snap(app).shown.iter().map(|v| v.id).collect();
    let mut ui = app.ui();
    ui.sel = ids;
}

/// Arrow keys, Home / End, PageUp / PageDown.
async fn sel_key(State(app): State<Shared>, Form(p): Params) -> Response {
    let order: Vec<u64> = super::page::snap(&app).shown.iter().map(|v| v.id).collect();
    if order.is_empty() {
        return done(&app);
    }
    let shift = flag(&p, "shift");
    {
        let mut ui = app.ui();
        let cur = ui.focus.and_then(|f| order.iter().position(|x| *x == f));
        let last = order.len() - 1;
        let idx = match (get(&p, "k").as_str(), cur) {
            ("up", Some(i)) => i.saturating_sub(1),
            ("up", None) => last,
            ("down", Some(i)) => (i + 1).min(last),
            ("down", None) => 0,
            ("home", _) => 0,
            ("end", _) => last,
            ("pgup", Some(i)) => i.saturating_sub(10),
            ("pgdn", Some(i)) => (i + 10).min(last),
            (_, i) => i.unwrap_or(0),
        };
        let id = order[idx];
        ui.focus = Some(id);
        if shift {
            let anchor = ui.anchor.filter(|a| order.contains(a)).unwrap_or(id);
            let a = order.iter().position(|x| *x == anchor).unwrap_or(0);
            ui.sel = order[a.min(idx)..=a.max(idx)].to_vec();
        } else {
            ui.sel = vec![id];
            ui.anchor = Some(id);
        }
    }
    done(&app)
}

async fn context_menu(State(app): State<Shared>, Form(p): Params) -> Response {
    let Some(id) = id_of(&p) else { return done(&app) };
    {
        let mut ui = app.ui();
        if !ui.sel.contains(&id) {
            ui.sel = vec![id];
            ui.anchor = Some(id);
        }
        ui.focus = Some(id);
    }
    app.wake.notify_waiters();
    let x: i32 = get(&p, "x").parse().unwrap_or(0);
    let y: i32 = get(&p, "y").parse().unwrap_or(0);
    let mut r = super::menus::row_menu(&app, id, x, y).into_response();
    hdr(&mut r, "hx-retarget", "#ctx");
    hdr(&mut r, "hx-reswap", "innerHTML");
    r
}

// ---------------------------------------------------------------- the actions

pub fn reveal_in_folder(v: &DownloadView) {
    let dir = v.save_path.parent().map(|p| p.to_path_buf()).unwrap_or_default();
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        if v.status == Status::Completed && v.save_path.exists() {
            let _ = std::process::Command::new(batiie_engine::system::windows_tool("explorer.exe"))
                .raw_arg(format!("/select,\"{}\"", v.save_path.display()))
                .spawn();
            return;
        }
    }
    let _ = std::fs::create_dir_all(&dir);
    let _ = open::that(&dir);
}

pub fn open_with(path: &std::path::Path) {
    #[cfg(windows)]
    {
        let _ = std::process::Command::new(batiie_engine::system::windows_tool("rundll32.exe"))
            .arg("shell32.dll,OpenAs_RunDLL")
            .arg(path)
            .spawn();
    }
    #[cfg(not(windows))]
    {
        let _ = open::that(path);
    }
}

fn sha256_of(path: &std::path::Path) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

pub fn set_clipboard(text: String) {
    if let Ok(mut c) = arboard::Clipboard::new() {
        let _ = c.set_text(text);
    }
    clipboard::mark_self_copy();
}

/// The progress window of a download that was just started (Options > Downloads).
pub fn show_progress_for_start(app: &App, id: DownloadId) {
    if app.mgr.peek(|s| s.progress_mode) <= 1 {
        super::progress::open(app, id);
    }
}

async fn act(State(app): State<Shared>, Path(name): Path<String>, Form(p): Params) -> Response {
    let id = id_of(&p);
    let ids = targets(&app, id);
    match name.as_str() {
        "resume" => {
            for &i in &ids {
                app.mgr.start(i);
            }
            if ids.len() <= 8 {
                for &i in &ids {
                    show_progress_for_start(&app, i);
                }
            }
        }
        "stop" => ids.iter().for_each(|&i| app.mgr.pause(i)),
        "stopall" => app.mgr.pause_all(),
        "resumeall" => app.mgr.start_all(),
        "restart" => {
            if ids.is_empty() {
                return done(&app);
            }
            let text = if ids.len() == 1 {
                let name = app.mgr.get(ids[0]).map(|v| v.file_name).unwrap_or_default();
                tf("Download “{}” again from the beginning?", &[&name])
            } else {
                tf("Download these {} files again from the beginning?", &[&ids.len()])
            };
            let csv = ids.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",");
            return modal(confirm("c-restart", &t("Redownload"), &text, &t("OK"), "/do/restart", serde_json::json!({ "ids": csv })));
        }
        "delete" => return delete_dialog(&app, &ids),
        "deletedone" => {
            let n = app.mgr.snapshot().iter().filter(|v| v.status == Status::Completed).count();
            if n == 0 {
                return done(&app);
            }
            return modal(confirm(
                "c-deldone",
                &t("Delete all completed"),
                &tf("Remove the {} finished downloads from the download list?\n\nOnly the list entries are removed: the downloaded files stay on your computer.", &[&n]),
                &t("OK"),
                "/do/deletedone",
                serde_json::json!({}),
            ));
        }
        "copy" => {
            let text: Vec<String> = ids.iter().filter_map(|&i| app.mgr.get(i)).map(|v| v.url).collect();
            if !text.is_empty() {
                set_clipboard(text.join("\r\n"));
                app.toast(t("Address copied"));
            }
        }
        "copyname" | "copypath" => {
            let text: Vec<String> = ids
                .iter()
                .filter_map(|&i| app.mgr.get(i))
                .map(|v| if name == "copyname" { v.file_name } else { v.save_path.display().to_string() })
                .collect();
            if !text.is_empty() {
                set_clipboard(text.join("\r\n"));
                app.toast(t("Copied"));
            }
        }
        "open" | "folder" | "openwith" | "sha256" | "move" | "properties" | "refresh" | "torrent" | "progress" => {
            let Some(id) = id.or_else(|| ids.first().copied()) else { return done(&app) };
            return one(&app, id, &name);
        }
        "activate" => {
            let Some(id) = id else { return done(&app) };
            let Some(v) = app.mgr.get(id) else { return done(&app) };
            if app.mgr.peek(|s| s.double_click) == 1 {
                return one(&app, id, "properties");
            } else if v.status == Status::Completed {
                let _ = open::that(&v.save_path);
            } else {
                super::progress::open(&app, id);
            }
        }
        "paste" => paste_from_clipboard(&app),
        "limiter" => {
            let mut s = app.mgr.settings();
            s.speed_limit_on = !s.speed_limit_on;
            if s.speed_limit_kb == 0 {
                s.speed_limit_kb = 500;
            }
            let _ = app.mgr.set_settings(s);
        }
        "limiter-on" | "limiter-off" => {
            let mut s = app.mgr.settings();
            s.speed_limit_on = name == "limiter-on";
            if s.speed_limit_on && s.speed_limit_kb == 0 {
                s.speed_limit_kb = 500;
            }
            let _ = app.mgr.set_settings(s);
        }
        "hide" => {
            app.ui().hidden = true;
            app.shell(ShellCmd::Hide);
        }
        "exit" => {
            app.shell(ShellCmd::Quit);
        }
        "selectall" => select_all(&app),
        "hidecats" => {
            let mut s = app.mgr.settings();
            s.hide_categories = !s.hide_categories;
            let _ = app.mgr.set_settings(s);
        }
        _ => return StatusCode::NOT_FOUND.into_response(),
    }
    done(&app)
}

/// Actions on one download.
fn one(app: &App, id: DownloadId, name: &str) -> Response {
    let Some(v) = app.mgr.get(id) else { return done(app) };
    match name {
        "open" => {
            let _ = open::that(&v.save_path);
        }
        "folder" => reveal_in_folder(&v),
        "openwith" => open_with(&v.save_path),
        "refresh" => {
            // a torrent keeps its magnet / .torrent file: nothing to refresh
            if v.kind != DownloadKind::Torrent {
                return super::misc::refresh_dialog(app, id);
            }
        }
        "move" => {
            if v.status == Status::Completed {
                let dlg = rfd::AsyncFileDialog::new().set_file_name(&v.file_name);
                let dlg = match v.save_path.parent() {
                    Some(dir) => dlg.set_directory(dir),
                    None => dlg,
                };
                let app2 = app.mgr.clone();
                let toast = app.bus.clone();
                app.rt.spawn(async move {
                    if let Some(h) = dlg.save_file().await {
                        if let Err(e) = app2.move_file(id, h.path()) {
                            let _ = toast.send(crate::state::Push::Alert(tf("Could not move the file:\n\n{}", &[&format!("{e:#}")])));
                        }
                    }
                });
            } else {
                return super::misc::properties_dialog(app, id);
            }
        }
        "sha256" => {
            let (path, name, bus) = (v.save_path.clone(), v.file_name.clone(), app.bus.clone());
            std::thread::spawn(move || {
                let text = match sha256_of(&path) {
                    Ok(hash) => {
                        set_clipboard(hash.clone());
                        tf("SHA-256 of {}:\n\n{}\n\n(copied to the clipboard)", &[&name, &hash])
                    }
                    Err(e) => tf("Could not read the file: {}", &[&e]),
                };
                let _ = bus.send(crate::state::Push::Alert(text));
            });
        }
        "properties" => return super::misc::properties_dialog(app, id),
        "torrent" | "progress" => {
            super::progress::open(app, id);
        }
        _ => {}
    }
    done(app)
}

fn delete_dialog(app: &App, ids: &[DownloadId]) -> Response {
    if ids.is_empty() {
        return done(app);
    }
    let csv = ids.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",");
    let (title, text) = if ids.len() == 1 {
        let Some(v) = app.mgr.get(ids[0]) else { return done(app) };
        if v.status == Status::Completed {
            (
                t("Delete download"),
                tf("Remove “{}” from the download list?\n\nOnly the list entry is removed: the file stays on your computer.", &[&v.file_name]),
            )
        } else {
            (
                t("Delete download"),
                tf("Delete the unfinished download “{}”?\nThe partly downloaded data will be removed.", &[&v.file_name]),
            )
        }
    } else {
        (
            t("Delete downloads"),
            tf("Remove these {} downloads from the download list?\n\nOnly the list entries are removed: the downloaded files stay on your computer.\n(Parts of unfinished downloads are cleaned up.)", &[&ids.len()]),
        )
    };
    modal(confirm("c-delete", &title, &text, &t("OK"), "/do/delete", serde_json::json!({ "ids": csv })))
}

fn ids_of(p: &HashMap<String, String>) -> Vec<DownloadId> {
    get(p, "ids").split(',').filter_map(|s| s.trim().parse().ok()).collect()
}

/// The answers of the confirmation windows.
async fn do_it(State(app): State<Shared>, Path(name): Path<String>, Form(p): Params) -> Response {
    let ids = ids_of(&p);
    match name.as_str() {
        "delete" => {
            match ids.len() {
                0 => {}
                1 => app.mgr.remove(ids[0], false),
                _ => app.mgr.remove_many(&ids),
            }
            let mut ui = app.ui();
            ui.sel.retain(|s| !ids.contains(s));
            drop(ui);
            for id in &ids {
                super::progress::close_window(&app, *id);
            }
            return closing(&app, "c-delete");
        }
        "restart" => {
            ids.iter().for_each(|&i| app.mgr.restart(i));
            return closing(&app, "c-restart");
        }
        "deletedone" => {
            app.mgr.remove_completed();
            return closing(&app, "c-deldone");
        }
        _ => {}
    }
    StatusCode::NOT_FOUND.into_response()
}

async fn act_queue(State(app): State<Shared>, Path(op): Path<String>, Form(p): Params) -> Response {
    let view = app.ui().view.clone();
    let q = get(&p, "q").parse::<u64>().ok().or_else(|| view.strip_prefix("queue:").and_then(|q| q.parse().ok()));
    let q = q.filter(|q| app.mgr.peek(|s| s.queue(*q).is_some())).unwrap_or(MAIN_QUEUE);
    match op.as_str() {
        "start" => app.mgr.start_queue(q),
        "stop" => app.mgr.stop_queue(q),
        _ => {}
    }
    done(&app)
}

async fn to_queue(State(app): State<Shared>, Form(p): Params) -> Response {
    let q = get(&p, "q").parse::<u64>().ok().filter(|q| *q > 0);
    for id in targets(&app, id_of(&p)) {
        app.mgr.set_queue(id, q);
    }
    done(&app)
}

async fn all_done(State(app): State<Shared>, Path(n): Path<i32>) -> Response {
    app.mgr.set_when_all_done(AfterDone::from_index(n));
    done(&app)
}

async fn theme(State(app): State<Shared>, Path(n): Path<u32>) -> Response {
    let mut s = app.mgr.settings();
    s.theme = n.min(8);
    let _ = app.mgr.set_settings(s);
    done(&app)
}

async fn language(State(app): State<Shared>, Path(code): Path<String>) -> Response {
    let mut s = app.mgr.settings();
    s.language = if code == "auto" { String::new() } else { code };
    let eff = crate::i18n::effective(&s.language).to_string();
    let _ = app.mgr.set_settings(s);
    crate::i18n::apply(&eff);
    // every text changes: the page is loaded again
    app.push(crate::state::Push::Open("reload".into()));
    done(&app)
}

async fn toolbar_size(State(app): State<Shared>, Path(size): Path<String>) -> Response {
    let mut s = app.mgr.settings();
    s.toolbar_small = size == "small";
    let _ = app.mgr.set_settings(s);
    done(&app)
}

// ---------------------------------------------------------------- the native pickers

/// "Browse…": the system's folder / file window. The answer is the text box again.
async fn pick(State(app): State<Shared>, Path(kind): Path<String>, Form(p): Params) -> Response {
    let name = get(&p, "name");
    let current = p.get(&name).cloned().unwrap_or_default();
    let start = std::path::PathBuf::from(&current);
    let mut dlg = rfd::AsyncFileDialog::new();
    let dir = if start.is_dir() { Some(start.clone()) } else { start.parent().filter(|d| d.is_dir()).map(|d| d.to_path_buf()) };
    if let Some(d) = dir {
        dlg = dlg.set_directory(d);
    }
    let _ = &app;
    let picked: Option<String> = match kind.as_str() {
        "folder" => dlg.pick_folder().await.map(|h| h.path().display().to_string()),
        "file" => dlg.pick_file().await.map(|h| h.path().display().to_string()),
        "image" => dlg
            .add_filter("Pictures", &["png", "jpg", "jpeg", "webp", "bmp", "gif"])
            .pick_file()
            .await
            .map(|h| h.path().display().to_string()),
        "sound" => {
            #[cfg(windows)]
            let dlg = if current.is_empty() { dlg.set_directory(r"C:\Windows\Media") } else { dlg };
            dlg.add_filter("Sounds", &["wav"]).pick_file().await.map(|h| h.path().display().to_string())
        }
        "torrent" => dlg.add_filter("Torrent", &["torrent"]).pick_file().await.map(|h| h.path().display().to_string()),
        "save" => dlg.save_file().await.map(|h| h.path().display().to_string()),
        _ => None,
    };
    let value = picked.unwrap_or(current);
    // the picture behind the window shows at once (Options > Appearance)
    let live = name == "bg_image";
    html! {
        input class="in" type="text" name=(name) value=(value) autocomplete="off" spellcheck="false"
            hx-post=[live.then_some("/dlg/settings/preview")] hx-trigger=[live.then_some("change")]
            hx-include=[live.then_some("#st-form [name=theme],#st-form [name=text_size],#st-form [name=font],#st-form [name=bg_image],#st-form [name=bg_strength]")] hx-swap=[live.then_some("none")]
            data-fire=[live.then_some("1")];
    }
    .into_response()
}

// ---------------------------------------------------------------- paste and drop

/// What the clipboard (or a drop) holds.
#[derive(Debug, PartialEq, Clone)]
pub enum Clip {
    Magnet(String),
    Torrent(std::path::PathBuf),
    Links(Vec<String>),
    Nothing,
}

pub fn classify(text: &str) -> Clip {
    let t = text.trim();
    if t.is_empty() || t.len() > 4096 {
        return Clip::Nothing;
    }
    if let Some(u) = is_link(t) {
        return Clip::Links(vec![u]);
    }
    if t.starts_with("magnet:") && t.len() < 4096 {
        return Clip::Magnet(t.to_string());
    }
    if is_info_hash(t) {
        return Clip::Magnet(format!("magnet:?xt=urn:btih:{t}"));
    }
    let path = t.trim_matches('"');
    if !path.contains('\n') && path.to_ascii_lowercase().ends_with(".torrent") && std::path::Path::new(path).is_file() {
        return Clip::Torrent(std::path::PathBuf::from(path));
    }
    let links = extract_links(t);
    if links.len() > 1 {
        return Clip::Links(links);
    }
    Clip::Nothing
}

pub fn is_info_hash(s: &str) -> bool {
    s.len() == 40 && s.chars().all(|c| c.is_ascii_hexdigit())
}

fn is_link(t: &str) -> Option<String> {
    let s = t.trim_matches(|c| c == '"' || c == '\'' || c == '<' || c == '>');
    (s.starts_with("http://") || s.starts_with("https://"))
        .then(|| s.to_string())
        .filter(|s| !s.contains(char::is_whitespace) && url::Url::parse(s).is_ok())
}

/// Every http(s) link in a text (separated by spaces or lines, quotes stripped).
pub fn extract_links(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for l in text
        .split_whitespace()
        .map(|l| l.trim_matches(|c| c == '"' || c == '\'' || c == '<' || c == '>' || c == ','))
        .filter(|l| (l.starts_with("http://") || l.starts_with("https://")) && url::Url::parse(l).is_ok())
    {
        out.push(l.to_string());
    }
    out
}

pub fn paste_from_clipboard(app: &App) {
    let text = match arboard::Clipboard::new() {
        Ok(mut c) => c.get_text().unwrap_or_default(),
        Err(_) => {
            app.toast(t("The clipboard is not available"));
            return;
        }
    };
    dispatch(app, &text);
}

async fn drop_text(State(app): State<Shared>, Form(p): Params) -> Response {
    let text = p.get("text").cloned().unwrap_or_default();
    dispatch(&app, &text);
    done(&app)
}

fn dispatch(app: &App, text: &str) {
    match classify(text) {
        Clip::Magnet(m) => super::torrent::open_add(app, Some(batiie_engine::TorrentSource::Magnet(m))),
        Clip::Torrent(p) => super::torrent::open_add(app, Some(batiie_engine::TorrentSource::Torrent(p))),
        Clip::Links(links) => {
            if links.len() > 1 {
                super::batch::open_with(app, text.to_string());
                app.toast(tf("{} links pasted — pick the folder and start them", &[&links.len()]));
                return;
            }
            one_link(app, links.into_iter().next().unwrap_or_default());
        }
        Clip::Nothing => app.toast(t("No download link found in the clipboard")),
    }
}

fn filename_from(url: &str) -> String {
    batiie_engine::filename::from_url(url)
        .map(|n| batiie_engine::filename::sanitize(&n))
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| url.to_string())
}

/// One link: a direct file goes to the list at once; a page opens in the Add window;
/// an old friend is brought back instead of added twice.
fn one_link(app: &App, url: String) {
    if let Some(id) = app.mgr.find_url(&url) {
        if let Some(v) = app.mgr.get(id) {
            if v.status.is_running() || v.status == Status::Completed {
                app.toast(t("This link is already in the download list"));
            } else {
                app.mgr.start(id);
                super::progress::open(app, id);
                app.toast(t("This link is already in the list — it continues now"));
            }
        }
        return;
    }
    if clipboard::downloadable_url(&url).is_some() {
        let id = app.mgr.add(NewDownload { url: url.clone(), start: true, ..Default::default() });
        show_progress_for_start(app, id);
        app.toast(tf("Download added from the clipboard: {}", &[&filename_from(&url)]));
        return;
    }
    super::add::request(app, super::add::AddOpen::url(url));
    app.toast(t("Link pasted — press Start to download it"));
}

// ---------------------------------------------------------------- tray and the browser add-in

/// A click in the tray menu. True = leave the program.
pub fn tray_action(app: &App, action: TrayAction) -> bool {
    match action {
        TrayAction::Show => {
            app.ui().hidden = false;
            app.shell(ShellCmd::Show);
        }
        TrayAction::AddUrl => {
            app.shell(ShellCmd::Show);
            super::add::request(app, super::add::AddOpen::url(String::new()));
        }
        TrayAction::ResumeAll => app.mgr.start_all(),
        TrayAction::StopAll => app.mgr.pause_all(),
        TrayAction::ToggleLimiter => {
            let mut s = app.mgr.settings();
            s.speed_limit_on = !s.speed_limit_on;
            if s.speed_limit_kb == 0 {
                s.speed_limit_kb = 500;
            }
            let _ = app.mgr.set_settings(s);
        }
        TrayAction::Scheduler => {
            app.shell(ShellCmd::Show);
            app.open("/dlg/scheduler");
        }
        TrayAction::Options => {
            app.shell(ShellCmd::Show);
            app.open("/dlg/settings");
        }
        TrayAction::About => {
            app.shell(ShellCmd::Show);
            app.open("/dlg/about");
        }
        TrayAction::Exit => return true,
    }
    app.wake.notify_waiters();
    false
}

/// A request from the browser add-in.
pub fn handle_api(app: &App, event: crate::api::ApiEvent) {
    use crate::api::ApiEvent;
    match event {
        ApiEvent::Show => {
            app.ui().hidden = false;
            app.shell(ShellCmd::Show);
        }
        ApiEvent::Torrent(text) => {
            app.shell(ShellCmd::Show);
            if let Some(source) = torrent_source_of(&text) {
                super::torrent::open_add(app, Some(source));
            }
        }
        ApiEvent::Links(l) => super::misc::open_links(app, l),
        ApiEvent::Add(req) => {
            // the same address again within a few seconds (an older add-in could send
            // a download over and over): show nothing new
            {
                let mut last = app.clip_last.lock().unwrap_or_else(|e| e.into_inner());
                let now = app.started.elapsed().as_secs();
                let key = format!("{}|{}", req.url, now / 10);
                let prev = format!("{}|{}", req.url, now.saturating_sub(1) / 10);
                if *last == key || *last == prev {
                    return;
                }
                *last = key;
            }
            if req.url.starts_with("magnet:") {
                super::torrent::open_add(app, Some(batiie_engine::TorrentSource::Magnet(req.url)));
                return;
            }
            let mut headers = RequestHeaders { referer: req.referrer, cookies: req.cookies, user_agent: req.user_agent, extra: Vec::new(), auth: None };
            if let Some(list) = &req.headers {
                batiie_engine::ytdlp::apply_headers(&mut headers, list);
            }
            // "Refresh download address": this is the new link we waited for
            if let Some(id) = app.misc.lock().unwrap_or_else(|e| e.into_inner()).refresh_id {
                if req.audio_url.is_none() {
                    super::misc::refresh_received(app, id, &req.url, headers);
                    return;
                }
            }
            let open = super::add::AddOpen {
                url: req.url.clone(),
                headers: headers.clone(),
                file_name: req.file_name.clone(),
                audio_url: req.audio_url.clone(),
                source_page: req.source_page.clone(),
                source_key: req.source_key.clone(),
            };
            let s = app.mgr.settings();
            if !s.show_start_dialog && app.mgr.find_url(&req.url).is_none() {
                let id = app.mgr.add(NewDownload {
                    url: req.url,
                    file_name: req.file_name,
                    headers,
                    audio_url: req.audio_url,
                    source_page: req.source_page,
                    source_key: req.source_key,
                    start: true,
                    ..Default::default()
                });
                show_progress_for_start(app, id);
                return;
            }
            app.shell(ShellCmd::Show);
            super::add::request(app, open);
        }
    }
    app.wake.notify_waiters();
}

pub fn torrent_source_of(text: &str) -> Option<batiie_engine::TorrentSource> {
    let t = text.trim();
    if t.to_ascii_lowercase().starts_with("magnet:") {
        Some(batiie_engine::TorrentSource::Magnet(t.to_string()))
    } else if t.to_ascii_lowercase().ends_with(".torrent") && std::path::Path::new(t).is_file() {
        Some(batiie_engine::TorrentSource::Torrent(std::path::PathBuf::from(t)))
    } else {
        None
    }
}

/// What a link (or magnet) copied somewhere else does: a magnet opens the torrent
/// window, a link follows Options > Monitoring.
pub fn clipboard_pickup(app: &App, pickup: String) {
    if pickup.starts_with("magnet:") {
        super::torrent::open_add(app, Some(batiie_engine::TorrentSource::Magnet(pickup)));
        return;
    }
    if app.mgr.peek(|s| s.auto_start_clipboard) {
        if app.mgr.find_url(&pickup).is_none() {
            app.mgr.add(NewDownload { url: pickup, start: true, ..Default::default() });
            app.wake.notify_waiters();
        }
        return;
    }
    if !super::add::is_busy(app) {
        super::add::request(app, super::add::AddOpen::url(pickup));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn magnets_and_hashes() {
        assert_eq!(
            classify("magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567&dn=x"),
            Clip::Magnet("magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567&dn=x".into())
        );
        assert_eq!(
            classify("0123456789ABCDEF0123456789ABCDEF01234567"),
            Clip::Magnet("magnet:?xt=urn:btih:0123456789ABCDEF0123456789ABCDEF01234567".into())
        );
        assert_eq!(classify("0123456789abcdef0123456789abcdef0123456"), Clip::Nothing);
    }

    #[test]
    fn links() {
        assert_eq!(classify("https://example.com/setup.exe"), Clip::Links(vec!["https://example.com/setup.exe".into()]));
        assert_eq!(classify("  <https://example.com/a.mp4>  "), Clip::Links(vec!["https://example.com/a.mp4".into()]));
        assert_eq!(
            classify("https://a.com/1.zip\r\nhttps://a.com/2.zip"),
            Clip::Links(vec!["https://a.com/1.zip".into(), "https://a.com/2.zip".into()])
        );
        assert_eq!(classify("look at https://example.com/a.zip please"), Clip::Nothing);
        assert_eq!(classify("ftp://example.com/f.zip"), Clip::Nothing);
    }
}
