//! The windows of one download: its live progress window, the "Download complete" window
//! and the countdown before the computer shuts down.
use super::actions::{self, done, flag, get, id_of, modal, Params};
use super::page::{kind_for, state_of};
use super::ui::*;
use super::windows;
use super::Shared;
use crate::i18n::{t, tf};
use crate::state::{App, ShellCmd, WinSpec};
use axum::extract::{Form, Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get as route_get, post};
use axum::Router;
use batiie_engine::config::AfterDone;
use batiie_engine::{filename, fmt_duration, fmt_size, DownloadId, DownloadKind, DownloadView, Status};
use maud::{html, Markup};

pub fn routes(r: Router<Shared>) -> Router<Shared> {
    r.route("/w/progress/{id}", route_get(page))
        .route("/w/progress/{id}/live", route_get(live))
        .route("/w/progress/{id}/{op}", post(win_op))
        .route("/dlg/complete/{id}", route_get(complete_dialog))
        .route("/dlg/complete/{id}/{op}", post(complete_op))
        .route("/dlg/countdown", route_get(countdown_dialog))
        .route("/dlg/countdown/{op}", post(countdown_op))
}

fn label(id: DownloadId) -> String {
    format!("pg-{id}")
}

/// Open (or bring to the front) the progress window of a download: a window of its own,
/// next to the main window and not inside it.
pub fn open(app: &App, id: DownloadId) {
    let Some(v) = app.mgr.get(id) else { return };
    if v.status == Status::Completed {
        return;
    }
    let path = format!("/w/progress/{id}");
    if windows::is_windowed(app) {
        let torrent = v.kind == DownloadKind::Torrent;
        app.shell(ShellCmd::Open(WinSpec {
            label: label(id),
            path,
            title: v.file_name.clone(),
            w: if torrent { 680.0 } else { 560.0 },
            h: if torrent { 720.0 } else { 660.0 },
            min_w: 440.0,
            min_h: 460.0,
            on_top: false,
            at_cursor: false,
        }));
    } else {
        // no native windows: the page opens in the browser, once
        let first = app.ui().progress_open.iter().all(|x| *x != id);
        if first {
            app.ui().progress_open.push(id);
            let _ = open::that(windows::url(app, &path));
        }
    }
}

pub fn show_complete(app: &App, id: DownloadId) {
    if app.mgr.get(id).is_some() {
        app.open(format!("/dlg/complete/{id}"));
    }
}

/// Close the progress window of a download (it finished, or was removed).
pub fn close_window(app: &App, id: DownloadId) {
    app.shell(ShellCmd::Close(label(id)));
    app.ui().progress_open.retain(|x| *x != id);
}

fn completion_labels() -> Vec<(String, String)> {
    ["Do nothing (keep the computer on)", "Exit Batiie", "Sleep", "Shut down the computer", "Hibernate"]
        .iter()
        .enumerate()
        .map(|(i, l)| (i.to_string(), t(l)))
        .collect()
}

/// The speed of the last minute as a small area graph.
pub fn speed_chart(hist: &[f32], big: bool) -> Markup {
    const N: usize = 90;
    let (w, h) = (300.0_f32, if big { 80.0_f32 } else { 22.0 });
    let max = hist.iter().cloned().fold(1.0_f32, f32::max) * 1.15;
    let start = N.saturating_sub(hist.len());
    let pts: Vec<String> = hist
        .iter()
        .enumerate()
        .map(|(i, v)| format!("{:.1},{:.1}", (start + i) as f32 / (N - 1) as f32 * w, h - (v / max * (h - 2.0)) - 1.0))
        .collect();
    let line = pts.join(" ");
    let x0 = start as f32 / (N - 1) as f32 * w;
    let area = if pts.is_empty() { String::new() } else { format!("{x0:.1},{h} {line} {w},{h}") };
    html! {
        svg class={"chart" @if big { " big" }} viewBox=(format!("0 0 {w} {h}")) preserveAspectRatio="none" role="img" aria-label=(t("Transfer rate")) {
            @if big { line x1="0" y1=(h / 2.0) x2=(w) y2=(h / 2.0) class="gl" {} }
            polygon class="ar" points=(area) {}
            polyline class="ln" points=(line) {}
        }
    }
}

