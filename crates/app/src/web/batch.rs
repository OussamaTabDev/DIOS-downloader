//! "Add batch download": a list of links, or an address with a * and a range.
use super::actions::{closing, extract_links, flag, get, modal, Params};
use super::ui::*;
use super::Shared;
use crate::i18n::{t, tf};
use crate::state::App;
use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum::routing::{get as route_get, post};
use axum::Router;
use batiie_engine::config::MAIN_QUEUE;
use batiie_engine::{NewDownload, RequestHeaders};
use maud::{html, Markup};
use std::collections::HashMap;
use std::path::PathBuf;

pub fn routes(r: Router<Shared>) -> Router<Shared> {
    r.route("/dlg/batch", route_get(open_dialog))
        .route("/dlg/batch/preview", post(preview))
        .route("/dlg/batch/paste", post(paste))
        .route("/dlg/batch/ok", post(ok))
}

/// Open the window with these links in it (pasted text, a drop).
pub fn open_with(app: &App, text: String) {
    app.misc.lock().unwrap_or_else(|e| e.into_inner()).batch_text = text;
    app.open("/dlg/batch");
}

/// "img*.jpg" + from/to -> the list of addresses (numbers or letters).
pub fn expand_pattern(pattern: &str, letters: bool, from: &str, to: &str, size: usize) -> Result<Vec<String>, String> {
    let pattern = pattern.trim();
    if !(pattern.starts_with("http://") || pattern.starts_with("https://")) {
        return Err(t("The address must start with http:// or https://"));
    }
    if !pattern.contains('*') {
        return Err(t("Put a * in the address where the number goes"));
    }
    let mut out = Vec::new();
    if letters {
        let a = from.trim().chars().next().filter(|c| c.is_ascii_alphabetic()).ok_or_else(|| t("From must be a letter"))?;
        let b = to.trim().chars().next().filter(|c| c.is_ascii_alphabetic()).ok_or_else(|| t("To must be a letter"))?;
        let (a, b) = (a as u8, b as u8);
        if a > b {
            return Err(t("From must come before To"));
        }
        for c in a..=b {
            out.push(pattern.replace('*', &(c as char).to_string()));
        }
    } else {
        let a: u64 = from.trim().parse().map_err(|_| t("From must be a number"))?;
        let b: u64 = to.trim().parse().map_err(|_| t("To must be a number"))?;
        if a > b {
            return Err(t("From must be smaller than To"));
        }
        if b - a >= 10_000 {
            return Err(t("That's more than 10000 files"));
        }
        for n in a..=b {
            out.push(pattern.replace('*', &format!("{n:0size$}")));
        }
    }
    Ok(out)
}

fn pattern_links(p: &HashMap<String, String>) -> Result<Vec<String>, String> {
    let size = get(p, "size").parse::<usize>().unwrap_or(1).clamp(1, 10);
    expand_pattern(&get(p, "pattern"), flag(p, "letters"), &get(p, "from"), &get(p, "to"), size)
}

fn preview_text(p: &HashMap<String, String>) -> String {
    match pattern_links(p) {
        Ok(l) if l.is_empty() => String::new(),
        Ok(l) => {
            let mut s = tf("{} files\n\nFirst:  {}", &[&l.len(), &l[0]]);
            if l.len() > 1 {
                s.push_str(&tf("\nSecond:  {}", &[&l[1]]));
            }
            if l.len() > 2 {
                s.push_str(&tf("\nLast:  {}", &[&l[l.len() - 1]]));
            }
            s
        }
        Err(e) => e,
    }
}

fn preview_box(text: &str) -> Markup {
    html! {
        div id="bpreview" class="mono sel-text hint" style="white-space:pre-wrap;word-break:break-all;min-height:3.2em" { (text) }
    }
}

