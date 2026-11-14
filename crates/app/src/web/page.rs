//! The main window: menu bar, toolbar, categories, the download list and the
//! status bar – rendered as HTML, and diffed so a live connection only gets
//! what changed.
use super::ui::*;
use crate::i18n::{t, tf};
use crate::state::{App, UiState};
use batiie_engine::config::{Settings, MAIN_QUEUE};
use batiie_engine::{fmt_duration, fmt_size, DownloadKind, DownloadView, Status};
use maud::{html, Markup, PreEscaped};
use std::collections::HashMap;
use std::fmt::Write;

pub fn state_of(s: Status) -> i32 {
    match s {
        Status::Queued => 0,
        Status::Connecting | Status::Downloading | Status::Assembling => 1,
        Status::Paused => 2,
        Status::Completed => 3,
        Status::Failed => 4,
    }
}

pub fn fmt_time(t: chrono::DateTime<chrono::Utc>) -> String {
    t.with_timezone(&chrono::Local).format("%b %d %H:%M").to_string()
}

pub fn kind_for(name: &str) -> &'static str {
    match batiie_engine::filename::extension(name).as_str() {
        "mp4" | "mkv" | "avi" | "mov" | "wmv" | "flv" | "webm" | "m4v" | "ts" | "3gp" | "mpg" | "mpeg" | "vob"
        | "rm" | "rmvb" | "ogv" | "asf" => "video",
        "mp3" | "wav" | "flac" | "aac" | "ogg" | "m4a" | "wma" | "opus" | "mid" | "mka" => "audio",
        "zip" | "rar" | "7z" | "tar" | "gz" | "tgz" | "bz2" | "xz" | "zst" | "cab" | "arj" | "lzh" => "archive",
        "iso" | "img" | "dmg" => "disc",
        "pdf" => "pdf",
        "doc" | "docx" | "odt" | "rtf" | "epub" => "doc",
        "xls" | "xlsx" | "csv" | "ods" => "sheet",
        "ppt" | "pptx" | "odp" | "pps" => "slides",
        "txt" | "md" | "log" | "json" | "xml" => "text",
        "exe" | "msi" | "msix" | "appx" | "bat" | "jar" | "deb" | "rpm" => "exe",
        "apk" | "xapk" => "apk",
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp" | "svg" | "tif" | "tiff" | "heic" | "ico" | "psd" => "image",
        _ => "general",
    }
}

/// Sidebar icon of a category.
fn kind_of_category(name: &str) -> &'static str {
    match name {
        "Compressed" => "archive",
        "Documents" => "doc",
        "Music" => "audio",
        "Programs" => "app",
        "Video" => "video",
        "Images" => "image",
        _ => "file",
    }
}

pub fn file_icon(kind: &str) -> &'static str {
    match kind {
        "video" => "video",
        "audio" => "audio",
        "archive" => "archive",
        "disc" => "download",
        "pdf" | "doc" | "sheet" | "slides" | "text" => "doc",
        "exe" | "apk" => "app",
        "image" => "image",
        _ => "file",
    }
}

pub fn in_view(v: &DownloadView, view: &str) -> bool {
    match view {
        "all" => true,
        "unfinished" => v.status != Status::Completed,
        "finished" => v.status == Status::Completed,
        other => {
            if let Some(c) = other.strip_prefix("cat:") {
                v.category == c
            } else if let Some(c) = other.strip_prefix("unf:") {
                v.category == c && v.status != Status::Completed
            } else if let Some(c) = other.strip_prefix("fin:") {
                v.category == c && v.status == Status::Completed
            } else if let Some(q) = other.strip_prefix("queue:") {
                v.queue.map(|x| x.to_string()).as_deref() == Some(q)
            } else {
                true
            }
        }
    }
}

pub fn sort_views(views: &mut [DownloadView], s: &Settings) {
    let order = |v: &DownloadView| state_of(v.status);
    match s.sort_by.as_str() {
        "name" => views.sort_by_key(|v| v.file_name.to_lowercase()),
        "size" => views.sort_by_key(|v| v.total_size.unwrap_or(v.downloaded)),
        "status" => views.sort_by_key(|v| (order(v), v.id)),
        "progress" => views.sort_by(|a, b| {
            let pa = a.total_size.map(|t| a.downloaded as f64 / t.max(1) as f64).unwrap_or(0.0);
            let pb = b.total_size.map(|t| b.downloaded as f64 / t.max(1) as f64).unwrap_or(0.0);
            pa.partial_cmp(&pb).unwrap_or(std::cmp::Ordering::Equal).then(a.id.cmp(&b.id))
        }),
        "eta" => views.sort_by_key(|v| (v.eta_secs.unwrap_or(u64::MAX), v.id)),
        "speed" => views.sort_by_key(|v| (v.speed as u64, v.id)),
        "desc" => views.sort_by_key(|v| (v.description.clone().unwrap_or_default().to_lowercase(), v.id)),
        "saveto" => views.sort_by_key(|v| (v.save_path.parent().map(|p| p.to_path_buf()), v.id)),
        "referer" => views.sort_by_key(|v| (v.referer().to_lowercase(), v.id)),
        "parent" => views.sort_by_key(|v| (v.parent_page.clone().unwrap_or_default().to_lowercase(), v.id)),
        "added" => views.sort_by_key(|v| (v.added_at, v.id)),
        _ => views.sort_by_key(|v| (v.finished_at.or(v.last_try).unwrap_or(v.added_at), v.id)),
    }
    if s.sort_desc {
        views.reverse();
    }
}

