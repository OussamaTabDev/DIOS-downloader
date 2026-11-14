//! The menu bar and the right-click menu of the list. They are asked for when
//! opened, so a check mark or a queue name is never out of date.
use super::ui::*;
use super::Shared;
use crate::i18n::{t, tf};
use crate::state::App;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use batiie_engine::config::AfterDone;
use batiie_engine::{DownloadId, DownloadKind, Status};
use maud::{html, Markup};

pub fn routes(r: Router<Shared>) -> Router<Shared> {
    r.route("/menu/{name}", get(menu))
}

/// "Select all   Ctrl+A" -> ("Select all", Some("Ctrl+A")): translations carry the key.
fn split_key(s: &str) -> (String, Option<String>) {
    match s.split_once("   ") {
        Some((a, b)) => (a.trim().to_string(), Some(b.trim().to_string())),
        None => (s.to_string(), None),
    }
}

fn post(label: &str, url: &str) -> Markup {
    let (l, k) = split_key(label);
    html! {
        button type="button" class="mi" role="menuitem" hx-post=(url) hx-swap="none" {
            span class="mck" {} span class="ml" { (l) } @if let Some(k) = k { kbd { (k) } }
        }
    }
}

fn post_k(label: &str, key: &str, url: &str) -> Markup {
    html! {
        button type="button" class="mi" role="menuitem" hx-post=(url) hx-swap="none" {
            span class="mck" {} span class="ml" { (label) } kbd { (key) }
        }
    }
}

fn dlg(label: &str, url: &str) -> Markup {
    let (l, k) = split_key(label);
    html! {
        button type="button" class="mi" role="menuitem" hx-get=(url) hx-target="#modal-host" hx-swap="beforeend" {
            span class="mck" {} span class="ml" { (l) } @if let Some(k) = k { kbd { (k) } }
        }
    }
}

fn dlg_k(label: &str, key: &str, url: &str) -> Markup {
    html! {
        button type="button" class="mi" role="menuitem" hx-get=(url) hx-target="#modal-host" hx-swap="beforeend" {
            span class="mck" {} span class="ml" { (label) } kbd { (key) }
        }
    }
}

fn checked(label: &str, on: bool, url: &str) -> Markup {
    html! {
        button type="button" class={"mi" @if on { " on" }} role="menuitemradio" aria-checked=(on) hx-post=(url) hx-swap="none" {
            span class="mck" { @if on { (icon("check")) } } span class="ml" { (label) }
        }
    }
}

fn sep() -> Markup {
    html! { div class="msep" role="separator" {} }
}

fn sub(label: &str, body: Markup) -> Markup {
    html! {
        div class="mi has-sub" role="menuitem" aria-haspopup="true" tabindex="0" {
            span class="mck" {} span class="ml" { (label) } span class="marrow" { (icon("right")) }
            div class="submenu" role="menu" { (body) }
        }
    }
}

fn disabled(label: &str) -> Markup {
    html! { div class="mi off" { span class="mck" {} span class="ml" { (label) } } }
}

