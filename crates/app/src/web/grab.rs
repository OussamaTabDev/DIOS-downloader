//! Site Grabber and "Download all links" (both use the same window).
use super::actions::{closing, flag, get, modal, Params};
use super::page::kind_for;
use super::ui::*;
use super::Shared;
use crate::i18n::{t, tf};
use crate::state::App;
use axum::extract::{Form, Query, State};
use axum::response::{IntoResponse, Response};
use axum::routing::{get as route_get, post};
use axum::Router;
use batiie_engine::config::MAIN_QUEUE;
use batiie_engine::grabber::{self, FoundFile, GrabOptions, GrabProgress};
use batiie_engine::{filename, NewDownload, RequestHeaders};
use maud::{html, Markup};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tokio_util::sync::CancellationToken;

pub fn routes(r: Router<Shared>) -> Router<Shared> {
    r.route("/dlg/grabber", route_get(open_dialog))
        .route("/dlg/grabber/explore", post(explore))
        .route("/dlg/grabber/stop", post(stop))
        .route("/dlg/grabber/poll", post(poll))
        .route("/dlg/grabber/list", post(list_route))
        .route("/dlg/grabber/toggle", post(toggle))
        .route("/dlg/grabber/checkall", post(check_all))
        .route("/dlg/grabber/download", post(download))
        .route("/dlg/grabber/close", post(close))
}

#[derive(Clone)]
struct Item {
    checked: bool,
    url: String,
    name: String,
    ext: String,
    kind: &'static str,
    text: String,
    page: String,
    is_page: bool,
}

#[derive(Default)]
pub struct GrabHub {
    all: Vec<Item>,
    incoming: Arc<Mutex<Vec<FoundFile>>>,
    progress: Option<Arc<GrabProgress>>,
    cancel: Option<CancellationToken>,
    finished: Arc<AtomicBool>,
    exploring: bool,
    /// links mode: the page and its cookies
    headers: RequestHeaders,
    links_mode: bool,
    start_url: String,
    filter: String,
    type_filter: i32,
    progress_text: String,
}

fn hub(app: &App) -> std::sync::MutexGuard<'_, super::misc::MiscState> {
    app.misc.lock().unwrap_or_else(|e| e.into_inner())
}

fn item_from(url: &str, text: &str, page: &str) -> Item {
    let name = filename::from_url(url).unwrap_or_else(|| "download".into());
    let ext = filename::extension(&name);
    let is_page = grabber::is_page_ext(&ext);
    Item {
        checked: !is_page,
        url: url.to_string(),
        kind: if is_page { "text" } else { kind_for(&name) },
        ext: ext.to_uppercase(),
        name,
        text: text.trim().chars().take(200).collect(),
        page: page.to_string(),
        is_page,
    }
}

fn type_ok(it: &Item, filter: i32) -> bool {
    match filter {
        1 => it.kind == "image",
        2 => it.kind == "video",
        3 => it.kind == "audio",
        4 => matches!(it.kind, "archive" | "exe" | "apk" | "disc"),
        5 => matches!(it.kind, "pdf" | "doc" | "sheet" | "slides" | "text") && !it.is_page,
        6 => it.is_page,
        _ => !it.is_page,
    }
}

fn visible(g: &GrabHub) -> Vec<usize> {
    let f = g.filter.to_lowercase();
    g.all
        .iter()
        .enumerate()
        .filter(|(_, it)| type_ok(it, g.type_filter))
        .filter(|(_, it)| f.is_empty() || it.name.to_lowercase().contains(&f) || it.url.to_lowercase().contains(&f))
        .map(|(i, _)| i)
        .collect()
}

const SHOW_MAX: usize = 1500;

fn list_markup(g: &GrabHub, oob: bool) -> Markup {
    let shown = visible(g);
    html! {
        div id="grab-list" class="tbl" hx-swap-oob=[oob.then_some("true")] style="--tcols:26px 1.3fr 60px 2fr;height:260px" {
            div class="tr th" { span {} span { (t("File name")) } span { (t("Type")) } span { (t("Address")) } }
            @for &i in shown.iter().take(SHOW_MAX) {
                @let it = &g.all[i];
                label class="tr pick" title=(it.text) {
                    input type="checkbox" checked[it.checked] hx-post=(format!("/dlg/grabber/toggle?i={i}")) hx-trigger="change" hx-swap="none";
                    span class={"ell"} { span class={"fi k-" (it.kind)} { (icon(super::page::file_icon(it.kind))) } " " (it.name) }
                    span class="muted" { (it.ext) }
                    span class="ell muted" title=(it.url) { (it.url) }
                }
            }
            @if shown.len() > SHOW_MAX { div class="tr muted" { span {} span { (tf("… and {} more (use the filter)", &[&(shown.len() - SHOW_MAX)])) } } }
            @if shown.is_empty() { div class="tr muted" { span {} span { "—" } } }
        }
        @if oob { div id="grab-count" class="muted" hx-swap-oob="true" { (tf("{} file(s) checked", &[&g.all.iter().filter(|i| i.checked).count()])) } }
    }
}