/// Does a download match the filter box / Find?
fn matches_search(v: &DownloadView, ui: &UiState) -> bool {
    let q = ui.search.to_lowercase();
    if !q.is_empty()
        && !(v.file_name.to_lowercase().contains(&q)
            || v.url.to_lowercase().contains(&q)
            || v.description.as_deref().unwrap_or("").to_lowercase().contains(&q))
    {
        return false;
    }
    true
}

/// Everything one render of the main window needs.
pub struct Snap {
    pub all: Vec<DownloadView>,
    /// what the list shows, in order
    pub shown: Vec<DownloadView>,
    pub s: Settings,
    pub ui_view: String,
    pub sel: Vec<u64>,
    pub focus: Option<u64>,
    pub open: [bool; 4],
    pub search: String,
    /// total speed, once a second (the small graph in the status bar)
    pub total_hist: Vec<f32>,
}

pub fn snap(app: &App) -> Snap {
    let mut all = app.mgr.snapshot();
    // not in the list before "Start" is pressed in the Add window
    if let Some(early) = app.add.lock().unwrap_or_else(|e| e.into_inner()).early_id() {
        all.retain(|v| v.id != early);
    }
    let s = app.mgr.settings();
    sort_views(&mut all, &s);
    let mut ui = app.ui();
    ui.sel.retain(|id| all.iter().any(|v| v.id == *id));
    if ui.focus.map(|f| !all.iter().any(|v| v.id == f)).unwrap_or(false) {
        ui.focus = None;
    }
    let shown: Vec<DownloadView> =
        all.iter().filter(|v| in_view(v, &ui.view) && matches_search(v, &ui)).cloned().collect();
    Snap {
        all,
        shown,
        s,
        ui_view: ui.view.clone(),
        sel: ui.sel.clone(),
        focus: ui.focus,
        open: [ui.open_all, ui.open_unf, ui.open_fin, ui.open_queues],
        search: ui.search.clone(),
        total_hist: app.total_hist.lock().unwrap_or_else(|e| e.into_inner()).iter().copied().collect(),
    }
}

fn status_text(v: &DownloadView) -> String {
    let pct = v.progress();
    let is_torrent = v.kind == DownloadKind::Torrent;
    match v.status {
        Status::Downloading if is_torrent && v.checking => t("Checking…"),
        Status::Downloading | Status::Paused if v.total_size.is_some() => format!("{:.2}%", pct * 100.0),
        Status::Downloading if v.parts_total > 0 => tf("{}/{} parts", &[&v.parts_done, &v.parts_total]),
        Status::Downloading => fmt_size(v.downloaded),
        Status::Paused => String::new(),
        Status::Assembling => t("Merging…"),
        Status::Failed => t("Error"),
        other => t(other.label()),
    }
}

fn desc_text(v: &DownloadView) -> String {
    match (&v.error, &v.description, v.kind) {
        (Some(e), _, _) if v.status == Status::Failed => e.clone(),
        (_, Some(d), _) => d.clone(),
        (_, _, DownloadKind::Torrent) => tf(
            "{}: torrent · {} files",
            &[&t(&v.category), &v.torrent.as_ref().map(|x| x.file_list.len()).unwrap_or(0)],
        ),
        (_, _, DownloadKind::Dash) => tf("{}: video + audio", &[&t(&v.category)]),
        (_, _, DownloadKind::Hls) => tf("{}: stream", &[&t(&v.category)]),
        _ => t(&v.category),
    }
}

