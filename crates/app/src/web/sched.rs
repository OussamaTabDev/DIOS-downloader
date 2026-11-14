//! The Scheduler: every queue with its start / stop times, its limits and the order of
//! its files.
use super::actions::{closing, done, flag, get, modal, Params};
use super::ui::*;
use super::Shared;
use crate::i18n::{t, tf};
use crate::state::App;
use axum::extract::{Form, Path, Query, State};
use axum::response::{IntoResponse, Response};
use axum::routing::{get as route_get, post};
use axum::Router;
use batiie_engine::config::{parse_hhmm, AfterDone, QueueDef, MAIN_QUEUE};
use batiie_engine::{fmt_size, Status};
use maud::{html, Markup};
use std::collections::HashMap;
use std::path::PathBuf;

pub fn routes(r: Router<Shared>) -> Router<Shared> {
    r.route("/dlg/scheduler", route_get(open_dialog))
        .route("/dlg/scheduler/pane", route_get(pane))
        .route("/dlg/scheduler/apply", post(apply))
        .route("/dlg/scheduler/new", post(new_queue))
        .route("/dlg/scheduler/delete", post(delete_queue))
        .route("/dlg/scheduler/file/{op}", post(file_op))
        .route("/dlg/queue/{id}", route_get(open_queue))
}

type Q = Query<HashMap<String, String>>;

fn queue_arg(app: &App, p: &HashMap<String, String>) -> u64 {
    let s = app.mgr.settings();
    p.get("q").and_then(|q| q.parse().ok()).filter(|q| s.queue(*q).is_some()).unwrap_or(MAIN_QUEUE)
}

fn list(app: &App, current: u64, oob: bool) -> Markup {
    let s = app.mgr.settings();
    let views = app.mgr.snapshot();
    html! {
        div id="sc-list" class="sc-list" hx-swap-oob=[oob.then_some("true")] role="listbox" {
            @for q in &s.queues {
                @let n = views.iter().filter(|v| v.queue == Some(q.id) && v.status != Status::Completed).count();
                div class={"sc-q" @if q.id == current { " act" }} role="option" aria-selected=(q.id == current)
                    hx-get=(format!("/dlg/scheduler/pane?q={}", q.id)) hx-target="#sc-pane" hx-swap="innerHTML" {
                    span class={"si-i" @if app.mgr.queue_running(q.id) { " live" }} { (icon("queue")) }
                    span class="si-l" { (q.name) }
                    span class="cnt" { (n) }
                }
            }
        }
    }
}

fn files(app: &App, q: u64, selected: Option<u64>) -> Markup {
    let mut v: Vec<_> = app.mgr.snapshot().into_iter().filter(|v| v.queue == Some(q) && v.status != Status::Completed).collect();
    v.sort_by_key(|v| v.queue_pos);
    html! {
        div id="sc-files" {
            div class="tbl" style="--tcols:24px 1fr 90px 100px;max-height:240px" {
                div class="tr th" { span {} span { (t("File name")) } span class="td-r" { (t("Size")) } span { (t("Status")) } }
                @for f in &v {
                    label class={"tr pick" @if selected == Some(f.id) { " on" }} {
                        input type="radio" name="file" value=(f.id) checked[selected == Some(f.id)];
                        span class="ell" title=(f.file_name) { (f.file_name) }
                        span class="td-r" { (f.total_size.map(fmt_size).unwrap_or_default()) }
                        span { (t(f.status.label())) }
                    }
                }
                @if v.is_empty() { div class="tr muted" { span {} span { (t("Nothing in this queue yet.")) } } }
            }
            div class="row-flex" style="margin-top:8px" {
                @for (op, ico, label) in [("up", "up", "Move up"), ("down", "down", "Move down"), ("remove", "trash", "Delete from queue")] {
                    button type="button" class="btn sm" hx-post=(format!("/dlg/scheduler/file/{op}?q={q}")) hx-include="#sc-files input[name=file]:checked" hx-target="#sc-files" hx-swap="outerHTML" { (icon(ico)) (t(label)) }
                }
            }
            (hint(&t("Files download in this order. Add files with right click > Move to queue, Add Batch, or \"Download Later\".")))
        }
    }
}