async fn open_dialog(State(app): State<Shared>) -> Response {
    let text = std::mem::take(&mut app.misc.lock().unwrap_or_else(|e| e.into_inner()).batch_text);
    let s = app.mgr.settings();
    let view = app.ui().view.clone();
    let current = view.strip_prefix("queue:").and_then(|q| q.parse::<u64>().ok()).unwrap_or(MAIN_QUEUE);
    let queues: Vec<(String, String)> = s.queues.iter().map(|q| (q.id.to_string(), q.name.clone())).collect();
    let cur = if s.queue(current).is_some() { current } else { MAIN_QUEUE };
    modal(dialog(
        "d-batch",
        &t("Add batch download"),
        "wide",
        None,
        html! {
            form hx-post="/dlg/batch/ok" hx-swap="none" {
                div class="dlg-body modes" {
                    div class="row-flex" {
                        label class="chk" { input type="radio" name="mode" value="0" checked; span class="box" { (icon("check")) } span class="chk-l" { (t("List of links")) } }
                        label class="chk" { input type="radio" name="mode" value="1"; span class="box" { (icon("check")) } span class="chk-l" { (t("Address with a pattern (*)")) } }
                    }
                    div data-mode="0" class="col" {
                        (hint(&t("Paste the links, one per line. They download one after another in the queue you choose.")))
                        textarea id="blinks" class="in mono" name="links" rows="9" spellcheck="false" placeholder="https://" { (extract_links(&text).join("\n")) }
                        div { button type="button" class="btn sm" hx-post="/dlg/batch/paste" hx-include="#blinks" hx-target="#blinks" hx-swap="outerHTML" { (icon("copy")) (t("Paste from clipboard")) } }
                    }
                    div data-mode="1" class="col" {
                        (hint(&t("Type an address with a * where the number (or letter) goes, for example\nhttps://www.site.com/pictures/img*.jpg")))
                        input class="in" type="text" name="pattern" placeholder=(t("https://www.site.com/pictures/img*.jpg")) autocomplete="off" spellcheck="false"
                            hx-post="/dlg/batch/preview" hx-trigger="input changed delay:150ms" hx-include="closest form" hx-target="#bpreview" hx-swap="outerHTML";
                        (check("letters", &t("Letters (a-z) instead of numbers"), false))
                        div class="row-flex" hx-post="/dlg/batch/preview" hx-trigger="input changed delay:150ms, change" hx-include="closest form" hx-target="#bpreview" hx-swap="outerHTML" {
                            (field(&t("From"), text_in("from", "1")))
                            (field(&t("To"), text_in("to", "10")))
                            (field(&t("Wildcard size"), number("size", 1, 1, 10)))
                        }
                        (hint(&t("Wildcard size 3 makes 001, 002 … 010.")))
                        (preview_box(""))
                        div class="row-flex" {
                            (field(&t("Login"), html! { input class="in" type="text" name="user" placeholder=(t("optional")) autocomplete="off"; }))
                            (field(&t("Password"), password("password", "")))
                        }
                    }
                    div class="col" {
                        (field(&t("Queue"), select("queue", &queues, &cur.to_string())))
                        (field(&t("Save all to"), html! {
                            div class="pathbox" id="p-folder" {
                                input class="in" type="text" name="folder" placeholder=(t("automatic (by file type)")) autocomplete="off" spellcheck="false";
                                button type="button" class="btn" hx-post="/pick/folder" hx-include="#p-folder input" hx-target="#p-folder input" hx-swap="outerHTML" hx-vals=r#"{"name":"folder"}"# { (icon("folder")) }
                            }
                        }))
                        (check("start", &t("Start the queue now"), true))
                    }
                    div id="bmsg" {}
                }
                (footer(html! {
                    (primary(&t("Add")))
                    (btn_close(&t("Cancel")))
                }))
            }
        },
    ))
}

fn text_in(name: &str, v: &str) -> Markup {
    html! { input class="in num" type="text" name=(name) value=(v) autocomplete="off"; }
}

async fn preview(Form(p): Params) -> Markup {
    preview_box(&preview_text(&p))
}

use axum::extract::Form;

async fn paste(Form(p): Params) -> Markup {
    let mut links = p.get("links").cloned().unwrap_or_default();
    if let Some(text) = arboard::Clipboard::new().ok().and_then(|mut c| c.get_text().ok()) {
        if !links.is_empty() && !links.ends_with('\n') {
            links.push('\n');
        }
        links.push_str(text.trim());
    }
    html! { textarea id="blinks" class="in mono" name="links" rows="9" spellcheck="false" placeholder="https://" { (links) } }
}

async fn ok(State(app): State<Shared>, Form(p): Params) -> Response {
    let (links, auth) = if get(&p, "mode") == "1" {
        match pattern_links(&p) {
            Ok(l) => {
                let user = get(&p, "user");
                (l, (!user.is_empty()).then(|| (user, p.get("password").cloned().unwrap_or_default())))
            }
            Err(e) => return error(&e),
        }
    } else {
        (extract_links(p.get("links").map(|s| s.as_str()).unwrap_or("")), None)
    };
    if links.is_empty() {
        return error(&t("No valid http:// or https:// links"));
    }
    let s = app.mgr.settings();
    let queue = get(&p, "queue").parse::<u64>().ok().filter(|q| s.queue(*q).is_some()).unwrap_or(MAIN_QUEUE);
    let folder = get(&p, "folder");
    for url in links {
        app.mgr.add(NewDownload {
            url,
            save_dir: (!folder.is_empty()).then(|| PathBuf::from(&folder)),
            headers: RequestHeaders { auth: auth.clone(), ..Default::default() },
            queue: Some(queue),
            start: false,
            ..Default::default()
        });
    }
    if flag(&p, "start") {
        app.mgr.start_queue(queue);
    }
    {
        let mut ui = app.ui();
        ui.view = format!("queue:{queue}");
        ui.sel.clear();
    }
    closing(&app, "d-batch")
}

fn error(msg: &str) -> Response {
    let mut r = html! { div id="bmsg" hx-swap-oob="true" { (msg_err(msg)) } }.into_response();
    // the form target is "none": the message is an out-of-band swap
    *r.status_mut() = axum::http::StatusCode::OK;
    r
}

#[cfg(test)]
mod tests {
    use super::expand_pattern;

    #[test]
    fn numbers_are_padded() {
        let l = expand_pattern("https://a.com/img*.jpg", false, "1", "12", 3).unwrap();
        assert_eq!(l.len(), 12);
        assert_eq!(l[0], "https://a.com/img001.jpg");
        assert_eq!(l[11], "https://a.com/img012.jpg");
    }

    #[test]
    fn letters() {
        let l = expand_pattern("https://a.com/*.zip", true, "a", "c", 1).unwrap();
        assert_eq!(l, ["https://a.com/a.zip", "https://a.com/b.zip", "https://a.com/c.zip"]);
    }

    #[test]
    fn bad_input_is_explained() {
        assert!(expand_pattern("ftp://a.com/*", false, "1", "2", 1).is_err());
        assert!(expand_pattern("https://a.com/x", false, "1", "2", 1).is_err());
        assert!(expand_pattern("https://a.com/*", false, "5", "2", 1).is_err());
        assert!(expand_pattern("https://a.com/*", false, "1", "99999", 1).is_err());
        assert!(expand_pattern("https://a.com/*", true, "1", "b", 1).is_err());
    }
}