async fn menu(State(app): State<Shared>, Path(name): Path<String>) -> Response {
    let s = app.mgr.settings();
    let queues = s.queues.clone();
    let body = match name.as_str() {
        "tasks" => html! {
            (dlg_k(&t("Add URL"), "Ctrl+N", "/dlg/add"))
            (dlg(&t("Add Batch"), "/dlg/batch"))
            (dlg(&t("Add Torrent"), "/dlg/torrent"))
            (post_k(&t("Paste"), "Ctrl+V", "/act/paste"))
            (sep())
            (dlg(&t("Site grabber"), "/dlg/grabber"))
            (dlg(&t("Playlist"), "/dlg/playlist"))
            (sep())
            (dlg(&t("Export download list…"), "/dlg/export"))
            (dlg(&t("Import download list…"), "/dlg/import"))
            (sep())
            (post(&t("Hide to tray"), "/act/hide"))
            (post(&t("Exit"), "/act/exit"))
        },
        "file" => html! {
            (post(&t("Download now"), "/act/resume"))
            (post(&t("Stop download"), "/act/stop"))
            (post(&t("Remove"), "/act/delete"))
            (post(&t("Redownload"), "/act/restart"))
            (post(&t("Refresh download address…"), "/act/refresh"))
            (sep())
            (post(&t("Open"), "/act/open"))
            (post(&t("Open with…"), "/act/openwith"))
            (post(&t("Open folder"), "/act/folder"))
            (post(&t("Move / Rename…"), "/act/move"))
            (post(&t("Copy address"), "/act/copy"))
            (sep())
            (post(&t("Select all   Ctrl+A"), "/act/selectall"))
            (post(&t("Properties"), "/act/properties"))
        },
        "dl" => {
            let all_done = app.mgr.when_all_done();
            html! {
                (post(&t("Resume all"), "/act/resumeall"))
                (post(&t("Stop all"), "/act/stopall"))
                (post(&t("Delete all completed"), "/act/deletedone"))
                (sep())
                (dlg(&t("Find…   Ctrl+F"), "/dlg/find"))
                (post(&t("Find next   F3"), "/act/findnext"))
                (sep())
                (sub(&t("Start queue"), html! { @for q in &queues { (post(&q.name, &format!("/act/queue/start?q={}", q.id))) } }))
                (sub(&t("Stop queue"), html! { @for q in &queues { (post(&q.name, &format!("/act/queue/stop?q={}", q.id))) } }))
                (sub(&t("Speed Limiter"), html! {
                    (checked(&t("Turn on"), s.speed_limit_on, "/act/limiter-on"))
                    (checked(&t("Turn off"), !s.speed_limit_on, "/act/limiter-off"))
                    (sep())
                    (dlg(&t("Settings…"), "/dlg/settings?tab=connection"))
                }))
                (sub(&t("When all downloads finish"), html! {
                    (checked(&t("Do nothing"), all_done == AfterDone::Nothing, "/act/alldone/0"))
                    (checked(&t("Exit Batiie"), all_done == AfterDone::Exit, "/act/alldone/1"))
                    (checked(&t("Sleep"), all_done == AfterDone::Sleep, "/act/alldone/2"))
                    (checked(&t("Shut down the computer"), all_done == AfterDone::Shutdown, "/act/alldone/3"))
                    (checked(&t("Hibernate"), all_done == AfterDone::Hibernate, "/act/alldone/4"))
                }))
                (sep())
                (dlg(&t("Scheduler…"), "/dlg/scheduler"))
                (dlg(&t("Options…"), "/dlg/settings"))
            }
        }
        "view" => {
            let arrange: [(&str, &str); 11] = [
                ("added", "By order of addition"),
                ("name", "File name"),
                ("size", "Size"),
                ("status", "Status"),
                ("eta", "Time left"),
                ("speed", "Transfer rate"),
                ("date", "Last try date"),
                ("desc", "Description"),
                ("saveto", "Save to"),
                ("referer", "Referer"),
                ("parent", "Parent web page"),
            ];
            let langs = crate::i18n::LANGS;
            let cur_lang = if s.language.is_empty() { "auto" } else { s.language.as_str() };
            let themes = ["Light", "Dark", "Ocean blue", "Midnight purple", "Forest green", "Black", "Sunset", "Teal night", "Rose"];
            html! {
                (button_focus(&t("Filter the list…")))
                (checked(&t("Show categories"), !s.hide_categories, "/act/hidecats"))
                (sub(&t("Arrange files"), html! {
                    @for (k, l) in arrange { (checked(&t(l), s.sort_by == k, &format!("/act/sortby/{k}"))) }
                    (sep())
                    (checked(&t("Ascending"), !s.sort_desc, "/act/sortdir/asc"))
                    (checked(&t("Descending"), s.sort_desc, "/act/sortdir/desc"))
                }))
                (sub(&t("Toolbar"), html! {
                    (checked(&t("Large buttons"), !s.toolbar_small, "/act/toolbar/large"))
                    (checked(&t("Small buttons"), s.toolbar_small, "/act/toolbar/small"))
                }))
                (dlg(&t("Columns…"), "/dlg/columns"))
                (sep())
                (sub(&t("Language"), html! {
                    (checked(&t("Automatic"), cur_lang == "auto", "/act/lang/auto"))
                    @for (code, label) in langs.iter() { (checked(label, cur_lang == *code, &format!("/act/lang/{code}"))) }
                }))
                (sub(&t("Theme"), html! {
                    @for (i, th) in themes.iter().enumerate() { (checked(&t(th), s.theme as usize == i, &format!("/act/theme/{i}"))) }
                    (sep())
                    (dlg(&t("Appearance…"), "/dlg/settings?tab=appearance"))
                }))
            }
        }
        "help" => html! {
            (dlg(&t("Add Batiie to my browser…"), "/dlg/browsers"))
            (dlg(&t("Tip of the day…"), "/dlg/tips"))
            (dlg_k(&t("Keyboard shortcuts"), "F1", "/dlg/shortcuts"))
            (sep())
            (dlg(&t("Check for updates"), "/dlg/update"))
            (dlg(&t("Check for updates (video downloader)"), "/dlg/update-ytdlp"))
            (sep())
            (dlg(&t("About Batiie"), "/dlg/about"))
        },
        _ => return axum::http::StatusCode::NOT_FOUND.into_response(),
    };
    html! { div class="menu" data-menu=(name) role="menu" { (body) } }.into_response()
}

