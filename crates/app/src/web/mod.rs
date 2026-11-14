//! The interface server. It listens on 127.0.0.1 only, answers htmx requests with
//! HTML fragments and pushes live changes over one server-sent-events stream.
//!
//! Because it controls a program that can open files and delete things, it is
//! locked down: a secret that only the program's own window knows, a check of the
//! Host and Origin headers (against other web pages and DNS rebinding), and a
//! custom header on everything that changes something.
pub mod actions;
pub mod add;
pub mod batch;
pub mod ext;
pub mod grab;
pub mod info;
pub mod menus;
pub mod misc;
pub mod progress;
pub mod sched;
pub mod settings;
pub mod torrent;
pub mod ui;
pub mod update;
pub mod windows;
pub mod page;
pub mod playlist;

use crate::state::{App, Push};
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode};
use axum::middleware::{self, Next};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::get;
use axum::Router;
use maud::Markup;
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;
use tokio_stream::wrappers::ReceiverStream;

pub type Shared = Arc<App>;

pub use actions::tray_action;

const CSS: &str = include_str!("../../assets/web/app.css");
const JS: &str = include_str!("../../assets/web/app.js");
const WINJS: &str = include_str!("../../assets/web/win.js");
const HTMX: &str = include_str!("../../assets/web/htmx.min.js");
const SSE_EXT: &str = include_str!("../../assets/web/htmx-sse.js");
const MORPH: &str = include_str!("../../assets/web/idiomorph-ext.min.js");
const ICON: &[u8] = include_bytes!("../../assets/img/icon-window.png");
const LOGO: &[u8] = include_bytes!("../../assets/img/logo.png");
const FONT: &[u8] = include_bytes!("../../assets/web/outfit-700.woff2");

pub fn router(app: Shared) -> Router {
    let r = Router::new()
        .route("/", get(index))
        .route("/static/{name}", get(static_file))
        .route("/bg", get(background))
        .route("/events", get(events));
    let r = actions::routes(r);
    let r = add::routes(r);
    let r = batch::routes(r);
    let r = menus::routes(r);
    let r = progress::routes(r);
    let r = sched::routes(r);
    let r = settings::routes(r);
    let r = info::routes(r);
    let r = ext::routes(r);
    let r = playlist::routes(r);
    let r = grab::routes(r);
    let r = torrent::routes(r);
    let r = update::routes(r);
    let r = windows::routes(r);
    let r = misc::routes(r);
    r.layer(middleware::from_fn_with_state(app.clone(), guard)).with_state(app)
}

fn cookie_ok(headers: &HeaderMap, token: &str) -> bool {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|kv| kv.trim().strip_prefix("batiie="))
        .any(|v| constant_eq(v.as_bytes(), token.as_bytes()))
}

fn constant_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Every request goes through here first.
async fn guard(State(app): State<Shared>, req: Request, next: Next) -> Response {
    let port = app.port.load(std::sync::atomic::Ordering::Relaxed);
    if std::env::var_os("BATIIE_LOG_REQUESTS").is_some() {
        eprintln!("[ui] {} {}", req.method(), req.uri());
    }
    let h = req.headers();
    // Host: only our own address (a web page that moved its domain to 127.0.0.1 is refused)
    let host_ok = h
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .map(|v| v == format!("127.0.0.1:{port}") || v == format!("localhost:{port}"))
        .unwrap_or(false);
    if !host_ok {
        return (StatusCode::FORBIDDEN, "forbidden").into_response();
    }
    // the first visit of a window brings the secret in its address; it becomes a cookie and
    // the address is shown again without it
    if req.method() == Method::GET {
        if let Some(q) = req.uri().query() {
            let pairs: Vec<(String, String)> = url::form_urlencoded::parse(q.as_bytes()).into_owned().collect();
            if let Some((_, k)) = pairs.iter().find(|(n, _)| n == "k") {
                if constant_eq(k.as_bytes(), app.token.as_bytes()) {
                    let rest: Vec<(String, String)> = pairs.into_iter().filter(|(n, _)| n != "k").collect();
                    let mut target = req.uri().path().to_string();
                    if !rest.is_empty() {
                        target.push('?');
                        target.push_str(&url::form_urlencoded::Serializer::new(String::new()).extend_pairs(rest).finish());
                    }
                    let mut resp = Redirect::to(&target).into_response();
                    resp.headers_mut().insert(
                        header::SET_COOKIE,
                        HeaderValue::from_str(&format!("batiie={}; Path=/; HttpOnly; SameSite=Strict", app.token)).unwrap(),
                    );
                    return resp;
                }
            }
        }
    }
    if !cookie_ok(h, &app.token) {
        return (StatusCode::UNAUTHORIZED, "open Batiie from its own window").into_response();
    }
    // anything that changes something: same origin, and our own header
    if req.method() != Method::GET && req.method() != Method::HEAD {
        let origin_ok = match h.get(header::ORIGIN).and_then(|v| v.to_str().ok()) {
            None => true,
            Some(o) => o == format!("http://127.0.0.1:{port}") || o == format!("http://localhost:{port}"),
        };
        if !origin_ok || !h.contains_key("x-batiie") {
            return (StatusCode::FORBIDDEN, "forbidden").into_response();
        }
    }
    let mut resp = next.run(req).await;
    // a dialog: a window of its own when the native window layer runs
    if resp.headers().contains_key("x-batiie-dialog") {
        let (parts, body) = resp.into_parts();
        let bytes = axum::body::to_bytes(body, 8 * 1024 * 1024).await.unwrap_or_default();
        let html = String::from_utf8_lossy(&bytes).into_owned();
        if windows::dialog_to_window(&app, &html) {
            return StatusCode::NO_CONTENT.into_response();
        }
        resp = Response::from_parts(parts, Body::from(bytes));
    }
    let hm = resp.headers_mut();
    hm.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; font-src 'self'; img-src 'self' data:; \
             connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'; object-src 'none'",
        ),
    );
    hm.insert("x-content-type-options", HeaderValue::from_static("nosniff"));
    hm.insert(header::REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
    if !hm.contains_key(header::CACHE_CONTROL) {
        hm.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    }
    resp
}

