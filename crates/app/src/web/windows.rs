//! Windows of their own. Every dialog of the program opens as a real top-level window
//! (not a panel inside the main window): the main window can be minimized, closed to the
//! tray or sit on another workspace without taking them along. Questions that must not be
//! missed (a new download, a finished download, a countdown) appear under the mouse pointer
//! and stay above other windows.
//!
//! A dialog is still written once, as a `<dialog>` fragment. When the native window layer
//! runs, the answer of its route is not sent to the main window but kept here and shown by a
//! window of its own (`/w/dlg/<label>`); without it the fragment stays an in-page dialog.
use super::page::look_attrs;
use super::ui::sprite;
use super::Shared;
use crate::i18n::{self, t};
use crate::state::{App, ShellCmd, WinPage, WinSpec};
use axum::extract::{Path, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use maud::{html, Markup, PreEscaped};
use std::sync::atomic::Ordering;

pub fn routes(r: Router<Shared>) -> Router<Shared> {
    r.route("/w/dlg/{label}", get(dialog_page))
}

/// The address of a page for a window (it brings the secret along the first time).
pub fn url(app: &App, path: &str) -> String {
    format!(
        "http://127.0.0.1:{}{}{}k={}",
        app.port.load(Ordering::Relaxed),
        path,
        if path.contains('?') { '&' } else { '?' },
        app.token
    )
}

pub fn is_windowed(app: &App) -> bool {
    app.windowed.load(Ordering::Relaxed)
}

/// How a dialog opens as a window: (width, height, min width, min height, above others,
/// under the pointer).
fn geometry(id: &str, class: &str) -> (f64, f64, f64, f64, bool, bool) {
    let by_id = match id {
        "d-add" => Some((660.0, 720.0, 520.0, 560.0, true, true)),
        "d-dup" => Some((500.0, 420.0, 420.0, 360.0, true, true)),
        "d-countdown" => Some((460.0, 380.0, 380.0, 320.0, true, true)),
        "d-refresh" => Some((580.0, 540.0, 460.0, 440.0, true, true)),
        "d-batch" => Some((720.0, 720.0, 560.0, 560.0, false, false)),
        "d-torrent" => Some((780.0, 740.0, 600.0, 580.0, false, false)),
        "d-settings" => Some((1000.0, 720.0, 760.0, 520.0, false, false)),
        "d-sched" => Some((1000.0, 700.0, 760.0, 520.0, false, false)),
        "d-grab" => Some((1040.0, 780.0, 760.0, 560.0, false, false)),
        "d-playlist" => Some((820.0, 740.0, 620.0, 560.0, false, false)),
        "d-find" => Some((480.0, 440.0, 400.0, 380.0, false, false)),
        "d-columns" => Some((440.0, 560.0, 360.0, 420.0, false, false)),
        "d-category" => Some((580.0, 560.0, 460.0, 460.0, false, false)),
        "d-tips" => Some((540.0, 400.0, 420.0, 340.0, false, false)),
        "d-about" => Some((460.0, 540.0, 380.0, 460.0, false, false)),
        "d-browsers" => Some((680.0, 580.0, 520.0, 460.0, false, false)),
        "d-update" => Some((500.0, 360.0, 400.0, 300.0, false, false)),
        "d-shortcuts" => Some((520.0, 620.0, 420.0, 420.0, false, false)),
        "d-export" | "d-import" => Some((440.0, 320.0, 360.0, 260.0, false, false)),
        _ => None,
    };
    if let Some(g) = by_id {
        return g;
    }
    if id.starts_with("d-complete-") {
        return (540.0, 430.0, 440.0, 360.0, true, true);
    }
    if id.starts_with("d-props-") {
        return (720.0, 740.0, 560.0, 560.0, false, false);
    }
    match class {
        "small" => (480.0, 440.0, 380.0, 320.0, false, false),
        "wide" => (780.0, 680.0, 580.0, 480.0, false, false),
        "xl" => (1000.0, 720.0, 760.0, 520.0, false, false),
        _ => (640.0, 660.0, 480.0, 420.0, false, false),
    }
}

fn unescape(s: &str) -> String {
    s.replace("&quot;", "\"").replace("&#39;", "'").replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&")
}

/// An attribute of the first `<dialog …>` tag of a fragment.
fn dialog_attr(html: &str, name: &str) -> Option<String> {
    let tag_end = html.find('>')?;
    let tag = &html[..tag_end];
    let key = format!(" {name}=\"");
    let start = tag.find(&key)? + key.len();
    let end = tag[start..].find('"')? + start;
    Some(unescape(&tag[start..end]))
}

/// Called with the answer of a route that produced a dialog. When windows of their own are
/// available the dialog opens as one and true is returned (the main window gets no answer).
pub fn dialog_to_window(app: &App, html: &str) -> bool {
    if !is_windowed(app) || !html.starts_with("<dialog ") {
        return false;
    }
    let (Some(id), title) = (dialog_attr(html, "id"), dialog_attr(html, "data-title").unwrap_or_default()) else { return false };
    // short questions with two buttons stay inside the window they come from
    if id.starts_with("c-") {
        return false;
    }
    let class = dialog_attr(html, "class").unwrap_or_default();
    let size_class = ["small", "wide", "xl"].iter().find(|c| class.split_whitespace().any(|x| x == **c)).copied().unwrap_or("");
    let (w, h, min_w, min_h, on_top, at_cursor) = geometry(&id, size_class);
    app.wins.lock().unwrap_or_else(|e| e.into_inner()).insert(id.clone(), WinPage { title: title.clone(), html: html.to_string() });
    app.shell(ShellCmd::Open(WinSpec { label: id.clone(), path: format!("/w/dlg/{id}"), title, w, h, min_w, min_h, on_top, at_cursor }));
    true
}

/// A plain message in a small window above everything else.
pub fn open_alert(app: &App, text: &str) {
    let n = app.win_seq.fetch_add(1, Ordering::Relaxed);
    let id = format!("d-alert-{n}");
    let body = html! {
        dialog id=(id) class="dlg small" data-title="Batiie" {
            div class="dlg-body ask" {
                span class="ask-i" { (super::ui::icon("info")) }
                div class="ask-t sel-text" { @for l in text.lines() { @if l.is_empty() { br; } @else { p { (l) } } } }
            }
            div class="dlg-foot" { span class="sp" {} button type="button" class="btn primary" data-close autofocus { (t("OK")) } }
        }
    }
    .into_string();
    app.wins.lock().unwrap_or_else(|e| e.into_inner()).insert(id.clone(), WinPage { title: "Batiie".into(), html: body });
    app.shell(ShellCmd::Open(WinSpec { label: id.clone(), path: format!("/w/dlg/{id}"), title: "Batiie".into(), w: 480.0, h: 300.0, min_w: 360.0, min_h: 240.0, on_top: true, at_cursor: true }));
}

/// A whole page for a window: the shared look, the scripts, and the content.
pub fn document(app: &App, title: &str, label: &str, class: &str, content: Markup) -> Markup {
    let s = app.mgr.settings();
    let lang = i18n::effective(&s.language);
    html! {
        (maud::DOCTYPE)
        html lang=(lang) dir=(if lang == "ar" { "rtl" } else { "ltr" }) data-theme=(s.theme) style=(super::page::root_style(&s)) {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                meta name="color-scheme" content="light dark";
                title { (title) }
                link rel="icon" href="/static/icon.png";
                link rel="stylesheet" href="/static/app.css";
                meta name="htmx-config" content=r#"{"allowEval":false,"allowScriptTags":false,"selfRequestsOnly":true,"historyCacheSize":0,"refreshOnHistoryMiss":false,"includeIndicatorStyles":false}"#;
                script src="/static/htmx.min.js" {}
                script src="/static/htmx-sse.js" {}
                script src="/static/idiomorph-ext.min.js" {}
                script src="/static/win.js" defer {}
            }
            body class={"win " (class)} hx-ext="morph,sse" hx-headers=r#"{"X-Batiie":"1"}"# data-label=(label) {
                (sprite())
                (content)
                div id="look" data-look=(look_attrs(&s)) hidden {}
                div id="sink" hx-ext="sse" sse-connect="/events?lite=1" sse-swap="ui,batiie-close" hx-swap="none" hidden {}
                (PreEscaped(format!(r#"<script type="application/json" id="batiie-boot">{}</script>"#, serde_json::json!({ "ok": t("OK"), "copied": t("Copied") }))))
            }
        }
    }
}

/// `/w/dlg/<label>`: the page of a window that was opened from a dialog.
async fn dialog_page(State(app): State<Shared>, Path(label): Path<String>) -> Response {
    let page = app.wins.lock().unwrap_or_else(|e| e.into_inner()).get(&label).cloned();
    let Some(page) = page else { return StatusCode::NOT_FOUND.into_response() };
    // the dialog is shown as it is, but open from the start and as big as the window
    let inner = page.html.replacen("<dialog ", "<dialog open ", 1).replacen(" data-modal", "", 1);
    let body = document(&app, &page.title, &label, "", PreEscaped(inner));
    let mut r = body.into_response();
    r.headers_mut().insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
    r
}

/// A window was closed by the person (its page was asked first): forget its page.
pub fn forget(app: &App, label: &str) {
    app.wins.lock().unwrap_or_else(|e| e.into_inner()).remove(label);
}