/// The shell: a ring that fills as the download does.
pub fn ring(pct: f64, st: i32) -> Markup {
    const C: f64 = 119.38; // 2 * pi * 19
    html! {
        div class={"ring s" (st)} role="img" aria-label=(format!("{:.0} %", pct * 100.0)) {
            svg viewBox="0 0 44 44" { circle class="trk" cx="22" cy="22" r="19" {} circle class="val" cx="22" cy="22" r="19" stroke-dasharray=(format!("{C:.2}")) stroke-dashoffset=(format!("{:.2}", C * (1.0 - pct.clamp(0.0, 1.0)))) {} }
            b { (format!("{:.0}", pct * 100.0)) small { "%" } }
        }
    }
}

/// The live part of a window: everything that changes while the download runs.
pub fn live_inner(app: &App, v: &DownloadView) -> Markup {
    if v.kind == DownloadKind::Torrent {
        return super::torrent::float_inner(app, v);
    }
    let pct = v.progress();
    let st = state_of(v.status);
    let status = match v.status {
        Status::Downloading => t("Receiving data…"),
        Status::Assembling => t("Joining the parts…"),
        Status::Failed => t("Error"),
        other => t(other.label()),
    };
    let id = v.id;
    let hist: Vec<f32> = app.speed_hist.lock().unwrap_or_else(|e| e.into_inner()).get(&id).map(|h| h.iter().copied().collect()).unwrap_or_default();
    let speed = if v.speed > 0.0 { format!("{}/s", fmt_size(v.speed as u64)) } else { "—".into() };
    let base = v.segments.iter().filter(|s| s.2 != u64::MAX).map(|s| s.2).max().unwrap_or(0);
    let mut segs = v.segments.clone();
    segs.sort_by_key(|s| s.0);
    let active = segs.iter().filter(|s| s.3 && s.1 < s.2).count();
    let running = v.status.is_running() || v.status == Status::Queued;
    let done_txt = match v.total_size {
        Some(total) => format!("{} / {}", fmt_size(v.downloaded), fmt_size(total)),
        None if v.parts_total > 0 => format!("{} ({}/{})", fmt_size(v.downloaded), v.parts_done, v.parts_total),
        None => fmt_size(v.downloaded),
    };
    html! {
        div class="pw-top" {
            span class={"pw-ico k-" (kind_for(&v.file_name))} { (icon(super::page::file_icon(kind_for(&v.file_name)))) }
            div class="pw-name" { div class="big-name" title=(v.file_name) { (v.file_name) } div class="muted ell sel-text" title=(v.url) { (v.url) } }
            (ring(pct as f64, st))
        }
        div class={"bar bigbar s" (st)} { i style=(format!("width:{:.2}%", pct * 100.0)) {} }
        div class="pw-sub" { span { (done_txt) } span class={"pw-st s" (st)} { (status) } }
        div class="pw-chart" {
            div class="pw-chart-h" { span class="muted" { (t("Transfer rate")) } b { (speed) } }
            (speed_chart(&hist, true))
        }
        div class="cards" {
            div class="card" { span class="muted" { (t("Time left")) } b { (v.eta_secs.map(fmt_duration).unwrap_or_else(|| "—".into())) } }
            div class="card" { span class="muted" { (t("File size")) } b { (v.total_size.map(fmt_size).unwrap_or_else(|| t("Unknown"))) } }
            div class="card" { span class="muted" { (t("Resume capability")) } b class=(if v.resumable { "ok-t" } else { "warn-t" }) { (t(if v.resumable { "Yes" } else { "No" })) } }
            @if base > 0 { div class="card" { span class="muted" { (t("Connections")) } b { (format!("{active} / {}", segs.len())) } } }
        }
        @if let Some(e) = &v.error { @if v.status == Status::Failed { (msg_err(e)) } }
        @if base > 0 && !segs.is_empty() {
            div class="muted" style="font-size:.9em" { (t("Start positions and download progress by connections")) }
            div class="segs" {
                @for (start, pos, end, act) in &segs {
                    i class={@if *act && v.status.is_running() && pos < end { "live" }}
                      style=(format!("inset-inline-start:{:.3}%;width:{:.3}%", *start as f64 / base as f64 * 100.0, (pos.saturating_sub(*start)) as f64 / base as f64 * 100.0)) {}
                }
            }
        }
        div class="pg-btns" {
            @if running {
                button type="button" class="btn" hx-post=(format!("/w/progress/{id}/pause")) hx-swap="none" { (icon("pause")) (t("Pause")) }
            } @else if v.status != Status::Completed {
                button type="button" class="btn primary" hx-post=(format!("/w/progress/{id}/resume")) hx-swap="none" { (icon("play")) (t("Resume")) }
            }
            button type="button" class="btn" hx-post=(format!("/w/progress/{id}/folder")) hx-swap="none" { (icon("folder")) (t("Open folder")) }
            span class="sp" {}
            button type="button" class="btn" hx-post=(format!("/w/progress/{id}/close")) hx-swap="none" { (t("Hide")) }
            button type="button" class="btn" hx-post=(format!("/w/progress/{id}/cancel")) hx-swap="none" { (if v.status == Status::Completed { t("Close") } else { t("Cancel") }) }
        }
    }
}