fn hhmm_input(name: &str, v: &str) -> Markup {
    html! { input class="in num" type="text" name=(name) value=(v) placeholder="HH:MM" autocomplete="off" maxlength="5"; }
}

fn pane_markup(app: &App, qid: u64) -> Markup {
    let s = app.mgr.settings();
    let Some(q) = s.queue(qid) else { return html! {} };
    let sc = &q.schedule;
    let days = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    let when: Vec<(String, String)> = ["Do nothing", "Exit Batiie", "Sleep", "Shut down the computer", "Hibernate"]
        .iter()
        .enumerate()
        .map(|(i, l)| (i.to_string(), t(l)))
        .collect();
    let running = app.mgr.queue_running(qid);
    html! {
        form id="sc-form" hx-post="/dlg/scheduler/apply" hx-swap="none" {
            input type="hidden" name="q" value=(qid);
            div class="sc-body" {
                (field(&t("Queue name"), text("name", &q.name)))
                (htabs(&format!("sct{qid}"), &[t("Schedule"), t("Files in the queue"), t("Options")], vec![
                    html! {
                        (check("start_enabled", &t("Start the queue automatically"), sc.start_enabled))
                        div class="grp-lite" data-enable="#sc-form [name=start_enabled]" {
                            (field(&t("Start time (HH:MM)"), hhmm_input("start_time", &sc.start_time)))
                            div class="row-flex" {
                                label class="chk" { input type="radio" name="once" value="0" checked[!sc.once]; span class="box" { (icon("check")) } span class="chk-l" { (t("Daily")) } }
                                label class="chk" { input type="radio" name="once" value="1" checked[sc.once]; span class="box" { (icon("check")) } span class="chk-l" { (t("Once at (YYYY-MM-DD)")) } }
                                input class="in num" style="width:140px" type="text" name="once_date" value=(sc.once_date) placeholder="2026-12-31" autocomplete="off";
                            }
                            div class="days" {
                                @for (i, d) in days.iter().enumerate() {
                                    label class="chk" { input type="checkbox" name=(format!("d{i}")) value="1" checked[sc.days[i]]; span class="box" { (icon("check")) } span class="chk-l" { (t(d)) } }
                                }
                            }
                        }
                        (check("on_startup", &t("Start the queue when Batiie starts"), sc.on_startup))
                        div class="row-flex" {
                            (check("stop_enabled", &t("Stop the queue at"), sc.stop_enabled))
                            (hhmm_input("stop_time", &sc.stop_time))
                        }
                        (field(&t("When the queue is done"), select("after_done", &when, &q.after_done.index().to_string())))
                        (hint(&t("For Sleep you may need to allow wake timers in the Windows power options, so the computer can wake up for the next start.")))
                    },
                    html! { (files(app, qid, None)) },
                    html! {
                        div class="row-flex" {
                            span { (t("Download")) }
                            (number("parallel", q.max_parallel.max(1), 1, 32))
                            span { (t("files at the same time")) }
                        }
                        (field(&t("Number of retries for each file if downloading failed"), number("retries", q.retries, 0, 50)))
                        (field(&t("Save all files of this queue to"), html! {
                            div class="pathbox" id="p-qfolder" {
                                input class="in" type="text" name="folder" value=(q.folder.as_ref().map(|f| f.display().to_string()).unwrap_or_default()) placeholder=(t("automatic (by file type)")) autocomplete="off" spellcheck="false";
                                button type="button" class="btn" hx-post="/pick/folder" hx-include="#p-qfolder input" hx-target="#p-qfolder input" hx-swap="outerHTML" hx-vals=r#"{"name":"folder"}"# { (icon("folder")) }
                            }
                        }))
                        (check("open_when_done", &t("Open each file when it has finished downloading"), q.open_when_done))
                    },
                ]))
                div id="sc-msg" {}
            }
            (footer(html! {
                button type="button" class="btn" disabled[qid == MAIN_QUEUE] hx-post="/dlg/scheduler/delete" hx-include="#sc-form [name=q]" hx-swap="none" { (icon("trash")) (t("Delete")) }
                span class="sp" {}
                @if running {
                    button type="button" class="btn" hx-post=(format!("/act/queue/stop?q={qid}")) hx-swap="none" { (icon("stop")) (t("Stop")) }
                } @else {
                    button type="submit" name="act" value="start" class="btn primary" { (icon("play")) (t("Start now")) }
                }
                button type="submit" class="btn" { (t("Apply")) }
                (btn_close(&t("Close")))
            }))
        }
    }
}