/// "Filter the list…": puts the cursor in the search box.
fn button_focus(label: &str) -> Markup {
    html! { button type="button" class="mi" role="menuitem" data-focus="#q" { span class="mck" {} span class="ml" { (label) } } }
}

/// The menu of a right click on a download.
pub fn row_menu(app: &App, id: DownloadId, x: i32, y: i32) -> Markup {
    let Some(v) = app.mgr.get(id) else { return html! {} };
    let sel = app.ui().sel.clone();
    let many = sel.len() > 1 && sel.contains(&id);
    let s = app.mgr.settings();
    let vals = format!(r#"{{"id":{id}}}"#);
    let item = |label: String, url: &str, on: bool| {
        html! {
            @if on {
                button type="button" class="mi" role="menuitem" hx-post=(url) hx-vals=(vals) hx-swap="none" { span class="mck" {} span class="ml" { (label) } }
            } @else { (disabled(&label)) }
        }
    };
    let done = v.status == Status::Completed;
    let can_resume = matches!(v.status, Status::Paused | Status::Failed | Status::Queued) || many;
    let can_stop = v.status.is_running() || v.status == Status::Queued || many;
    let torrent = v.kind == DownloadKind::Torrent;
    html! {
        div class="menu ctx" role="menu" style=(format!("left:{x}px;top:{y}px")) data-x=(x) data-y=(y) {
            @if many { div class="mhead" { (tf("{} selected", &[&sel.len()])) } }
            @if !many {
                (item(t("Open"), "/act/open", done))
                (item(t("Open with…"), "/act/openwith", done))
                (item(t("Open folder"), "/act/folder", true))
                (sep())
            }
            (item(t("Download now"), "/act/resume", can_resume))
            (item(t("Stop download"), "/act/stop", can_stop))
            (item(t("Redownload"), "/act/restart", true))
            @if !many && !torrent { (item(t("Refresh download address…"), "/act/refresh", true)) }
            (sep())
            (sub(&t("Move to queue"), html! {
                @for q in &s.queues {
                    button type="button" class={"mi" @if v.queue == Some(q.id) { " on" }} role="menuitemradio"
                        hx-post="/act/to-queue" hx-vals=(format!(r#"{{"id":{id},"q":{}}}"#, q.id)) hx-swap="none" {
                        span class="mck" { @if v.queue == Some(q.id) { (icon("check")) } } span class="ml" { (q.name) }
                    }
                }
                (sep())
                button type="button" class="mi" role="menuitem" hx-post="/act/to-queue" hx-vals=(format!(r#"{{"id":{id},"q":0}}"#)) hx-swap="none" {
                    span class="mck" {} span class="ml" { (t("Remove from queue")) }
                }
            }))
            (sep())
            (item(t("Copy address"), "/act/copy", true))
            @if !many {
                (item(t("Copy file name"), "/act/copyname", true))
                (item(t("Copy full path"), "/act/copypath", true))
                (item(t("SHA-256 checksum"), "/act/sha256", done))
                (item(t("Move / Rename…"), "/act/move", true))
                @if torrent { (item(t("Torrent files…"), "/act/torrent", true)) }
                (item(t("Properties"), "/act/properties", true))
            }
            (sep())
            (item(t("Remove"), "/act/delete", true))
        }
    }
}