/// `/w/progress/<id>`: the whole page of the progress window.
async fn page(State(app): State<Shared>, Path(id): Path<DownloadId>) -> Response {
    let Some(v) = app.mgr.get(id) else { return StatusCode::NOT_FOUND.into_response() };
    let s = app.mgr.settings();
    let torrent = v.kind == DownloadKind::Torrent;
    let content = html! {
        div class="pw" id="pw" data-onclose=(format!("/w/progress/{id}/cancel")) data-title=(v.file_name) {
            @if torrent {
                div class="ppanel live" style="display:flex" id="pw-live" hx-get=(format!("/w/progress/{id}/live")) hx-trigger="every 700ms" hx-swap="morph:innerHTML" { (live_inner(&app, &v)) }
            } @else {
                div class="ptabs" {
                    input type="radio" class="tabr" name="pt" id="pt-1" checked;
                    input type="radio" class="tabr" name="pt" id="pt-2";
                    input type="radio" class="tabr" name="pt" id="pt-3";
                    nav class="fl-tabs" {
                        label for="pt-1" { (t("Download status")) }
                        label for="pt-2" { (t("Speed Limiter")) }
                        label for="pt-3" { (t("Options on completion")) }
                    }
                    div class="ppanel live" id="pw-live" hx-get=(format!("/w/progress/{id}/live")) hx-trigger="every 600ms" hx-swap="morph:innerHTML" { (live_inner(&app, &v)) }
                    div class="ppanel" {
                        form hx-post=(format!("/w/progress/{id}/limit")) hx-trigger="change, input changed delay:400ms" hx-swap="none" {
                            (check("limit_on", &t("Use speed limiter for this download"), v.speed_limit_kb.is_some()))
                            div class="fld" data-enable="[name=limit_on]" {
                                span class="fld-l" { (t("Maximum download speed (KB/s)")) }
                                (number("limit_kb", v.speed_limit_kb.unwrap_or(500).min(1_000_000), 1, 1_000_000))
                            }
                            (hint(&t("Changes apply right away, even while downloading.")))
                        }
                    }
                    div class="ppanel" {
                        form hx-post=(format!("/w/progress/{id}/completion")) hx-trigger="change" hx-swap="none" {
                            (field(&t("When this download completes:"), select("when", &completion_labels(), &v.on_complete.index().to_string())))
                            (check("force", &t("Force programs to close when shutting down"), s.force_shutdown))
                            (check("dialog", &t("Show the \"Download complete\" window"), s.show_complete_dialog))
                            (hint(&t("You get 30 seconds to cancel before the computer shuts down, sleeps or hibernates.")))
                        }
                    }
                }
            }
        }
    };
    windows::document(&app, &v.file_name, &label(id), "pwin", content).into_response()
}