/// Tabs along the top of a pane (no script: radio buttons and :has()).
pub fn htabs(id: &str, labels: &[String], panels: Vec<Markup>) -> Markup {
    html! {
        div class="ptabs" {
            @for i in 0..labels.len() { input type="radio" class="tabr" name=(id) id={(id) "-" (i)} checked[i == 0]; }
            nav class="fl-tabs" { @for (i, l) in labels.iter().enumerate() { label for={(id) "-" (i)} { (l) } } }
            @for p in panels { div class="ppanel" { (p) } }
        }
    }
}

async fn open_dialog(State(app): State<Shared>, Query(p): Q) -> Response {
    let current = if p.contains_key("q") {
        queue_arg(&app, &p)
    } else {
        let v = app.ui().view.clone();
        let q = v.strip_prefix("queue:").and_then(|q| q.parse().ok());
        q.filter(|q| app.mgr.peek(|s| s.queue(*q).is_some())).unwrap_or(MAIN_QUEUE)
    };
    dialog_response(&app, current)
}

fn dialog_response(app: &App, current: u64) -> Response {
    modal(dialog(
        "d-sched",
        &t("Scheduler"),
        "xl",
        None,
        html! {
            div class="sched" {
                div class="sc-side" {
                    (list(app, current, false))
                    div class="sc-btns" { button type="button" class="btn sm" hx-post="/dlg/scheduler/new" hx-swap="none" { (icon("plus")) (t("New queue")) } }
                }
                div id="sc-pane" class="sc-pane" { (pane_markup(app, current)) }
            }
        },
    ))
}

async fn open_queue(State(app): State<Shared>, Path(id): Path<String>) -> Response {
    if id == "new" {
        return new_queue_inner(&app, true);
    }
    let q = id.parse::<u64>().unwrap_or(MAIN_QUEUE);
    let q = Some(q).filter(|q| app.mgr.peek(|s| s.queue(*q).is_some())).unwrap_or(MAIN_QUEUE);
    dialog_response(&app, q)
}

async fn pane(State(app): State<Shared>, Query(p): Q) -> Response {
    let q = queue_arg(&app, &p);
    html! { (pane_markup(&app, q)) (list(&app, q, true)) }.into_response()
}

async fn new_queue(State(app): State<Shared>) -> Response {
    new_queue_inner(&app, false)
}

fn new_queue_inner(app: &App, open: bool) -> Response {
    let s = app.mgr.settings();
    let mut n = s.queues.len();
    let name = loop {
        let candidate = format!("Queue {n}");
        if !s.queues.iter().any(|q| q.name == candidate) {
            break candidate;
        }
        n += 1;
    };
    match app.mgr.add_queue(&name, 1, None) {
        Ok(id) => {
            app.ui().view = format!("queue:{id}");
            if open {
                return dialog_response(app, id);
            }
            html! { (pane_markup(app, id)) (list(app, id, true)) }.into_response().tap(|r| {
                r.headers_mut().insert("hx-retarget", "#sc-pane".parse().unwrap());
                r.headers_mut().insert("hx-reswap", "innerHTML".parse().unwrap());
            })
        }
        Err(e) => {
            app.alert(tf("Could not create the queue: {}", &[&format!("{e:#}")]));
            done(app)
        }
    }
}

trait Tap: Sized {
    fn tap(self, f: impl FnOnce(&mut Self)) -> Self;
}
impl Tap for Response {
    fn tap(mut self, f: impl FnOnce(&mut Self)) -> Self {
        f(&mut self);
        self
    }
}