fn status_markup(g: &GrabHub, oob: bool) -> Markup {
    html! {
        div id="grab-status" class="muted" hx-swap-oob=[oob.then_some("true")]
            hx-post=[g.exploring.then_some("/dlg/grabber/poll")] hx-trigger=[g.exploring.then_some("every 500ms")] hx-swap="none" {
            @if g.exploring { span class="spin" {} " " }
            (g.progress_text)
        }
    }
}

fn reset(g: &mut GrabHub, links_mode: bool) {
    if let Some(c) = g.cancel.take() {
        c.cancel();
    }
    *g = GrabHub { links_mode, ..Default::default() };
}

/// "Download all links with Batiie" from the browser.
pub fn open_links(app: &App, l: crate::api::LinksRequest) {
    {
        let mut m = hub(app);
        let g = &mut m.grab;
        reset(g, true);
        g.headers = RequestHeaders {
            referer: (!l.page.is_empty()).then(|| l.page.clone()),
            cookies: l.cookies.clone(),
            user_agent: l.user_agent.clone(),
            ..Default::default()
        };
        let mut seen = std::collections::HashSet::new();
        for link in &l.links {
            if seen.insert(link.url.clone()) {
                g.all.push(item_from(&link.url, &link.text, &l.page));
            }
        }
        g.start_url = l.page.clone();
    }
    app.shell(crate::state::ShellCmd::Show);
    app.open("/dlg/grabber?links=1");
}

async fn open_dialog(State(app): State<Shared>, Query(q): Query<HashMap<String, String>>) -> Response {
    let s = app.mgr.settings();
    let links_mode = q.contains_key("links");
    {
        let mut m = hub(&app);
        // opened again from the menu while it is open: it stays as it is
        if !links_mode && (m.grab.exploring || !m.grab.all.is_empty()) && !m.grab.links_mode {
            // keep the earlier results
        } else if !links_mode {
            reset(&mut m.grab, false);
        }
    }
    let m = hub(&app);
    let g = &m.grab;
    let queues: Vec<(String, String)> = s.queues.iter().map(|q| (q.id.to_string(), q.name.clone())).collect();
    let templates = opts(&["All files (not web pages)", "All images", "All videos", "All music / audio", "Archives and programs", "Documents (PDF, Office)", "Custom file types"]);
    let types = templates.clone();
    let sel_attrs = "<select hx-post=\"/dlg/grabber/list\" hx-trigger=\"change\" hx-include=\"#grab-form [name=filter],#grab-form [name=type_filter]\" hx-swap=\"none\" ";
    let type_select = maud::PreEscaped(select("type_filter", &types, &g.type_filter.to_string()).into_string().replacen("<select ", sel_attrs, 1));
    let count_html = maud::PreEscaped(format!(
        "<span id=\"grab-count\" class=\"muted\">{}</span>",
        tf("{} file(s) checked", &[&g.all.iter().filter(|i| i.checked).count()])
    ));
    let body = html! {
        form id="grab-form" hx-post="/dlg/grabber/download" hx-swap="none" {
            div class="dlg-body" {
                @if !g.links_mode {
                    div class="fld" {
                        span class="fld-l" { (t("Start page")) }
                        div class="pathbox" {
                            input class="in" type="text" name="start_url" value=(g.start_url) placeholder=(t("https://example.com/gallery/")) autocomplete="off" spellcheck="false" data-noselect;
                            button type="button" class="btn primary" data-enter hx-post="/dlg/grabber/explore" hx-include="#grab-form" hx-swap="none" { (icon("search")) (t("Start exploring")) }
                            button type="button" class="btn" hx-post="/dlg/grabber/stop" hx-swap="none" { (icon("stop")) (t("Stop")) }
                        }
                    }
                    div class="row-flex" {
                        (field(&t("Get"), select("template", &templates, "0")))
                        input class="in" style="flex:1;min-width:160px" type="text" name="exts" placeholder=(t("jpg png mp4 (custom types)")) autocomplete="off" spellcheck="false";
                    }
                    div class="row-flex" {
                        (field(&t("Levels to explore"), number("depth", 2, 0, 10)))
                        (field(&t("Pages at the same time"), number("parallel", 4, 1, 10)))
                        (check("other_sites", &t("Also explore other sites"), false))
                        (check("no_parent", &t("Stay inside the start folder"), false))
                    }
                    (status_markup(g, false))
                }
                div class="row-flex" {
                    input class="in" style="flex:1" type="search" name="filter" value=(g.filter) placeholder=(t("Filter the list…")) autocomplete="off" hx-post="/dlg/grabber/list" hx-trigger="input changed delay:200ms, search" hx-include="#grab-form [name=filter],#grab-form [name=type_filter]" hx-swap="none";
                    (type_select)
                    button type="button" class="btn sm" hx-post="/dlg/grabber/checkall?on=1" hx-swap="none" { (t("Check all")) }
                    button type="button" class="btn sm" hx-post="/dlg/grabber/checkall?on=0" hx-swap="none" { (t("Uncheck all")) }
                }
                (list_markup(g, false))
                div class="row-flex" {
                    (field(&t("Queue"), select("queue", &queues, &queues.first().map(|q| q.0.clone()).unwrap_or_default())))
                    (field(&t("Save all to"), html! {
                        div class="pathbox" id="p-gfolder" {
                            input class="in" type="text" name="folder" placeholder=(t("automatic (by file type)")) autocomplete="off" spellcheck="false";
                            button type="button" class="btn" hx-post="/pick/folder" hx-include="#p-gfolder input" hx-target="#p-gfolder input" hx-swap="outerHTML" hx-vals=r#"{"name":"folder"}"# { (icon("folder")) }
                        }
                    }))
                }
                div class="row-flex" {
                    (check("use_text", &t("Use the link text as the description"), true))
                    (check("start_now", &t("Start the queue now"), false))
                }
                div class="row-flex" { div id="grab-msg" class="muted" {} }
            }
            (footer(html! {
                (count_html)
                span class="sp" {}
                (primary(&t("Download")))
                (btn_close(&t("Close")))
            }))
        }
    };
    drop(m);
    modal(dialog("d-grab", &if links_mode { t("Download all links") } else { t("Site Grabber") }, "xl", Some("/dlg/grabber/close"), body))
}