/// The live part again (polled by the page).
async fn live(State(app): State<Shared>, Path(id): Path<DownloadId>) -> Response {
    let Some(v) = app.mgr.get(id) else { return StatusCode::GONE.into_response() };
    live_inner(&app, &v).into_response()
}

async fn win_op(State(app): State<Shared>, Path((id, op)): Path<(DownloadId, String)>, Form(p): Params) -> Response {
    match op.as_str() {
        "pause" => app.mgr.pause(id),
        "resume" => app.mgr.start(id),
        "folder" => {
            if let Some(v) = app.mgr.get(id) {
                actions::reveal_in_folder(&v);
            }
        }
        // "Hide": the window goes, the download goes on
        "close" => close_window(&app, id),
        // "Cancel" / the window's own X: stops the download and closes the window
        "cancel" => {
            if app.mgr.get(id).map(|v| v.status != Status::Completed).unwrap_or(false) {
                app.mgr.pause(id);
            }
            close_window(&app, id);
        }
        "limit" => {
            let kb = get(&p, "limit_kb").parse::<u64>().unwrap_or(500).clamp(1, 1_000_000);
            app.mgr.set_download_limit(id, flag(&p, "limit_on").then_some(kb));
        }
        "completion" => {
            let n = get(&p, "when").parse::<i32>().unwrap_or(0);
            app.mgr.set_on_complete(id, AfterDone::from_index(n));
            let mut s = app.mgr.settings();
            let (dlg, force) = (flag(&p, "dialog"), flag(&p, "force"));
            if s.show_complete_dialog != dlg || s.force_shutdown != force {
                s.show_complete_dialog = dlg;
                s.force_shutdown = force;
                let _ = app.mgr.set_settings(s);
            }
        }
        _ => return StatusCode::NOT_FOUND.into_response(),
    }
    done(&app)
}

// ---------------------------------------------------------------- complete

async fn complete_dialog(State(app): State<Shared>, Path(id): Path<DownloadId>) -> Response {
    let Some(v) = app.mgr.get(id) else { return done(&app) };
    let ext = filename::extension(&v.file_name).to_uppercase();
    let kind = kind_for(&v.file_name);
    let did = format!("d-complete-{id}");
    let path = v.save_path.display().to_string();
    let exists = v.save_path.exists();
    modal(dialog(
        &did,
        &t("Download complete"),
        "",
        None,
        html! {
            form class="done" hx-swap="none" {
                div class="dlg-body done-body" {
                    div class="done-hero" {
                        span class="done-check" { (icon("check")) }
                        div class="done-t" { (t("Download complete")) }
                    }
                    div class="filecard" {
                        span class={"ai-icon big k-" (kind)} { (icon(super::page::file_icon(kind))) @if !ext.is_empty() { b { (ext) } } }
                        div class="fc-t" {
                            div class="big-name" title=(v.file_name) { (v.file_name) }
                            div class="muted" { (v.total_size.map(fmt_size).unwrap_or_default()) @if !exists { " · " span class="err-t" { (t("The file is not there any more")) } } }
                        }
                    }
                    dl class="stats sel-text tight" {
                        dt { (t("Saved as")) } dd title=(path) { (path) }
                        dt { (t("Address")) } dd title=(v.url) { (v.url) }
                    }
                    (check("dont", &t("Don't show this again"), false))
                }
                (footer(html! {
                    button type="button" class="btn primary" autofocus disabled[!exists] hx-post=(format!("/dlg/complete/{id}/open")) hx-include="closest form" hx-swap="none" { (icon("open")) (t("Open")) }
                    button type="button" class="btn" hx-post=(format!("/dlg/complete/{id}/folder")) hx-include="closest form" hx-swap="none" { (icon("folder")) (t("Open folder")) }
                    button type="button" class="btn" disabled[!exists] hx-post=(format!("/dlg/complete/{id}/openwith")) hx-include="closest form" hx-swap="none" { (t("Open with…")) }
                    span class="sp" {}
                    button type="button" class="btn" hx-post=(format!("/dlg/complete/{id}/close")) hx-include="closest form" hx-swap="none" { (t("Close")) }
                }))
            }
        },
    ))
}

