//! The smaller windows: Properties, Columns, Find, Category, Tip of the day, About,
//! Refresh download address, Export / Import of the list.
use super::actions::{closing, done, flag, get, modal, Params};
use super::page::{fmt_time, kind_for};
use super::ui::*;
use super::Shared;
use crate::i18n::{t, tf};
use crate::state::App;
use axum::extract::{Form, Path, Query, State};
use axum::response::{IntoResponse, Response};
use axum::routing::{get as route_get, post};
use axum::Router;
use batiie_engine::config::{CategoryDef, ALL_COLUMNS, DEFAULT_COLUMNS, MAIN_QUEUE};
use batiie_engine::ef2::{self, Ef2Item};
use batiie_engine::{filename, fmt_size, DetailsEdit, DownloadId, DownloadKind, NewDownload, RequestHeaders, Status};
use maud::{html, Markup};
use std::collections::HashMap;
use std::path::PathBuf;

pub fn routes(r: Router<Shared>) -> Router<Shared> {
    r.route("/dlg/props/{id}", route_get(props_dialog))
        .route("/dlg/props/{id}/save", post(props_save))
        .route("/dlg/columns", route_get(columns_dialog))
        .route("/dlg/columns/save", post(columns_save))
        .route("/dlg/columns/reset", post(columns_reset))
        .route("/cols/width", post(col_width))
        .route("/cols/move", post(col_move))
        .route("/dlg/find", route_get(find_dialog))
        .route("/dlg/find/next", post(find_next))
        .route("/act/findnext", post(find_next_key))
        .route("/dlg/category/{name}", route_get(category_dialog))
        .route("/dlg/category/{name}/save", post(category_save))
        .route("/dlg/category/{name}/delete", post(category_delete))
        .route("/dlg/tips", route_get(tips_dialog))
        .route("/dlg/tips/next", post(tips_next))
        .route("/dlg/tips/close", post(tips_close))
        .route("/dlg/about", route_get(about_dialog))
        .route("/dlg/shortcuts", route_get(shortcuts_dialog))
        .route("/act/openurl", post(open_url))
        .route("/dlg/refresh/{id}", route_get(refresh_dialog_route))
        .route("/dlg/refresh/use", post(refresh_use))
        .route("/dlg/refresh/stop", post(refresh_stop))
        .route("/dlg/export", route_get(export_dialog))
        .route("/dlg/import", route_get(import_dialog))
        .route("/dlg/export/{kind}", post(export_do))
        .route("/dlg/import/{kind}", post(import_do))
}

pub fn is_web_address(s: &str) -> bool {
    url::Url::parse(s.trim())
        .map(|u| matches!(u.scheme(), "http" | "https") && u.host_str().is_some_and(|h| !h.is_empty()))
        .unwrap_or(false)
}

/// Only web pages are ever opened from a link inside the program: an address that
/// came from a browser or a list file must never start a program.
async fn open_url(State(app): State<Shared>, Form(p): Params) -> Response {
    let url = get(&p, "url");
    if is_web_address(&url) {
        let _ = open::that(url);
    }
    done(&app)
}

// ---------------------------------------------------------------- properties