async fn explore(State(app): State<Shared>, Form(p): Params) -> Response {
    let start = get(&p, "start_url");
    let msg = |m: String| html! { div id="grab-msg" hx-swap-oob="true" class="muted" { (msg_err(&m)) } }.into_response();
    if !(start.starts_with("http://") || start.starts_with("https://")) || url::Url::parse(&start).is_err() {
        return msg(t("Type the address of the start page (http:// or https://)"));
    }
    let template = get(&p, "template").parse::<i32>().unwrap_or(0);
    let exts: Vec<String> = if template == 6 {
        get(&p, "exts").split_whitespace().map(|e| e.trim_start_matches('.').to_lowercase()).collect()
    } else {
        grabber::template_exts(template)
    };
    let s = app.mgr.settings();
    let opts = GrabOptions {
        start_url: start.clone(),
        depth: get(&p, "depth").parse::<u32>().unwrap_or(2).clamp(0, 10),
        other_sites: flag(&p, "other_sites"),
        no_parent: flag(&p, "no_parent"),
        exts,
        include: Vec::new(),
        exclude: Vec::new(),
        max_pages: 2000,
        parallel: get(&p, "parallel").parse::<usize>().unwrap_or(4).clamp(1, 10),
        headers: RequestHeaders { auth: s.login_for(&start), ..Default::default() },
        user_agent: s.user_agent.clone(),
    };
    let progress = Arc::new(GrabProgress::default());
    let cancel = CancellationToken::new();
    let finished = Arc::new(AtomicBool::new(false));
    let incoming = Arc::new(Mutex::new(Vec::new()));
    {
        let mut m = hub(&app);
        let g = &mut m.grab;
        if let Some(c) = g.cancel.take() {
            c.cancel();
        }
        g.all.clear();
        g.progress = Some(progress.clone());
        g.cancel = Some(cancel.clone());
        g.finished = finished.clone();
        g.incoming = incoming.clone();
        g.exploring = true;
        g.start_url = start;
        g.progress_text = t("Starting…");
    }
    let client = app.mgr.client();
    app.rt.spawn(async move {
        let sink = incoming.clone();
        grabber::crawl(client, opts, progress, cancel, move |f| sink.lock().unwrap_or_else(|e| e.into_inner()).push(f)).await;
        finished.store(true, Ordering::Relaxed);
    });
    let m = hub(&app);
    html! { (status_markup(&m.grab, true)) (list_markup(&m.grab, true)) }.into_response()
}

async fn stop(State(app): State<Shared>) -> Response {
    {
        let m = hub(&app);
        if let Some(c) = m.grab.cancel.clone() {
            c.cancel();
        }
        m.grab.finished.store(true, Ordering::Relaxed);
    }
    poll(State(app)).await
}