/// One line of the list.
pub fn row(v: &DownloadView, cols: &[(String, u32)], sel: bool, focus: bool) -> Markup {
    let pct = v.progress();
    let is_torrent = v.kind == DownloadKind::Torrent;
    let kind = if is_torrent { "torrent" } else { kind_for(&v.file_name) };
    let st = state_of(v.status);
    let size = match v.total_size {
        Some(t) => fmt_size(t),
        None if v.downloaded > 0 => fmt_size(v.downloaded),
        None => String::new(),
    };
    let speed = if v.status.is_running() && v.speed > 0.0 { format!("{}/s", fmt_size(v.speed as u64)) } else { String::new() };
    let id = v.id;
    html! {
        div class={"row st" (st) @if sel { " sel" } @if focus { " foc" }} id={"r" (id)} data-id=(id) role="row" aria-selected=(sel) {
            @for (key, _) in cols {
                @match key.as_str() {
                    "name" => {
                        div class="c c-name" title=(v.file_name) {
                            span class={"fi k-" (kind)} { (icon(if is_torrent { "torrent" } else { file_icon(kind) })) }
                            span class="nm" { (v.file_name) }
                            @if v.seeding() { span class="tag" { (t("Seeding")) } }
                            span class="quick" {
                                @match v.status {
                                    Status::Downloading | Status::Connecting | Status::Queued | Status::Assembling => { button type="button" class="rq" data-act="/act/stop" title=(t("Stop download")) { (icon("pause")) } }
                                    Status::Paused | Status::Failed => { button type="button" class="rq" data-act="/act/resume" title=(t("Download now")) { (icon("play")) } }
                                    Status::Completed => { button type="button" class="rq" data-act="/act/open" title=(t("Open")) { (icon("open")) } }
                                }
                                button type="button" class="rq" data-act="/act/folder" title=(t("Open folder")) { (icon("folder")) }
                            }
                        }
                    }
                    "q" => { div class="c c-q" { (v.queue.map(|q| q.to_string()).unwrap_or_default()) } }
                    "size" => { div class="c c-r" { (size) } }
                    "status" => {
                        div class={"c c-status s" (st)} {
                            @if matches!(v.status, Status::Failed) { span class="dot" {} }
                            (status_text(v))
                        }
                    }
                    "progress" => {
                        div class="c c-prog" {
                            div class={"bar s" (st)} role="progressbar" aria-valuenow=(format!("{:.0}", pct * 100.0)) {
                                i style=(format!("width:{:.2}%", pct * 100.0)) {}
                            }
                        }
                    }
                    "eta" => { div class="c c-r" { (v.eta_secs.map(fmt_duration).unwrap_or_default()) } }
                    "speed" => { div class="c c-r" { (speed) } }
                    "last" => { div class="c" { (fmt_time(v.finished_at.or(v.last_try).unwrap_or(v.added_at))) } }
                    "desc" => { div class="c c-dim" title=(desc_text(v)) { (desc_text(v)) } }
                    "added" => { div class="c" { (fmt_time(v.added_at)) } }
                    "saveto" => { div class="c c-dim" { (v.save_path.parent().map(|p| p.display().to_string()).unwrap_or_default()) } }
                    "referer" => { div class="c c-dim" { (v.referer()) } }
                    "parent" => { div class="c c-dim" { (v.parent_page.clone().unwrap_or_default()) } }
                    _ => { div class="c" {} }
                }
            }
        }
    }
}

fn col_title(key: &str) -> String {
    t(match key {
        "name" => "File name",
        "q" => "Q",
        "size" => "Size",
        "status" => "Status",
        "progress" => "Progress",
        "eta" => "Time left",
        "speed" => "Transfer rate",
        "last" => "Last try date",
        "desc" => "Description",
        "added" => "Date added",
        "saveto" => "Save to",
        "referer" => "Referer",
        "parent" => "Parent web page",
        _ => "",
    })
}

fn grid_cols(cols: &[(String, u32)]) -> String {
    cols.iter().map(|(_, w)| format!("{w}px")).collect::<Vec<_>>().join(" ")
}