async fn index(State(app): State<Shared>) -> Markup {
    page::document(&app)
}

async fn static_file(axum::extract::Path(name): axum::extract::Path<String>) -> Response {
    let (ct, body): (&str, Body) = match name.as_str() {
        "app.css" => ("text/css; charset=utf-8", Body::from(CSS)),
        "app.js" => ("text/javascript; charset=utf-8", Body::from(JS)),
        "win.js" => ("text/javascript; charset=utf-8", Body::from(WINJS)),
        "htmx.min.js" => ("text/javascript; charset=utf-8", Body::from(HTMX)),
        "htmx-sse.js" => ("text/javascript; charset=utf-8", Body::from(SSE_EXT)),
        "idiomorph-ext.min.js" => ("text/javascript; charset=utf-8", Body::from(MORPH)),
        "icon.png" => ("image/png", Body::from(ICON)),
        "logo.png" => ("image/png", Body::from(LOGO)),
        "outfit-700.woff2" => ("font/woff2", Body::from(FONT)),
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    let mut r = Response::new(body);
    r.headers_mut().insert(header::CONTENT_TYPE, HeaderValue::from_static(ct));
    // the files are part of the program; a restart is the only time they change
    r.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("max-age=3600"));
    r
}

/// The picture behind the window (Options > Appearance) – only that one file.
async fn background(State(app): State<Shared>) -> Response {
    let path = app.mgr.peek(|s| s.background_image.clone());
    if path.is_empty() {
        return StatusCode::NOT_FOUND.into_response();
    }
    let p = std::path::PathBuf::from(&path);
    let ct = match batiie_engine::filename::extension(&path).as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    match tokio::fs::read(&p).await {
        Ok(b) if b.len() < 40_000_000 => {
            let mut r = Response::new(Body::from(b));
            r.headers_mut().insert(header::CONTENT_TYPE, HeaderValue::from_static(ct));
            r
        }
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}

/// One live connection: pushes what changed, as soon as it changes.
async fn events(State(app): State<Shared>, axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    // a window of its own listens only for its look and for the order to close
    let lite = q.contains_key("lite");
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Event, Infallible>>(64);
    let mut bus = app.bus.subscribe();
    let waiting: Vec<Push> = if lite { Vec::new() } else { std::mem::take(&mut *app.pending.lock().unwrap_or_else(|e| e.into_inner())) };
    tokio::spawn(async move {
        for p in waiting {
            let e = match p {
                Push::Open(u) => Event::default().event("batiie-open").data(u),
                Push::Alert(t) => Event::default().event("batiie-alert").data(t),
                Push::Toast(t) => Event::default().event("batiie-toast").data(t),
                Push::Close(i) => Event::default().event("batiie-close").data(i),
            };
            if tx.send(Ok(e)).await.is_err() {
                return;
            }
        }
        let mut cache = page::Cache::default();
        loop {
            let mut event: Option<Event> = None;
            // quick while things change, relaxed when nothing does (a click wakes it at once)
            let delay = if cache.quiet >= 6 { Duration::from_millis(1000) } else { Duration::from_millis(250) };
            tokio::select! {
                _ = tokio::time::sleep(delay) => {}
                _ = app.wake.notified() => {}
                p = bus.recv() => {
                    match p {
                        Ok(Push::Open(_) | Push::Alert(_) | Push::Toast(_)) if lite => continue,
                        Ok(Push::Open(u)) => event = Some(Event::default().event("batiie-open").data(u)),
                        Ok(Push::Alert(t)) => event = Some(Event::default().event("batiie-alert").data(t)),
                        Ok(Push::Toast(t)) => event = Some(Event::default().event("batiie-toast").data(t)),
                        Ok(Push::Close(i)) => event = Some(Event::default().event("batiie-close").data(i)),
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                        Err(_) => break,
                    }
                }
                _ = tx.closed() => break,
            }
            if let Some(e) = event {
                if tx.send(Ok(e)).await.is_err() {
                    break;
                }
                continue;
            }
            let html = if lite { page::diff_lite(&app, &mut cache) } else { page::diff(&app, &mut cache) };
            if !html.is_empty() && tx.send(Ok(Event::default().event("ui").data(html))).await.is_err() {
                break;
            }
        }
    });
    Sse::new(ReceiverStream::new(rx)).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}