async fn complete_op(State(app): State<Shared>, Path((id, op)): Path<(DownloadId, String)>, Form(p): Params) -> Response {
    if let Some(v) = app.mgr.get(id) {
        match op.as_str() {
            "open" => {
                let _ = open::that(&v.save_path);
            }
            "openwith" => actions::open_with(&v.save_path),
            "folder" => actions::reveal_in_folder(&v),
            _ => {}
        }
    }
    if flag(&p, "dont") {
        let mut s = app.mgr.settings();
        s.show_complete_dialog = false;
        let _ = app.mgr.set_settings(s);
    }
    super::actions::closing(&app, &format!("d-complete-{id}"))
}

// ---------------------------------------------------------------- countdown

pub const COUNTDOWN: i32 = 30;

pub fn start_countdown(app: &App, action: AfterDone) {
    app.misc.lock().unwrap_or_else(|e| e.into_inner()).power = Some((action, COUNTDOWN));
    app.open("/dlg/countdown");
}

/// Once a second.
pub fn tick_countdown(app: &App) {
    let left = {
        let mut m = app.misc.lock().unwrap_or_else(|e| e.into_inner());
        let Some((_, left)) = m.power.as_mut() else { return };
        *left -= 1;
        *left
    };
    if left <= 0 {
        power_now(app);
    }
}

fn power_now(app: &App) {
    let Some((action, _)) = app.misc.lock().unwrap_or_else(|e| e.into_inner()).power.take() else { return };
    app.push(crate::state::Push::Close("d-countdown".into()));
    // save the list first: the downloads resume after the restart
    app.mgr.shutdown();
    let force = app.mgr.peek(|s| s.force_shutdown);
    batiie_engine::system::power(action, force);
}

fn countdown_body(app: &App, oob: bool) -> Markup {
    let (action, left) = app.misc.lock().unwrap_or_else(|e| e.into_inner()).power.unwrap_or((AfterDone::Nothing, 0));
    let text = match action {
        AfterDone::Shutdown => "shut down the computer",
        AfterDone::Hibernate => "hibernate the computer",
        _ => "put the computer to sleep",
    };
    html! {
        div id="cd-body" class="dlg-body" hx-swap-oob=[oob.then_some("true")] hx-get=[(!oob).then_some("/dlg/countdown?live=1")] hx-trigger=[(!oob).then_some("every 1s")] hx-swap=[(!oob).then_some("outerHTML")] {
            p style="margin:0;text-align:center" { (tf("Batiie will {} in", &[&t(text)])) }
            div class="cd-big" { (left.max(0)) }
            div class="bar bigbar s1" { i style=(format!("width:{:.1}%", (left.max(0) as f32 / COUNTDOWN as f32) * 100.0)) {} }
        }
    }
}

async fn countdown_dialog(State(app): State<Shared>, axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>) -> Response {
    let active = app.misc.lock().unwrap_or_else(|e| e.into_inner()).power.is_some();
    if q.contains_key("live") {
        if !active {
            return actions::closing(&app, "d-countdown");
        }
        return countdown_body(&app, false).into_response();
    }
    if !active {
        return done(&app);
    }
    modal(dialog(
        "d-countdown",
        &t("Shutting down"),
        "small",
        Some("/dlg/countdown/stop"),
        html! {
            (countdown_body(&app, false))
            (footer(html! {
                button type="button" class="btn danger" hx-post="/dlg/countdown/now" hx-swap="none" { (t("Now")) }
                button type="button" class="btn primary" autofocus data-close { (t("Stop")) }
            }))
        },
    ))
}

async fn countdown_op(State(app): State<Shared>, Path(op): Path<String>) -> Response {
    match op.as_str() {
        "now" => power_now(&app),
        _ => {
            app.misc.lock().unwrap_or_else(|e| e.into_inner()).power = None;
        }
    }
    let _ = id_of;
    actions::closing(&app, "d-countdown")
}
