//! "Add Batiie to your browser": finds the browsers on this computer and walks
//! the person through adding the extension.
use super::actions::{done, modal, set_clipboard, Params};
use super::ui::*;
use super::Shared;
use crate::browsers::{detect_all, extension_dir, firefox_xpi, other_browser, Browser};
use crate::i18n::{t, tf};
use axum::extract::{Form, Path, State};
use axum::response::{IntoResponse, Response};
use axum::routing::{get as route_get, post};
use axum::Router;
use maud::{html, Markup};
use std::path::PathBuf;

pub fn routes(r: Router<Shared>) -> Router<Shared> {
    r.route("/dlg/browsers", route_get(open_dialog))
        .route("/dlg/browsers/add/{i}", post(add))
        .route("/dlg/browsers/copy", post(copy_folder))
        .route("/dlg/browsers/folder", post(open_folder))
}

fn launch(cmd: &[String], url: &str) {
    let Some((prog, args)) = cmd.split_first() else { return };
    let mut c = std::process::Command::new(prog);
    c.args(args).arg(url);
    #[cfg(not(windows))]
    c.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    let _ = c.spawn();
}

fn missing_folder_text() -> String {
    t("The extension folder was not found next to Batiie.exe. Install Batiie again.")
}

fn result(steps: &str, folder: &str) -> Markup {
    html! {
        div id="br-result" hx-swap-oob="true" {
            @if !steps.is_empty() {
                div class="br-steps" { b { (t("Almost done:")) } p class="sel-text" style="white-space:pre-line;margin:6px 0 0" { (steps) } }
            }
            @if !folder.is_empty() {
                div class="pathbox" { input class="in mono" type="text" id="br-folder" name="folder" value=(folder) readonly; }
                div class="row-flex" {
                    button type="button" class="btn" hx-post="/dlg/browsers/copy" hx-include="#br-folder" hx-swap="none" { (icon("copy")) (t("Copy folder path")) }
                    button type="button" class="btn" hx-post="/dlg/browsers/folder" hx-include="#br-folder" hx-swap="none" { (icon("folder")) (t("Open folder")) }
                }
            }
        }
    }
}

async fn open_dialog(State(app): State<Shared>) -> Response {
    let mut list = detect_all();
    let none_found = list.is_empty();
    list.push(other_browser());
    app.misc.lock().unwrap_or_else(|e| e.into_inner()).browsers = list.clone();
    let folder = extension_dir(false).map(|p| p.display().to_string()).unwrap_or_default();
    let steps = if none_found { t("No browser was found on this computer automatically. Click \"Other browser\" and follow the steps.") } else { String::new() };
    modal(dialog(
        "d-browsers",
        &t("Add Batiie to your browser"),
        "",
        None,
        html! {
            div class="dlg-body" {
                p style="margin:0" { (t("Click your browser:")) }
                div class="br-grid" {
                    @for (i, b) in list.iter().enumerate() {
                        button type="button" class="br-card" hx-post=(format!("/dlg/browsers/add/{i}")) hx-swap="none" {
                            span class="br-dot" style=(format!("background:rgb({},{},{})", b.tint.0, b.tint.1, b.tint.2)) { (b.name.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default()) }
                            span class="br-t" { span class="br-n" { (b.name) } span class="muted" {
                                (t(if b.generic { "Any other browser: copy the folder" } else if b.store.is_empty() { "Click to add" } else { "Click to add from the store" }))
                            } }
                        }
                    }
                }
                (result(&steps, &folder).into_string().replacen(r#" hx-swap-oob="true""#, "", 1).pipe_markup())
            }
            (footer(html! { span class="sp" {} (btn_close(&t("Close"))) }))
        },
    ))
}

trait PipeMarkup {
    fn pipe_markup(self) -> Markup;
}
impl PipeMarkup for String {
    fn pipe_markup(self) -> Markup {
        maud::PreEscaped(self)
    }
}

async fn add(State(app): State<Shared>, Path(i): Path<usize>) -> Response {
    let b: Option<Browser> = app.misc.lock().unwrap_or_else(|e| e.into_inner()).browsers.get(i).cloned();
    let Some(b) = b else { return done(&app) };
    if b.generic {
        // any browser: the folder path is copied, the steps are the same for all of them
        let Some(dir) = extension_dir(false) else { return result(&missing_folder_text(), "").into_response() };
        let path = dir.display().to_string();
        set_clipboard(path.clone());
        return result(&t("1. Open your browser's extensions page and turn on \"Developer mode\".\n2. Click \"Load unpacked\" (Firefox: \"Load Temporary Add-on…\").\n3. Paste the folder path (Ctrl+V – it is already copied) and choose it.\n\nFirefox-based browsers use the \"extension-firefox\" folder instead."), &path).into_response();
    }
    if !b.store.is_empty() {
        launch(&b.cmd, b.store);
        return result(&tf("{} opened the store page: click \"Add\" / \"Get\" there.", &[&b.name]), "").into_response();
    }
    if b.firefox {
        if let Some(xpi) = firefox_xpi() {
            // the signed add-on: Firefox asks "Add Batiie?" and keeps it
            launch(&b.cmd, &xpi.display().to_string());
            return result(&t("Firefox opened the Batiie add-on.\n1. Click \"Add\" in the message Firefox shows.\n2. Then click \"OK\".\n\nThe add-on is signed by Mozilla, so it stays in Firefox after you close it and updates by itself."), &xpi.display().to_string()).into_response();
        }
    }
    let Some(dir) = extension_dir(b.firefox) else { return result(&missing_folder_text(), "").into_response() };
    let path = dir.display().to_string();
    set_clipboard(path.clone());
    launch(&b.cmd, &b.extensions_page);
    let steps = if b.firefox {
        t("Firefox opened its add-ons debugging page.\n1. Click \"Load Temporary Add-on…\".\n2. Paste the folder path (Ctrl+V, it is already copied) and open the file \"manifest.json\" in it.\n\nFirefox removes add-ons loaded this way when it closes. To keep it for good the extension must be signed by Mozilla (free) – ask for the signed version.")
    } else {
        tf("{} opened its extensions page.\n1. Turn on \"Developer mode\" (a switch on that page).\n2. Click \"Load unpacked\".\n3. Paste the folder path (Ctrl+V – it is already copied) and click \"Select folder\".\n\nThat's all: the Batiie icon appears next to the address bar. You do this only once.", &[&b.name])
    };
    result(&steps, &path).into_response()
}

async fn copy_folder(State(app): State<Shared>, Form(p): Params) -> Response {
    if let Some(f) = p.get("folder") {
        set_clipboard(f.clone());
        app.toast(t("Copied"));
    }
    done(&app)
}

async fn open_folder(State(app): State<Shared>, Form(p): Params) -> Response {
    let f = p.get("folder").cloned().unwrap_or_default();
    if !f.is_empty() {
        // a file (the .xpi): its folder
        let p = PathBuf::from(&f);
        let target = if p.is_file() { p.parent().map(|x| x.to_path_buf()).unwrap_or(p) } else { p };
        let _ = open::that(target);
    }
    done(&app)
}