/// Column headers (click = sort, drag the edge = width, drag the title = move).
pub fn head(s: &Settings) -> Markup {
    let cols = s.column_layout();
    let total: u32 = cols.iter().map(|c| c.1).sum();
    html! {
        div id="lhead" class="lhead" role="row" style=(format!("--cols:{};min-width:{}px", grid_cols(&cols), total)) {
            @for (key, _) in &cols {
                @let sorted = s.sort_by == *key || (key == "last" && s.sort_by == "date");
                div class={"hc" @if sorted { " sorted" }} data-key=(key) draggable="true"
                    hx-post="/sort" hx-vals=(format!(r#"{{"k":"{}"}}"#, if key == "last" { "date" } else { key })) hx-swap="none"
                    role="columnheader" aria-sort=(if sorted { if s.sort_desc { "descending" } else { "ascending" } } else { "none" }) {
                    span class="ht" { (col_title(key)) }
                    @if sorted { span class="sa" { (icon(if s.sort_desc { "down" } else { "up" })) } }
                    i class="rs" data-key=(key) {}
                }
            }
        }
    }
}

/// The toolbar's buttons (the filter box is not in here, so typing is never interrupted).
pub fn toolbar(snap: &Snap) -> Markup {
    let chosen: Vec<&DownloadView> = snap.all.iter().filter(|v| snap.sel.contains(&v.id)).collect();
    let can_resume = chosen.iter().any(|v| matches!(v.status, Status::Paused | Status::Failed));
    let can_stop = chosen.iter().any(|v| v.status == Status::Queued || v.status.is_running());
    let any_sel = !chosen.is_empty();
    let q = snap
        .ui_view
        .strip_prefix("queue:")
        .and_then(|q| q.parse::<u64>().ok())
        .filter(|q| snap.s.queue(*q).is_some())
        .unwrap_or(MAIN_QUEUE);
    let q_running = snap.all.iter().any(|v| v.queue == Some(q) && v.status.is_running());
    let any_done = snap.all.iter().any(|v| v.status == Status::Completed);
    let tb = |ico: &str, label: String, url: &str, on: bool, tip: &str, key: &str| {
        html! {
            @if url.starts_with("/dlg/") {
                button type="button" class={"tb" @if !on { " off" }} disabled[!on] title=(tip)
                    hx-get=(url) hx-target="#modal-host" hx-swap="beforeend" data-k=(key) {
                    (icon(ico)) span class="tl" { (label) }
                }
            } @else {
                button type="button" class={"tb" @if !on { " off" }} disabled[!on] title=(tip)
                    hx-post=(url) hx-swap="none" data-k=(key) {
                    (icon(ico)) span class="tl" { (label) }
                }
            }
        }
    };
    html! {
        div id="tb-btns" class={"tbs" @if snap.s.toolbar_small { " small" }} role="toolbar" {
            (tb("add", t("Add URL"), "/dlg/add", true, &t("Add a new download   Ctrl+N"), "add"))
            (tb("batch", t("Add Batch"), "/dlg/batch", true, &t("Add a list of addresses"), "batch"))
            (tb("magnet", t("Add Torrent"), "/dlg/torrent", true, &t("Add a torrent file or magnet link"), "tor"))
            (tb("copy", t("Paste"), "/act/paste", true, &t("Add the address from the clipboard   Ctrl+V"), "paste"))
            span class="tsep" {}
            (tb("play", t("Resume"), "/act/resume", can_resume, &t("Download now"), "resume"))
            (tb("stop", t("Stop"), "/act/stop", can_stop, &t("Stop download"), "stop"))
            (tb("stopall", t("Stop All"), "/act/stopall", snap.all.iter().any(|v| v.status.is_running() || v.status == Status::Queued), &t("Stop all"), "stopall"))
            (tb("trash", t("Delete"), "/act/delete", any_sel, &t("Remove   Del"), "delete"))
            (tb("check", t("Delete Done"), "/act/deletedone", any_done, &t("Delete all completed"), "deldone"))
            span class="tsep" {}
            (tb("gear", t("Options"), "/dlg/settings", true, &t("Options…"), "opts"))
            (tb("clock", t("Scheduler"), "/dlg/scheduler", true, &t("Scheduler…"), "sched"))
            button type="button" class={"tb" @if snap.s.speed_limit_on { " on" }} hx-post="/act/limiter" hx-swap="none" title=(t("Speed Limiter")) {
                (icon("speed")) span class="tl" { (if snap.s.speed_limit_on { t("Limiter: On") } else { t("Limiter: Off") }) }
            }
            (tb("queue", if q_running { t("Stop Queue") } else { t("Start Queue") }, &format!("/act/queue/{}", if q_running { "stop" } else { "start" }), true, &t("Start queue"), "queue"))
            span class="tsep" {}
            (tb("spider", t("Grabber"), "/dlg/grabber", true, &t("Site grabber"), "grab"))
            (tb("rss", t("Playlist"), "/dlg/playlist", true, &t("Playlist"), "play"))
        }
    }
}

fn count(n: usize) -> Markup {
    html! { @if n > 0 { span class="cnt" { (n) } } }
}

pub fn sidebar(snap: &Snap) -> Markup {
    let s = &snap.s;
    let all = &snap.all;
    let cur = snap.ui_view.as_str();
    // `edit`: the category this entry belongs to (its pencil opens the category window)
    let item = |view: String, ico: &str, label: String, n: usize, extra: &str, edit: Option<&str>| {
        html! {
            div class={"si" @if cur == view { " act" } " " (extra)} role="treeitem"
                hx-post="/view" hx-vals=(format!(r#"{{"v":{}}}"#, serde_json::Value::String(view.clone()))) hx-swap="none" {
                span class="si-i" { (icon(ico)) }
                span class="si-l" { (label) }
                (count(n))
                @if let Some(name) = edit {
                    button type="button" class="mini edit" title=(t("Category properties")) hx-get=(format!("/dlg/category/{}", super::info::urlencode(name))) hx-target="#modal-host" hx-swap="beforeend" hx-trigger="click consume" { (icon("edit")) }
                }
            }
        }
    };
    let head = |key: &str, view: &str, ico: &str, label: String, n: usize, open: bool| {
        html! {
            div class={"si top" @if cur == view { " act" }} role="treeitem" aria-expanded=(open) {
                span class="tw" hx-post="/tree" hx-vals=(format!(r#"{{"k":"{key}"}}"#)) hx-swap="none" { (icon(if open { "down" } else { "right" })) }
                span class="si-main" hx-post="/view" hx-vals=(format!(r#"{{"v":"{view}"}}"#)) hx-swap="none" {
                    span class="si-i" { (icon(ico)) }
                    span class="si-l" { (label) }
                    (count(n))
                }
            }
        }
    };
    let names = s.category_names();
    let cat_n = |name: &str, f: &dyn Fn(&DownloadView) -> bool| all.iter().filter(|v| v.category == name && f(v)).count();
    html! {
        nav id="sidebar" class="side" aria-label=(t("Categories")) {
            @if !s.hide_categories {
                div class="sh" {
                    span { (t("Categories")) }
                    span class="sh-acts" {
                        button type="button" class="mini" title=(t("Add category")) hx-get="/dlg/category/new" hx-target="#modal-host" hx-swap="beforeend" { (icon("plus")) }
                    }
                }
                (head("all", "all", "download", t("All Downloads"), all.len(), snap.open[0]))
                @if snap.open[0] {
                    @for name in &names {
                        (item(format!("cat:{name}"), kind_of_category(name), t(name), cat_n(name, &|_| true), "sub", Some(name)))
                    }
                }
                (head("unf", "unfinished", "clock", t("Unfinished"), all.iter().filter(|v| v.status != Status::Completed).count(), snap.open[1]))
                @if snap.open[1] {
                    @for name in &names {
                        (item(format!("unf:{name}"), kind_of_category(name), t(name), cat_n(name, &|v| v.status != Status::Completed), "sub", None))
                    }
                }
                (head("fin", "finished", "check", t("Finished"), all.iter().filter(|v| v.status == Status::Completed).count(), snap.open[2]))
                @if snap.open[2] {
                    @for name in &names {
                        (item(format!("fin:{name}"), kind_of_category(name), t(name), cat_n(name, &|v| v.status == Status::Completed), "sub", None))
                    }
                }
            }
            div class="sh" {
                span { (t("Queues")) }
                span class="sh-acts" {
                    button type="button" class="mini" title=(t("New queue")) hx-get="/dlg/queue/new" hx-target="#modal-host" { (icon("plus")) }
                }
            }
            @for q in &s.queues {
                @let running = all.iter().any(|v| v.queue == Some(q.id) && v.status.is_running());
                @let n = all.iter().filter(|v| v.queue == Some(q.id) && v.status != Status::Completed).count();
                div class={"si sub" @if cur == format!("queue:{}", q.id) { " act" }} role="treeitem"
                    hx-post="/view" hx-vals=(format!(r#"{{"v":"queue:{}"}}"#, q.id)) hx-swap="none" {
                    span class={"si-i" @if running { " live" }} { (icon("queue")) }
                    span class="si-l" { (if running { tf("{} (running)", &[&q.name]) } else { q.name.clone() }) }
                    (count(n))
                    button type="button" class="mini edit" title=(t("Edit")) hx-get=(format!("/dlg/queue/{}", q.id)) hx-target="#modal-host" { (icon("edit")) }
                }
            }
        }
    }
}

pub fn status_bar(snap: &Snap) -> Markup {
    let running = snap.all.iter().filter(|v| v.status.is_running()).count();
    let speed: f64 = snap.all.iter().map(|v| v.speed).sum();
    let text = if running > 0 {
        tf("{} downloading  ·  {}/s", &[&running, &fmt_size(speed as u64)])
    } else {
        tf("Ready  ·  {} files in the list", &[&snap.all.len()])
    };
    let lim = if snap.s.speed_limit_on {
        tf("Speed limiter: on ({} KB/s)", &[&snap.s.speed_limit_kb])
    } else {
        t("Speed limiter: off")
    };
    html! {
        footer id="status" class="status" role="status" {
            span class="st-main" { @if running > 0 { span class="pulse" {} } (text) }
            @if running > 0 { span class="spark" { (super::progress::speed_chart(&snap.total_hist, false)) } }
            span class="st-sp" {}
            span class="st-sel" { @if snap.sel.len() > 1 { (tf("{} selected", &[&snap.sel.len()])) } }
            button type="button" class={"st-lim" @if snap.s.speed_limit_on { " on" }} hx-post="/act/limiter" hx-swap="none" title=(t("Speed Limiter")) {
                (icon("speed")) (lim)
            }
        }
    }
}

pub fn list_body(snap: &Snap) -> Markup {
    let cols = snap.s.column_layout();
    html! {
        @if snap.shown.is_empty() {
            div class="empty" {
                img class="empty-logo" src="/static/logo.png" alt="";
                @if !snap.search.is_empty() {
                    p { (tf("Nothing matches “{}”.", &[&snap.search])) }
                } @else {
                    @for line in t("Nothing to race today.\nPaste a link, click “Add URL”, or send a download from your browser.").lines() { p { (line) } }
                    div class="empty-acts" {
                        button type="button" class="btn primary" hx-get="/dlg/add" hx-target="#modal-host" hx-swap="beforeend" { (icon("add")) (t("Add URL")) }
                        button type="button" class="btn" hx-post="/act/paste" hx-swap="none" { (icon("copy")) (t("Paste")) }
                    }
                }
            }
        } @else {
            @for v in &snap.shown {
                (row(v, &cols, snap.sel.contains(&v.id), snap.focus == Some(v.id)))
            }
        }
    }
}

// ------------------------------------------------------------------ live updates

/// What one live connection has already been sent.
#[derive(Default)]
pub struct Cache {
    head: String,
    toolbar: String,
    sidebar: String,
    status: String,
    order: Vec<u64>,
    rows: HashMap<u64, String>,
    /// what each row looked like (a cheap fingerprint), so unchanged rows are never rendered again
    fps: HashMap<u64, u64>,
    /// how many looks in a row found nothing new (the live connection slows down)
    pub quiet: u32,
    list_empty: Option<String>,
    look: String,
    title: String,
}

/// Everything a row shows, boiled down to one number.
fn row_fp(v: &DownloadView, sel: bool, foc: bool, cols: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (sel, foc, cols).hash(&mut h);
    (&v.file_name, &v.category, &v.description, &v.error, &v.parent_page, v.queue, v.total_size, v.downloaded).hash(&mut h);
    (std::mem::discriminant(&v.status), std::mem::discriminant(&v.kind), v.checking, v.parts_done, v.parts_total).hash(&mut h);
    (v.speed.to_bits(), v.eta_secs, v.added_at, v.finished_at, v.last_try, v.resumable).hash(&mut h);
    (v.save_path.as_os_str(), v.referer(), v.seeding()).hash(&mut h);
    v.torrent.as_ref().map(|t| t.file_list.len()).hash(&mut h);
    h.finish()
}

/// The element that holds the rows.
pub fn list_el(s: &Settings, inner: &str, oob: bool) -> String {
    let cols = s.column_layout();
    let total: u32 = cols.iter().map(|c| c.1).sum();
    format!(
        r#"<div id="list" class="list" role="rowgroup" tabindex="0"{} style="--cols:{};min-width:{}px">{}</div>"#,
        if oob { r#" hx-swap-oob="morph""# } else { "" },
        grid_cols(&cols),
        total,
        inner
    )
}

/// The changes since the last call, as out-of-band swaps for htmx.
pub fn diff(app: &App, c: &mut Cache) -> String {
    let snap = snap(app);
    let mut out = String::new();
    let cols = snap.s.column_layout();

    let hd = head(&snap.s).into_string();
    if hd != c.head {
        out.push_str(&hd.replacen("<div ", r#"<div hx-swap-oob="morph" "#, 1));
        c.head = hd;
    }
    let tb = toolbar(&snap).into_string();
    if tb != c.toolbar {
        out.push_str(&tb.replacen("<div ", r#"<div hx-swap-oob="morph" "#, 1));
        c.toolbar = tb;
    }
    let sb = sidebar(&snap).into_string();
    if sb != c.sidebar {
        out.push_str(&sb.replacen("<nav ", r#"<nav hx-swap-oob="morph" "#, 1));
        c.sidebar = sb;
    }
    let st = status_bar(&snap).into_string();
    if st != c.status {
        out.push_str(&st.replacen("<footer ", r#"<footer hx-swap-oob="morph" "#, 1));
        c.status = st;
    }

    // the look (theme, text size, language direction) – changes now and then
    let look = look_attrs(&snap.s);
    if look != c.look {
        out.push_str(&format!(r#"<div id="look" hx-swap-oob="true" data-look="{}"></div>"#, look));
        c.look = look;
    }

    // the list: rows in a new order / number → the whole list, else only changed rows
    let order: Vec<u64> = snap.shown.iter().map(|v| v.id).collect();
    let cols_sig = hd_cols(&snap.s);
    let mut fps: HashMap<u64, u64> = HashMap::with_capacity(snap.shown.len());
    let mut rendered: HashMap<u64, String> = HashMap::with_capacity(snap.shown.len());
    let mut changed: Vec<u64> = Vec::new();
    for v in &snap.shown {
        let fp = row_fp(v, snap.sel.contains(&v.id), snap.focus == Some(v.id), &cols_sig);
        fps.insert(v.id, fp);
        match (c.fps.get(&v.id), c.rows.get(&v.id)) {
            (Some(old), Some(html)) if *old == fp => {
                rendered.insert(v.id, html.clone());
            }
            _ => {
                rendered.insert(v.id, row(v, &cols, snap.sel.contains(&v.id), snap.focus == Some(v.id)).into_string());
                changed.push(v.id);
            }
        }
    }
    let empty_html = if snap.shown.is_empty() { Some(list_body(&snap).into_string()) } else { None };
    if order != c.order || empty_html != c.list_empty || cols_changed(&c.head, &cols_sig) {
        let mut inner = String::new();
        match &empty_html {
            Some(e) => inner.push_str(e),
            None => {
                for id in &order {
                    inner.push_str(&rendered[id]);
                }
            }
        }
        out.push_str(&list_el(&snap.s, &inner, true));
    } else {
        for id in &changed {
            out.push_str(&rendered[id].replacen("<div ", r#"<div hx-swap-oob="morph" "#, 1));
        }
    }
    c.order = order;
    c.rows = rendered;
    c.fps = fps;
    c.list_empty = empty_html;

    // the window title shows the speed while something downloads
    let running = snap.all.iter().filter(|v| v.status.is_running()).count();
    let speed: f64 = snap.all.iter().map(|v| v.speed).sum();
    let title = if running > 0 { format!("{}/s · Batiie", fmt_size(speed as u64)) } else { "Batiie".into() };
    if title != c.title {
        out.push_str(&format!(r#"<div id="title" hx-swap-oob="true" data-title="{}"></div>"#, maud::html! { (title) }.into_string()));
        app.shell(crate::state::ShellCmd::Title(title.clone()));
        c.title = title;
    }
    c.quiet = if out.is_empty() { c.quiet.saturating_add(1) } else { 0 };
    out
}

/// The look only (theme, text size, language): what a window of its own needs.
pub fn diff_lite(app: &App, c: &mut Cache) -> String {
    let look = look_attrs(&app.mgr.settings());
    c.quiet = c.quiet.saturating_add(1);
    if look != c.look {
        c.look = look.clone();
        c.quiet = 0;
        return format!(r#"<div id="look" hx-swap-oob="true" data-look="{look}" hidden></div>"#);
    }
    String::new()
}

fn hd_cols(s: &Settings) -> String {
    grid_cols(&s.column_layout())
}

fn cols_changed(old_head: &str, new_cols: &str) -> bool {
    !old_head.contains(&format!("--cols:{new_cols};"))
}

/// The classes / variables that make the page look the way the options say.
pub fn look_attrs(s: &Settings) -> String {
    format!(
        "{}|{}|{}|{}|{}|{}",
        s.theme,
        s.font_scale,
        s.font_family,
        crate::i18n::effective(&s.language),
        s.background_image,
        s.background_strength
    )
}

pub fn root_style(s: &Settings) -> String {
    let scale = (s.font_scale.clamp(70, 200) as f32) / 100.0;
    let mut st = format!("--fs:{:.3}", scale);
    if !s.font_family.trim().is_empty() {
        let fam: String = s.font_family.chars().filter(|c| c.is_alphanumeric() || " -_".contains(*c)).collect();
        let _ = write!(st, ";--font:'{fam}',system-ui,sans-serif");
    }
    let _ = write!(st, ";--bgs:{:.2}", (s.background_strength.min(80) as f32) / 100.0);
    st
}

// ------------------------------------------------------------------ the page

/// The snail's little joke is played once: on the very first launch.
fn first_launch() -> bool {
    let dir = std::env::var_os("BATIIE_DATA_DIR").or_else(|| std::env::var_os("DIOS_DATA_DIR")).map(std::path::PathBuf::from).unwrap_or_else(batiie_engine::default_data_dir);
    let marker = dir.join(".intro-seen");
    if marker.exists() {
        return false;
    }
    let _ = std::fs::create_dir_all(&dir);
    std::fs::write(&marker, b"1").is_ok()
}

pub fn document(app: &App) -> Markup {
    let show_intro = first_launch();
    let snap = snap(app);
    let s = &snap.s;
    let lang = crate::i18n::effective(&s.language);
    let rtl = lang == "ar";
    html! {
        (maud::DOCTYPE)
        html lang=(lang) dir=(if rtl { "rtl" } else { "ltr" }) data-theme=(s.theme) style=(root_style(s)) {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                meta name="color-scheme" content="light dark";
                title { "Batiie" }
                link rel="icon" href="/static/icon.png";
                link rel="stylesheet" href="/static/app.css";
                meta name="htmx-config" content=r#"{"allowEval":false,"allowScriptTags":false,"defaultSwapStyle":"innerHTML","selfRequestsOnly":true,"historyCacheSize":0,"refreshOnHistoryMiss":false,"includeIndicatorStyles":false}"#;
                script src="/static/htmx.min.js" {}
                script src="/static/htmx-sse.js" {}
                script src="/static/idiomorph-ext.min.js" {}
                script src="/static/app.js" defer {}
            }
            body hx-ext="morph,sse" hx-headers=r#"{"X-Batiie":"1"}"# {
                (sprite())
                @if !s.background_image.is_empty() { div class="bgimg" style="background-image:url(/bg)" {} }
                div id="app" class="app" {
                    div id="menubar" class="menubar" role="menubar" {
                        button type="button" class="brand" title=(t("About Batiie")) hx-get="/dlg/about" hx-target="#modal-host" hx-swap="beforeend" {
                            img src="/static/icon.png" alt="";
                            span class="wm" { "batiie" }
                        }
                        @for (k, label) in [("tasks", "Tasks"), ("file", "File"), ("dl", "Downloads"), ("view", "View"), ("help", "Help")] {
                            button type="button" class="mt" role="menuitem" data-menu=(k) aria-haspopup="true" hx-get=(format!("/menu/{k}")) hx-target="#menu-drop" hx-swap="innerHTML" { (t(label)) }
                        }
                        span class="mb-sp" {}
                        div class="searchbox" {
                            (icon("search"))
                            input id="q" type="search" name="q" value=(snap.search) placeholder=(t("Filter the list…")) autocomplete="off" spellcheck="false"
                                hx-post="/search" hx-trigger="input changed delay:120ms, search" hx-swap="none" aria-label=(t("Filter the list…"));
                        }
                        @if app.mgr.peek(|s| s.close_to_tray) {
                            button type="button" class="tb hidebtn" hx-post="/act/hide" hx-swap="none" title=(t("Hide to tray")) { (icon("min")) span class="tl" { (t("Hide")) } }
                        }
                    }
                    div id="toolbar" class="toolbar" { (toolbar(&snap)) }
                    div class="main" {
                        (sidebar(&snap))
                        div class="split" id="split" {}
                        section class="listwrap" {
                            div class="lscroll" id="lscroll" {
                                (head(s))
                                (PreEscaped(list_el(s, &list_body(&snap).into_string(), false)))
                            }
                            div class="dropzone" id="dropzone" { (icon("download")) span { (t("Drop to add to Batiie")) } }
                        }
                    }
                    (status_bar(&snap))
                }
                @if show_intro {
                    div id="intro" class="intro" aria-hidden="true" {
                        div class="intro-snail" { span class="intro-trail" {} img src="/static/logo.png" alt=""; }
                        div class="intro-wm" { "batiie" }
                        div class="intro-tag" { (t("Slow by name. Fast by nature.")) }
                    }
                }
                div id="menu-drop" class="menu-drop" {}
                div id="ctx" class="ctx-host" {}
                div id="modal-host" {}
                div id="toasts" class="toasts" aria-live="polite" {}
                div id="look" data-look=(look_attrs(s)) hidden {}
                div id="title" hidden {}
                div id="sink" hx-ext="sse" sse-connect="/events" sse-swap="ui,batiie-open,batiie-alert,batiie-toast,batiie-close" hx-swap="none" hidden {}
                (PreEscaped(format!(r#"<script type="application/json" id="batiie-boot">{}</script>"#, serde_json::json!({
                    "close": t("Close"), "ok": t("OK"), "cancel": t("Cancel"), "notice": "Batiie"
                }))))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A real list entry (added, not started: nothing touches the network).
    fn view(name: &str) -> DownloadView {
        static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!("batiie-test-{}-{}", std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
        let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().unwrap();
        let mgr = batiie_engine::Manager::new(rt.handle().clone(), dir.clone()).unwrap();
        let id = mgr.add(batiie_engine::NewDownload { url: "https://example.com/a".into(), file_name: Some(name.into()), start: false, ..Default::default() });
        let v = mgr.get(id).unwrap();
        mgr.shutdown();
        rt.shutdown_timeout(std::time::Duration::from_millis(200));
        let _ = std::fs::remove_dir_all(dir);
        v
    }

    #[test]
    fn names_are_escaped() {
        // the engine already cleans file names; the page must not rely on that
        let mut v = view("x.zip");
        v.file_name = "<img src=x onerror=alert(1)>.zip".into();
        let html = row(&v, &[("name".into(), 200)], false, false).into_string();
        assert!(!html.contains("<img"), "{html}");
        assert!(html.contains("&lt;img"));
    }

    #[test]
    fn file_kinds() {
        assert_eq!(kind_for("a.MP4"), "video");
        assert_eq!(kind_for("x.tar.gz"), "archive");
        assert_eq!(kind_for("noext"), "general");
    }

    #[test]
    fn views_filter() {
        let mut v = view("a.zip");
        v.category = "Compressed".into();
        v.status = Status::Completed;
        assert!(in_view(&v, "all") && in_view(&v, "finished") && !in_view(&v, "unfinished"));
        assert!(in_view(&v, "fin:Compressed") && !in_view(&v, "unf:Compressed") && !in_view(&v, "cat:Video"));
        v.status = Status::Paused;
        assert!(in_view(&v, "unfinished") && in_view(&v, "unf:Compressed") && !in_view(&v, "fin:Compressed"));
    }
}