async fn delete_queue(State(app): State<Shared>, Form(p): Params) -> Response {
    let id = get(&p, "q").parse::<u64>().unwrap_or(MAIN_QUEUE);
    if id == MAIN_QUEUE {
        app.alert(t("The main download queue can't be deleted."));
        return done(&app);
    }
    match app.mgr.delete_queue(id) {
        Ok(()) => {
            if app.ui().view == format!("queue:{id}") {
                app.ui().view = "all".into();
            }
            html! { (pane_markup(&app, MAIN_QUEUE)) (list(&app, MAIN_QUEUE, true)) }
                .into_response()
                .tap(|r| {
                    r.headers_mut().insert("hx-retarget", "#sc-pane".parse().unwrap());
                    r.headers_mut().insert("hx-reswap", "innerHTML".parse().unwrap());
                })
        }
        Err(e) => {
            app.alert(format!("{e:#}"));
            done(&app)
        }
    }
}

fn read(p: &HashMap<String, String>, mut q: QueueDef) -> Result<QueueDef, String> {
    let name = get(p, "name");
    if name.is_empty() {
        return Err(t("Give the queue a name"));
    }
    let (start, stop) = (get(p, "start_time"), get(p, "stop_time"));
    let (start_on, stop_on) = (flag(p, "start_enabled"), flag(p, "stop_enabled"));
    if start_on && parse_hhmm(&start).is_none() {
        return Err(t("The start time must look like 02:30"));
    }
    if stop_on && parse_hhmm(&stop).is_none() {
        return Err(t("The stop time must look like 08:00"));
    }
    let once = get(p, "once") == "1";
    let date = get(p, "once_date");
    if start_on && once && chrono::NaiveDate::parse_from_str(&date, "%Y-%m-%d").is_err() {
        return Err(t("The date must look like 2026-12-31"));
    }
    q.name = name;
    q.schedule.start_enabled = start_on;
    q.schedule.start_time = start;
    q.schedule.once = once;
    q.schedule.once_date = date;
    for i in 0..7 {
        q.schedule.days[i] = flag(p, &format!("d{i}"));
    }
    q.schedule.on_startup = flag(p, "on_startup");
    q.schedule.stop_enabled = stop_on;
    q.schedule.stop_time = stop;
    q.max_parallel = get(p, "parallel").parse::<u32>().unwrap_or(1).clamp(1, 32);
    q.retries = get(p, "retries").parse::<u32>().unwrap_or(0).clamp(0, 50);
    let folder = get(p, "folder");
    q.folder = (!folder.is_empty()).then(|| PathBuf::from(folder));
    q.after_done = AfterDone::from_index(get(p, "after_done").parse().unwrap_or(0));
    q.open_when_done = flag(p, "open_when_done");
    Ok(q)
}

async fn apply(State(app): State<Shared>, Form(p): Params) -> Response {
    let qid = get(&p, "q").parse::<u64>().unwrap_or(MAIN_QUEUE);
    let Some(q) = app.mgr.settings().queue(qid).cloned() else { return done(&app) };
    let msg = |m: Markup| html! { div id="sc-msg" hx-swap-oob="true" { (m) } }.into_response();
    match read(&p, q).map_err(anyhow::Error::msg).and_then(|q| app.mgr.update_queue(q)) {
        Ok(()) => {
            if get(&p, "act") == "start" {
                app.mgr.start_queue(qid);
            }
            app.wake.notify_waiters();
            html! { div id="sc-msg" hx-swap-oob="true" { (msg_ok(&t("Saved"))) } (list(&app, qid, true)) }.into_response()
        }
        Err(e) => msg(msg_err(&format!("{e:#}"))),
    }
}

async fn file_op(State(app): State<Shared>, Path(op): Path<String>, Query(q): Q, Form(p): Params) -> Response {
    let qid = queue_arg(&app, &q);
    let id = get(&p, "file").parse::<u64>().ok();
    let mut selected = id;
    if let Some(id) = id {
        match op.as_str() {
            "up" => app.mgr.move_in_queue(id, -1),
            "down" => app.mgr.move_in_queue(id, 1),
            "remove" => {
                app.mgr.set_queue(id, None);
                selected = None;
            }
            _ => {}
        }
    }
    app.wake.notify_waiters();
    files(&app, qid, selected).into_response()
}

#[allow(dead_code)]
fn unused(app: &App) -> Response {
    closing(app, "")
}