/// Twice a second while exploring: new files and the counters.
async fn poll(State(app): State<Shared>) -> Response {
    let mut m = hub(&app);
    let g = &mut m.grab;
    if !g.exploring {
        return axum::http::StatusCode::NO_CONTENT.into_response();
    }
    let new: Vec<FoundFile> = std::mem::take(&mut *g.incoming.lock().unwrap_or_else(|e| e.into_inner()));
    let finished = g.finished.load(Ordering::Relaxed);
    let had_new = !new.is_empty();
    for f in new {
        g.all.push(item_from(&f.url, &f.text, &f.page));
    }
    if let Some(p) = &g.progress {
        let current = p.current.lock().map(|c| c.clone()).unwrap_or_default();
        g.progress_text = tf(
            "{} of {} pages explored  ·  {} files found",
            &[&p.pages_done.load(Ordering::Relaxed), &p.pages_queued.load(Ordering::Relaxed), &p.files.load(Ordering::Relaxed)],
        ) + &if finished { t("  ·  done") } else { format!("  ·  {current}") };
    }
    if finished {
        g.exploring = false;
    }
    let list = (had_new || finished).then(|| list_markup(g, true));
    html! { (status_markup(g, true)) @if let Some(l) = list { (l) } }.into_response()
}

async fn list_route(State(app): State<Shared>, Form(p): Params) -> Response {
    let mut m = hub(&app);
    m.grab.filter = get(&p, "filter");
    m.grab.type_filter = get(&p, "type_filter").parse().unwrap_or(0);
    list_markup(&m.grab, true).into_response()
}

async fn toggle(State(app): State<Shared>, Query(q): Query<HashMap<String, String>>) -> Response {
    let mut m = hub(&app);
    if let Some(i) = q.get("i").and_then(|i| i.parse::<usize>().ok()) {
        if let Some(it) = m.grab.all.get_mut(i) {
            it.checked = !it.checked;
        }
    }
    let n = m.grab.all.iter().filter(|i| i.checked).count();
    html! { div id="grab-count" hx-swap-oob="true" class="muted" { (tf("{} file(s) checked", &[&n])) } }.into_response()
}

async fn check_all(State(app): State<Shared>, Query(q): Query<HashMap<String, String>>) -> Response {
    let on = q.get("on").map(|v| v == "1").unwrap_or(false);
    let mut m = hub(&app);
    for i in visible(&m.grab) {
        m.grab.all[i].checked = on;
    }
    list_markup(&m.grab, true).into_response()
}

async fn download(State(app): State<Shared>, Form(p): Params) -> Response {
    let (items, headers) = {
        let m = hub(&app);
        (m.grab.all.iter().filter(|i| i.checked).cloned().collect::<Vec<_>>(), m.grab.headers.clone())
    };
    if items.is_empty() {
        return html! { div id="grab-msg" hx-swap-oob="true" class="muted" { (msg_err(&t("Check the files you want first"))) } }.into_response();
    }
    let s = app.mgr.settings();
    let queue = get(&p, "queue").parse::<u64>().ok().filter(|q| s.queue(*q).is_some()).unwrap_or(MAIN_QUEUE);
    let folder = get(&p, "folder");
    let use_text = flag(&p, "use_text");
    let mut added = 0;
    for it in items {
        if app.mgr.find_url(&it.url).is_some() {
            continue;
        }
        let mut h = headers.clone();
        if h.referer.is_none() && !it.page.is_empty() {
            h.referer = Some(it.page.clone());
        }
        app.mgr.add(NewDownload {
            url: it.url.clone(),
            save_dir: (!folder.is_empty()).then(|| PathBuf::from(&folder)),
            headers: h,
            queue: Some(queue),
            description: (use_text && !it.text.is_empty()).then(|| it.text.clone()),
            start: false,
            ..Default::default()
        });
        added += 1;
    }
    if flag(&p, "start_now") {
        app.mgr.start_queue(queue);
    }
    app.ui().view = format!("queue:{queue}");
    app.wake.notify_waiters();
    // the window stays open for more: what was added is reported
    {
        let mut m = hub(&app);
        for it in m.grab.all.iter_mut() {
            it.checked = false;
        }
    }
    let m = hub(&app);
    html! {
        div id="grab-msg" hx-swap-oob="true" { (msg_ok(&tf("{} file(s) added to the queue", &[&added]))) }
        (list_markup(&m.grab, true))
    }
    .into_response()
}

async fn close(State(app): State<Shared>) -> Response {
    let mut m = hub(&app);
    if let Some(c) = m.grab.cancel.take() {
        c.cancel();
    }
    m.grab.exploring = false;
    drop(m);
    let _ = closing;
    axum::http::StatusCode::NO_CONTENT.into_response()
}