pub fn properties_dialog(app: &App, id: DownloadId) -> Response {
    let Some(v) = app.mgr.get(id) else { return done(app) };
    let done_ = v.status == Status::Completed;
    let ext = filename::extension(&v.file_name).to_uppercase();
    let type_text = if ext.is_empty() { t(&v.category) } else { tf("{} file  ·  {}", &[&ext, &t(&v.category)]) };
    let status = match v.status {
        Status::Downloading | Status::Paused if v.total_size.is_some() => format!("{} ({:.1}%)", t(v.status.label()), v.progress() * 100.0),
        s => t(s.label()),
    };
    let size = match v.total_size {
        Some(n) => format!("{} ({} bytes)", fmt_size(n), n),
        None => t("Unknown"),
    };
    let result = match (&v.error, v.status) {
        (Some(e), _) => e.clone(),
        (None, Status::Completed) => t("The file was downloaded successfully"),
        _ => String::new(),
    };
    let kind = kind_for(&v.file_name);
    let did = format!("d-props-{id}");
    modal(dialog(
        &did,
        &tf("File Properties – {}", &[&v.file_name]),
        "wide",
        None,
        html! {
            form hx-post=(format!("/dlg/props/{id}/save")) hx-swap="none" {
                div class="dlg-body" {
                    div class="row-flex" style="align-items:center" {
                        span class={"ai-icon big k-" (kind)} { (icon(super::page::file_icon(kind))) @if !ext.is_empty() { b { (ext) } } }
                        div style="min-width:0" { div class="big-name" { (v.file_name) } div class="muted" { (type_text) } }
                    }
                    dl class="stats sel-text" {
                        dt { (t("Status")) } dd { (status) }
                        dt { (t("Size")) } dd { (size) }
                        dt { (t("Date added")) } dd { (fmt_time(v.added_at)) }
                        dt { (t("Last try date")) } dd { (v.finished_at.or(v.last_try).map(fmt_time).unwrap_or_default()) }
                    }
                    (field(&t("Save To"), html! {
                        div class="pathbox" id="p-pr-save" {
                            input class="in" type="text" name="save_to" value=(v.save_path.display().to_string()) autocomplete="off" spellcheck="false";
                            button type="button" class="btn" hx-post="/pick/save" hx-include="#p-pr-save input" hx-target="#p-pr-save input" hx-swap="outerHTML" hx-vals=r#"{"name":"save_to"}"# { (icon("folder")) }
                        }
                    }))
                    (field(&t("Address"), html! { input class="in" type="text" name="url" value=(v.url) readonly[done_] autocomplete="off" spellcheck="false"; }))
                    (field(&t("Referer"), text("referer", v.referer())))
                    (field(&t("Description"), text("description", v.description.as_deref().unwrap_or(""))))
                    label class="chk" { input type="checkbox" id="pr-login" name="uselogin" value="1" checked[v.headers.auth.is_some()]; span class="box" { (icon("check")) } span class="chk-l" { (t("Use authorization (server login)")) } }
                    div class="row-flex" data-enable="#pr-login" {
                        (field(&t("Login"), text("user", v.headers.auth.as_ref().map(|a| a.0.as_str()).unwrap_or(""))))
                        (field(&t("Password"), password("password", v.headers.auth.as_ref().map(|a| a.1.as_str()).unwrap_or(""))))
                    }
                    @if !result.is_empty() { div class="muted sel-text" { (tf("Result: {}", &[&result])) } }
                    div id="pr-msg" {}
                }
                (footer(html! {
                    @if done_ { button type="button" class="btn" hx-post="/act/open" hx-vals=(format!(r#"{{"id":{id}}}"#)) hx-swap="none" { (icon("open")) (t("Open")) } }
                    span class="sp" {}
                    (primary(&t("OK")))
                    (btn_close(&t("Cancel")))
                }))
            }
        },
    ))
}

async fn props_dialog(State(app): State<Shared>, Path(id): Path<DownloadId>) -> Response {
    properties_dialog(&app, id)
}

async fn props_save(State(app): State<Shared>, Path(id): Path<DownloadId>, Form(p): Params) -> Response {
    let Some(v) = app.mgr.get(id) else { return closing(&app, &format!("d-props-{id}")) };
    let msg = |text: String| html! { div id="pr-msg" hx-swap-oob="true" { (msg_err(&text)) } }.into_response();
    let auth = (flag(&p, "uselogin") && !get(&p, "user").is_empty()).then(|| (get(&p, "user"), p.get("password").cloned().unwrap_or_default()));
    let edit = DetailsEdit {
        url: (v.status != Status::Completed).then(|| get(&p, "url")),
        referer: Some(get(&p, "referer")),
        description: Some(get(&p, "description")),
        auth,
    };
    if let Err(e) = app.mgr.update_details(id, edit) {
        return msg(format!("{e:#}"));
    }
    let target = PathBuf::from(get(&p, "save_to"));
    if !target.as_os_str().is_empty() && target != v.save_path {
        let result = if v.status == Status::Completed {
            app.mgr.move_file(id, &target)
        } else {
            let dir = target.parent().map(|p| p.to_path_buf()).unwrap_or_default();
            let name = target.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            if name.is_empty() {
                Err(anyhow::anyhow!("type a file name"))
            } else {
                app.mgr.change_target(id, &dir, &name)
            }
        };
        if let Err(e) = result {
            return msg(tf("Could not change where it is saved: {}", &[&format!("{e:#}")]));
        }
    }
    closing(&app, &format!("d-props-{id}"))
}

// ---------------------------------------------------------------- columns

fn col_label(k: &str) -> String {
    t(match k {
        "q" => "Q (queue number)",
        "size" => "Size",
        "status" => "Status",
        "progress" => "Progress bar",
        "eta" => "Time left",
        "speed" => "Transfer rate",
        "last" => "Last Try Date",
        "desc" => "Description",
        "added" => "Date Added",
        "saveto" => "Save To",
        "referer" => "Referer",
        "parent" => "Parent Web Page",
        _ => "",
    })
}

fn columns_form(on: &[String]) -> Markup {
    html! {
        form id="cols-form" hx-post="/dlg/columns/save" hx-swap="none" {
            div class="dlg-body" {
                p class="hint" style="margin:0" { (t("Show these columns in the download list:")) }
                div class="col" {
                    label class="chk" { input type="checkbox" checked disabled; span class="box" { (icon("check")) } span class="chk-l" { (t("File Name (always shown)")) } }
                    @for k in ALL_COLUMNS {
                        label class="chk" { input type="checkbox" name=(k) value="1" checked[on.iter().any(|c| c == k)]; span class="box" { (icon("check")) } span class="chk-l" { (col_label(k)) } }
                    }
                }
            }
            (footer(html! {
                button type="button" class="btn" hx-post="/dlg/columns/reset" hx-target="#cols-form" hx-swap="outerHTML" { (t("Reset")) }
                span class="sp" {}
                (primary(&t("OK")))
                (btn_close(&t("Cancel")))
            }))
        }
    }
}

async fn columns_dialog(State(app): State<Shared>) -> Response {
    let on = app.mgr.peek(|s| s.columns.clone());
    modal(dialog("d-columns", &t("Columns"), "small", None, columns_form(&on)))
}

async fn columns_reset() -> Markup {
    let on: Vec<String> = DEFAULT_COLUMNS.iter().map(|c| c.to_string()).collect();
    columns_form(&on)
}

async fn columns_save(State(app): State<Shared>, Form(p): Params) -> Response {
    let mut s = app.mgr.settings();
    s.columns = ALL_COLUMNS.iter().filter(|k| flag(&p, k)).map(|k| k.to_string()).collect();
    let _ = app.mgr.set_settings(s);
    closing(&app, "d-columns")
}

async fn col_width(State(app): State<Shared>, Form(p): Params) -> Response {
    let key = get(&p, "key");
    let w = get(&p, "w").parse::<u32>().unwrap_or(0).clamp(36, 2000);
    if key == "name" || ALL_COLUMNS.contains(&key.as_str()) {
        let mut s = app.mgr.settings();
        s.column_widths.insert(key, w);
        let _ = app.mgr.set_settings(s);
    }
    done(&app)
}

/// A column was dragged in front of another one.
async fn col_move(State(app): State<Shared>, Form(p): Params) -> Response {
    let (key, before) = (get(&p, "key"), get(&p, "before"));
    let mut s = app.mgr.settings();
    let mut order: Vec<String> = s.column_layout().into_iter().map(|c| c.0).collect();
    // columns that are hidden keep their old place
    let hidden: Vec<String> = s.column_order.iter().filter(|k| !order.contains(k)).cloned().collect();
    if let Some(i) = order.iter().position(|k| *k == key) {
        let k = order.remove(i);
        let at = order.iter().position(|k| *k == before).unwrap_or(order.len());
        order.insert(at, k);
        order.extend(hidden);
        s.column_order = order;
        let _ = app.mgr.set_settings(s);
    }
    done(&app)
}

// ---------------------------------------------------------------- find

async fn find_dialog(State(app): State<Shared>) -> Response {
    let f = app.ui().find.clone();
    modal(dialog(
        "d-find",
        &t("Find file"),
        "small",
        None,
        html! {
            form hx-post="/dlg/find/next" hx-swap="none" {
                div class="dlg-body" {
                    (field(&t("Find what"), html! { input class="in" type="text" name="text" value=(f.text) autocomplete="off" spellcheck="false"; }))
                    (section(&t("Look in"), html! {
                        (check("in_name", &t("File name"), f.in_name))
                        (check("in_desc", &t("Description"), f.in_desc))
                        (check("in_addr", &t("Address, referer and parent web page"), f.in_addr))
                    }))
                    div class="row-flex" {
                        (check("case", &t("Match case"), f.case))
                        (check("whole", &t("Match whole string"), f.whole))
                    }
                    div id="find-msg" class="muted" style="min-height:1.5em" {}
                }
                (footer(html! {
                    span class="sp" {}
                    (primary(&t("Find Next")))
                    (btn_close(&t("Close")))
                }))
            }
        },
    ))
}

/// Select the next download after the selected one that matches.
fn do_find(app: &App) -> Result<Option<String>, ()> {
    let f = app.ui().find.clone();
    let needle = f.text.trim().to_string();
    if needle.is_empty() {
        return Err(());
    }
    let norm = |s: &str| if f.case { s.to_string() } else { s.to_lowercase() };
    let needle = norm(&needle);
    let hit = |hay: &str| {
        let hay = norm(hay);
        if f.whole { hay == needle } else { hay.contains(&needle) }
    };
    // search the whole list, in the order shown
    {
        let mut ui = app.ui();
        if ui.view != "all" && ui.search.is_empty() {
            ui.view = "all".into();
        }
    }
    let snap = super::page::snap(app);
    let ids: Vec<u64> = snap.shown.iter().map(|v| v.id).collect();
    let focus = app.ui().focus;
    let start = focus.and_then(|f| ids.iter().position(|x| *x == f)).map(|p| p + 1).unwrap_or(0);
    for k in 0..ids.len() {
        let id = ids[(start + k) % ids.len()];
        let Some(v) = snap.shown.iter().find(|v| v.id == id) else { continue };
        let found = (f.in_name && hit(&v.file_name))
            || (f.in_desc && hit(v.description.as_deref().unwrap_or("")))
            || (f.in_addr && (hit(&v.url) || hit(v.referer()) || hit(v.parent_page.as_deref().unwrap_or(""))));
        if found {
            let mut ui = app.ui();
            ui.sel = vec![id];
            ui.anchor = Some(id);
            ui.focus = Some(id);
            return Ok(Some(v.file_name.clone()));
        }
    }
    Ok(None)
}

async fn find_next(State(app): State<Shared>, Form(p): Params) -> Response {
    {
        let mut ui = app.ui();
        ui.find.text = get(&p, "text");
        ui.find.case = flag(&p, "case");
        ui.find.whole = flag(&p, "whole");
        ui.find.in_name = flag(&p, "in_name");
        ui.find.in_desc = flag(&p, "in_desc");
        ui.find.in_addr = flag(&p, "in_addr");
    }
    let text = match do_find(&app) {
        Ok(Some(n)) => tf("Found: {}", &[&n]),
        Ok(None) => t("Not found"),
        Err(()) => String::new(),
    };
    app.wake.notify_waiters();
    html! { div id="find-msg" hx-swap-oob="true" class="muted" style="min-height:1.5em" { (text) } }.into_response()
}

/// F3 / Downloads > Find next.
async fn find_next_key(State(app): State<Shared>) -> Response {
    if app.ui().find.text.trim().is_empty() {
        app.open("/dlg/find");
        return done(&app);
    }
    match do_find(&app) {
        Ok(Some(n)) => app.toast(tf("Found: {}", &[&n])),
        Ok(None) => app.toast(t("Not found")),
        Err(()) => {}
    }
    done(&app)
}

// ---------------------------------------------------------------- category

pub fn category_form(app: &App, name: &str) -> Option<Markup> {
    let s = app.mgr.settings();
    let (is_new, builtin, nm, exts, sites, folder) = if name == "new" {
        (true, false, String::new(), String::new(), String::new(), String::new())
    } else if name == "General" {
        (false, true, "General".to_string(), t("(every other file type)"), String::new(), s.category_dirs.get("General").map(|p| p.display().to_string()).unwrap_or_default())
    } else {
        let c = s.categories.iter().find(|c| c.name == name)?;
        (false, c.builtin, c.name.clone(), c.exts.join(" "), c.sites.join(" "), s.category_dirs.get(name).map(|p| p.display().to_string()).unwrap_or_default())
    };
    let path_enc = urlencode(name);
    Some(dialog(
        "d-category",
        &if is_new { t("Add category") } else { t("Category properties") },
        "",
        None,
        html! {
            form hx-post=(format!("/dlg/category/{path_enc}/save")) hx-swap="none" {
                div class="dlg-body" {
                    (field(&t("Category name"), html! { input class="in" type="text" name="name" value=(nm) placeholder=(t("e.g. Courses")) readonly[name == "General"] autocomplete="off"; }))
                    (field(&t("File types"), html! { input class="in" type="text" name="exts" value=(exts) placeholder=(t("mp4 mkv avi (separated by spaces)")) readonly[name == "General"] autocomplete="off" spellcheck="false"; }))
                    (field(&t("Sites"), html! { input class="in" type="text" name="sites" value=(sites) placeholder=(t("udemy.com *.coursera.org (optional)")) readonly[name == "General"] autocomplete="off" spellcheck="false"; }))
                    (field(&t("Save files to"), html! {
                        div class="pathbox" id="p-cat-folder" {
                            input class="in" type="text" name="folder" value=(folder) placeholder=(tf("automatic (Downloads\\{})", &[&nm])) autocomplete="off" spellcheck="false";
                            button type="button" class="btn" hx-post="/pick/folder" hx-include="#p-cat-folder input" hx-target="#p-cat-folder input" hx-swap="outerHTML" hx-vals=r#"{"name":"folder"}"# { (icon("folder")) }
                        }
                    }))
                    (hint(&t("Files with these extensions, or from these sites, are put into this category and saved to its folder.")))
                    div id="cat-msg" {}
                }
                (footer(html! {
                    @if !is_new && !builtin { button type="button" class="btn danger" hx-post=(format!("/dlg/category/{path_enc}/delete")) hx-swap="none" hx-confirm=(tf("Delete the category “{}”?\nIts files stay where they are and move to their file type's category.", &[&nm])) { (icon("trash")) (t("Delete")) } }
                    span class="sp" {}
                    (primary(&t("OK")))
                    (btn_close(&t("Cancel")))
                }))
            }
        },
    ))
}

pub fn urlencode(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

async fn category_dialog(State(app): State<Shared>, Path(name): Path<String>) -> Response {
    match category_form(&app, &name) {
        Some(m) => modal(m),
        None => done(&app),
    }
}

async fn category_save(State(app): State<Shared>, Path(old): Path<String>, Form(p): Params) -> Response {
    let msg = |text: String| html! { div id="cat-msg" hx-swap-oob="true" { (msg_err(&text)) } }.into_response();
    let mut s = app.mgr.settings();
    let editing = (old != "new").then_some(old);
    let name = get(&p, "name");
    if name.is_empty() {
        return msg(t("Give the category a name"));
    }
    let taken = name.eq_ignore_ascii_case("General") || s.categories.iter().any(|c| c.name.eq_ignore_ascii_case(&name) && Some(&c.name) != editing.as_ref());
    if taken && editing.as_deref() != Some("General") {
        return msg(t("A category with this name already exists"));
    }
    let exts: Vec<String> = get(&p, "exts")
        .split(|c: char| c.is_whitespace() || c == ',' || c == ';')
        .map(|e| e.trim().trim_start_matches("*.").trim_start_matches('.').to_lowercase())
        .filter(|e| !e.is_empty() && !e.starts_with('('))
        .collect();
    let sites: Vec<String> = get(&p, "sites").split_whitespace().map(|x| x.to_lowercase()).collect();
    let folder = get(&p, "folder");
    match editing.as_deref() {
        Some("General") => {}
        Some(old) => {
            if let Some(c) = s.categories.iter_mut().find(|c| c.name == old) {
                c.name = name.clone();
                c.exts = exts.clone();
                c.sites = sites;
            }
            // a type belongs to one category only
            for c in s.categories.iter_mut().filter(|c| c.name != name) {
                c.exts.retain(|e| !exts.contains(e));
            }
            if old != name {
                if let Some(p) = s.category_dirs.remove(old) {
                    s.category_dirs.insert(name.clone(), p);
                }
            }
        }
        None => {
            for c in s.categories.iter_mut() {
                c.exts.retain(|e| !exts.contains(e));
            }
            s.categories.push(CategoryDef { name: name.clone(), exts, sites, builtin: false });
        }
    }
    if folder.is_empty() {
        s.category_dirs.remove(&name);
    } else {
        s.category_dirs.insert(name.clone(), PathBuf::from(folder));
    }
    if let Err(e) = app.mgr.set_settings(s) {
        return msg(format!("{e:#}"));
    }
    app.mgr.recategorize();
    closing(&app, "d-category")
}

async fn category_delete(State(app): State<Shared>, Path(name): Path<String>) -> Response {
    let mut s = app.mgr.settings();
    let Some(c) = s.categories.iter().find(|c| c.name == name) else { return done(&app) };
    if c.builtin || name == "General" {
        app.alert(t("This category comes with Batiie and can't be deleted (you can change its file types in Properties)."));
        return done(&app);
    }
    s.categories.retain(|c| c.name != name);
    s.category_dirs.remove(&name);
    let _ = app.mgr.set_settings(s);
    app.mgr.recategorize();
    {
        let mut ui = app.ui();
        if ui.view.ends_with(&format!(":{name}")) {
            ui.view = "all".into();
        }
    }
    closing(&app, "d-category")
}

// ---------------------------------------------------------------- tips, about, what's new

const WHATS_NEW: [&str; 9] = [
    "DIOS Downloader is now Batiie — new name, new logo, new colors. Your settings and downloads were carried over.",
    "Every window is a window of its own now: it can sit on another workspace or screen, and can be resized.",
    "“Download File Info” and “Download complete” open under the mouse pointer and stay on top.",
    "The progress window shows a live speed graph and a shell-shaped progress ring.",
    "Quick buttons (pause, resume, open, folder) on the line under the pointer, and a speed graph in the status bar.",
    "Press F1 to see all keyboard shortcuts.",
    "The main window remembers its size and position.",
    "The “Add to browser” extension is now called Batiie Connect.",
    "Faster list updates, and almost no idle work with thousands of downloads.",
];

pub fn whats_new(app: &App) {
    let mut text = tf("Batiie {} is ready.", &[&env!("CARGO_PKG_VERSION")]);
    text.push_str("\n\n");
    text.push_str(&t("What's new:"));
    for line in WHATS_NEW {
        text.push_str("\n•  ");
        text.push_str(&t(line));
    }
    app.alert(text);
}

pub const TIPS: [&str; 16] = [
    "Copy a download link anywhere (Ctrl+C) and Batiie offers to download it.",
    "Press Ctrl+V in the download list to download what the clipboard holds — a link, a magnet or a .torrent file.",
    "Drag a .torrent file (or a link) from Explorer and drop it on the main window to add it.",
    "Play a video on a web page and click \"Download this video\" above it to choose the quality.",
    "Right click a web page in Chrome or Edge and choose \"Download all links with Batiie\" to pick files from the page.",
    "Every download uses up to 32 connections. If a server is slow with many, add it to Options > Connection > Exceptions.",
    "Make a queue (Queues > New queue) and set \"1 file at the same time\" to download files one after another.",
    "The Scheduler can start a queue at night and shut down the computer when everything is finished.",
    "Downloads > When all downloads finish lets you shut down, sleep or hibernate the computer after the last file.",
    "The speed limiter on the toolbar keeps your internet usable for other things while downloading.",
    "Add Batch > \"Address with a pattern\" downloads img001.jpg … img100.jpg from one address with a *.",
    "An old link expired? Right click the download > \"Refresh download address\" and continue from where it stopped.",
    "Right click a category in the sidebar > Properties to change its file types and its own folder.",
    "View > Columns shows more columns, like Date Added, Save To and Referer.",
    "Tasks > Run site grabber collects every image, video or archive of a web site.",
    "Batiie means “slow” in Arabic (بطيء). Don't judge a snail by its speed: it downloads over up to 32 connections at once.",
];

fn tip_body(app: &App, oob: bool) -> Markup {
    let s = app.mgr.settings();
    let i = s.tip_index as usize % TIPS.len();
    html! {
        div id="tip-body" class="dlg-body" hx-swap-oob=[oob.then_some("true")] {
            div class="tipbox" { (icon("bulb")) p { (t(TIPS[i])) } }
            div class="muted" { (tf("Tip {} of {}", &[&(i + 1), &TIPS.len()])) }
            (check("show_on_start", &t("Show tips on startup"), s.show_tips))
        }
    }
}

/// Opens the tip window; at startup the next tip is queued for the next time.
pub fn show_tip(app: &App, from_menu: bool) {
    if !from_menu {
        let mut s = app.mgr.settings();
        let cur = s.tip_index;
        s.tip_index = (cur + 1) % TIPS.len() as u32;
        // this start shows the tip that was waiting
        let _ = app.mgr.set_settings(s);
        app.open(format!("/dlg/tips?at={cur}"));
    } else {
        app.open("/dlg/tips");
    }
}

async fn tips_dialog(State(app): State<Shared>, Query(q): Query<HashMap<String, String>>) -> Response {
    // opened at startup: the tip that was due (the stored index already moved on)
    if let Some(at) = q.get("at").and_then(|a| a.parse::<u32>().ok()) {
        let mut s = app.mgr.settings();
        let next = s.tip_index;
        s.tip_index = at;
        let _ = app.mgr.set_settings(s.clone());
        let body = tip_body(&app, false);
        s.tip_index = next;
        let _ = app.mgr.set_settings(s);
        return tips_modal(body);
    }
    tips_modal(tip_body(&app, false))
}

fn tips_modal(body: Markup) -> Response {
    modal(dialog(
        "d-tips",
        &t("Tip of the day"),
        "small",
        Some("/dlg/tips/close"),
        html! {
            form id="tips-form" hx-post="/dlg/tips/close" hx-swap="none" {
                (body)
                (footer(html! {
                    button type="button" class="btn" hx-post="/dlg/tips/next" hx-include="#tips-form" hx-swap="none" { (t("Next tip")) }
                    span class="sp" {}
                    (primary(&t("Close")))
                }))
            }
        },
    ))
}

async fn tips_next(State(app): State<Shared>, Form(p): Params) -> Response {
    let mut s = app.mgr.settings();
    s.tip_index = (s.tip_index + 1) % TIPS.len() as u32;
    s.show_tips = flag(&p, "show_on_start");
    let _ = app.mgr.set_settings(s);
    tip_body(&app, true).into_response()
}

async fn tips_close(State(app): State<Shared>, Form(p): Params) -> Response {
    if p.contains_key("show_on_start") || p.contains_key("_x") || !p.is_empty() {
        let mut s = app.mgr.settings();
        let want = flag(&p, "show_on_start");
        if s.show_tips != want {
            s.show_tips = want;
            let _ = app.mgr.set_settings(s);
        }
    }
    closing(&app, "d-tips")
}

async fn about_dialog() -> Response {
    let link = |url: &str, label: &str| html! { a href="#" data-ext=(url) { (label) } };
    let ar = crate::i18n::current() == "ar";
    modal(dialog(
        "d-about",
        &t("About Batiie"),
        "small",
        None,
        html! {
            div class="dlg-body about" {
                img class="about-logo" src="/static/logo.png" alt="Batiie";
                div class="wm-big" { "batiie" }
                div class="muted" { (tf("Version {}", &[&env!("CARGO_PKG_VERSION")])) }
                div class="tagline" { (t("Slow by name. Fast by nature.")) }
                @if !ar { div class="tag-ar" lang="ar" dir="rtl" { "بطيء بالاسم. سريع فالتحميل." } }
                div class="muted" style="margin-top:6px" { (t("Coded by Batiie Team")) }
                div class="row-flex" style="justify-content:center" {
                    (link("https://github.com/OussamaTabDev/DIOS-downloader", "GitHub"))
                    span class="muted" { "·" }
                    (link("https://www.facebook.com/profile.php?id=61558130707674", "Facebook"))
                }
                p class="muted" style="margin:14px 0 4px" { (t("Like Batiie? You can support its development:")) }
                a class="btn primary" href="#" data-ext="https://www.paypal.com/paypalme/ahmedezzat99" { (t("Support with PayPal")) }
            }
            (footer(html! { span class="sp" {} (btn_close(&t("Close"))) }))
        },
    ))
}

// ---------------------------------------------------------------- refresh download address

pub fn refresh_dialog(app: &App, id: DownloadId) -> Response {
    let Some(v) = app.mgr.get(id) else { return done(app) };
    if v.status == Status::Completed {
        return done(app);
    }
    app.mgr.pause(id);
    app.misc.lock().unwrap_or_else(|e| e.into_inner()).refresh_id = Some(id);
    let page = v.parent_page.clone().unwrap_or_default();
    modal(dialog(
        "d-refresh",
        &t("Refresh download address"),
        "",
        Some("/dlg/refresh/stop"),
        html! {
            form hx-post="/dlg/refresh/use" hx-swap="none" {
                div class="dlg-body" {
                    div class="big-name" { (v.file_name) }
                    p class="sel-text" style="margin:0;white-space:pre-line" { (t("1. Open the web page where you got this file.\n2. Click the download link there again (or start the video).\n3. Batiie takes the new address and continues from where it stopped.")) }
                    @if is_web_address(&page) {
                        div { a class="btn" href="#" data-ext=(page) { (icon("globe")) (t("Open the page")) } }
                    }
                    div class="row-flex" style="color:var(--accent)" { span class="spin" {} (t("Waiting for the new address from the browser…")) }
                    (field(&t("Or paste the new address here:"), html! { input class="in" type="text" name="url" placeholder=(t("https://…")) autocomplete="off" spellcheck="false"; }))
                }
                (footer(html! {
                    span class="sp" {}
                    (primary(&t("Use it")))
                    button type="button" class="btn" hx-post="/dlg/refresh/stop" hx-swap="none" { (t("Stop waiting")) }
                }))
            }
        },
    ))
}

async fn refresh_dialog_route(State(app): State<Shared>, Path(id): Path<DownloadId>) -> Response {
    refresh_dialog(&app, id)
}

/// The browser sent a download while we were waiting for a new address.
pub fn refresh_received(app: &App, id: DownloadId, url: &str, headers: RequestHeaders) {
    app.misc.lock().unwrap_or_else(|e| e.into_inner()).refresh_id = None;
    app.push(crate::state::Push::Close("d-refresh".into()));
    app.mgr.set_url(id, url, Some(headers));
    app.mgr.start(id);
    super::progress::open(app, id);
}

async fn refresh_use(State(app): State<Shared>, Form(p): Params) -> Response {
    let url = get(&p, "url");
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return done(&app);
    }
    let id = app.misc.lock().unwrap_or_else(|e| e.into_inner()).refresh_id;
    if let Some(id) = id {
        refresh_received(&app, id, &url, app.mgr.headers_of(id).unwrap_or_default());
    }
    closing(&app, "d-refresh")
}

async fn refresh_stop(State(app): State<Shared>) -> Response {
    app.misc.lock().unwrap_or_else(|e| e.into_inner()).refresh_id = None;
    closing(&app, "d-refresh")
}

// ---------------------------------------------------------------- export / import

fn io_dialog(id: &str, title: String, base: &str) -> Response {
    modal(dialog(
        id,
        &title,
        "small",
        None,
        html! {
            div class="dlg-body" {
                div class="col" {
                    button type="button" class="btn" hx-post=(format!("{base}/ef2")) hx-swap="none" { (icon("file")) (t("Download list (.ef2)")) }
                    button type="button" class="btn" hx-post=(format!("{base}/txt")) hx-swap="none" { (icon("doc")) (t("Text file with links (.txt)")) }
                }
            }
            (footer(html! { span class="sp" {} (btn_close(&t("Close"))) }))
        },
    ))
}

async fn export_dialog() -> Response {
    io_dialog("d-export", t("Export download list…"), "/dlg/export")
}

async fn import_dialog() -> Response {
    io_dialog("d-import", t("Import download list…"), "/dlg/import")
}

async fn export_do(State(app): State<Shared>, Path(kind): Path<String>) -> Response {
    let ef2k = kind == "ef2";
    let mgr = app.mgr.clone();
    let bus = app.bus.clone();
    app.rt.spawn(async move {
        let Some(h) = rfd::AsyncFileDialog::new()
            .set_file_name(if ef2k { "batiie-list.ef2" } else { "batiie-links.txt" })
            .add_filter(if ef2k { "Download list" } else { "Text file" }, &[if ef2k { "ef2" } else { "txt" }])
            .save_file()
            .await
        else {
            return;
        };
        let text = if ef2k {
            let items: Vec<Ef2Item> = mgr
                .snapshot()
                .into_iter()
                .map(|v| Ef2Item {
                    url: v.parent_page.clone().filter(|_| v.kind == DownloadKind::Dash).unwrap_or(v.url.clone()),
                    referer: v.headers.referer.clone(),
                    cookies: mgr.headers_of(v.id).and_then(|h| h.cookies),
                    user_agent: v.headers.user_agent.clone(),
                    file_name: Some(v.file_name.clone()),
                    description: v.description.clone(),
                })
                .collect();
            ef2::write(&items)
        } else {
            mgr.all_urls().join("\r\n")
        };
        if let Err(e) = std::fs::write(h.path(), text) {
            let _ = bus.send(crate::state::Push::Alert(tf("Could not save the list: {}", &[&e])));
        }
    });
    closing(&app, "d-export")
}

async fn import_do(State(app): State<Shared>, Path(kind): Path<String>) -> Response {
    let ef2k = kind == "ef2";
    let a = app.me.lock().unwrap_or_else(|e| e.into_inner()).upgrade();
    let Some(a) = a else { return done(&app) };
    app.rt.spawn(async move {
        let Some(h) = rfd::AsyncFileDialog::new().add_filter(if ef2k { "Download list" } else { "Text file" }, &[if ef2k { "ef2" } else { "txt" }]).pick_file().await else {
            return;
        };
        let text = match std::fs::read(h.path()) {
            Ok(b) => String::from_utf8_lossy(&b).into_owned(),
            Err(e) => return a.alert(tf("Could not read the file: {}", &[&e])),
        };
        let items = ef2::parse(&text);
        if items.is_empty() {
            return a.alert(t("No download addresses were found in this file."));
        }
        let mut added = 0;
        for it in items {
            if a.mgr.find_url(&it.url).is_some() {
                continue;
            }
            a.mgr.add(NewDownload {
                url: it.url,
                file_name: it.file_name,
                headers: RequestHeaders { referer: it.referer, cookies: it.cookies, user_agent: it.user_agent, ..Default::default() },
                description: it.description,
                queue: Some(MAIN_QUEUE),
                start: false,
                ..Default::default()
            });
            added += 1;
        }
        a.alert(tf("{} download(s) were added to the main download queue.\nStart them with Start Queue, or one by one with Resume.", &[&added]));
    });
    closing(&app, "d-import")
}

#[cfg(test)]
mod tests {
    use super::is_web_address;
    #[test]
    fn only_web_pages_are_opened() {
        assert!(is_web_address("https://example.com/page"));
        assert!(is_web_address(" http://example.com "));
        for bad in ["C:\\Windows\\System32\\calc.exe", "\\\\server\\share\\x.exe", "file:///C:/x.exe", "javascript:alert(1)", "ms-settings:", "https://", ""] {
            assert!(!is_web_address(bad), "{bad}");
        }
    }
}

// ---------------------------------------------------------------- keyboard shortcuts

async fn shortcuts_dialog() -> Response {
    let groups: [(&str, &[(&str, &str)]); 3] = [
        ("Downloads", &[("Ctrl+N", "Add URL"), ("Ctrl+V", "Paste"), ("Enter", "Open the file / progress window"), ("Delete", "Remove"), ("F5", "Refresh")]),
        ("List", &[("↑ ↓ Home End PgUp PgDn", "Move the selection"), ("Shift + click", "Select a range"), ("Ctrl + click", "Add to the selection"), ("Ctrl+A", "Select all"), ("Right click", "Menu of the download")]),
        ("Program", &[("Ctrl+F", "Find…"), ("F3", "Find next"), ("Ctrl+,", "Options…"), ("F1 or ?", "Keyboard shortcuts"), ("Esc", "Close the window")]),
    ];
    modal(dialog(
        "d-shortcuts",
        &t("Keyboard shortcuts"),
        "small",
        None,
        html! {
            div class="dlg-body" {
                div class="kbd-tbl" {
                    @for (g, rows) in groups {
                        h4 class="kbd-h" { (t(g)) }
                        @for (k, what) in rows { kbd { (k) } span { (t(what)) } }
                    }
                }
            }
            (footer(html! { span class="sp" {} (btn_close(&t("Close"))) }))
        },
    ))
}
