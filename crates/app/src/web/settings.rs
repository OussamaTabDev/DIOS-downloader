//! Options: every setting of the program, ten tabs in one form. What is changed in the
//! window (theme, text size, picture) shows at once; Cancel puts the saved look back.
use super::actions::{closing, done, modal};
use super::ui::*;
use super::Shared;
use crate::i18n::{t, tf};
use crate::state::App;
use crate::{autostart, sound};
use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Response};
use axum::routing::{get as route_get, post};
use axum::Router;
use batiie_engine::config::{ServerLimit, Settings, SiteLogin, DEFAULT_CAPTURE_EXTS};
use batiie_engine::fmt_size;
use maud::{html, Markup};
use std::collections::HashMap;
use std::path::PathBuf;

pub const FONTS: [&str; 7] = ["", "Tahoma", "Arial", "Calibri", "Verdana", "Times New Roman", "Courier New"];
const KEYS: [&str; 4] = ["alt", "ctrl", "shift", "none"];

pub fn routes(r: Router<Shared>) -> Router<Shared> {
    r.route("/dlg/settings", route_get(open_dialog))
        .route("/dlg/settings/save", post(save))
        .route("/dlg/settings/preview", post(preview))
        .route("/dlg/settings/cancel", post(cancel))
        .route("/dlg/settings/server/{op}", post(server_op))
        .route("/dlg/settings/login/{op}", post(login_op))
        .route("/dlg/settings/resetpaths", post(reset_paths))
        .route("/dlg/settings/defaultexts", post(default_exts))
        .route("/dlg/settings/play/{which}", post(play))
}

/// A form body with repeated keys.
struct Body {
    map: HashMap<String, String>,
    all: Vec<(String, String)>,
}

impl Body {
    fn new(b: &Bytes) -> Body {
        let all: Vec<(String, String)> = url::form_urlencoded::parse(b).into_owned().collect();
        let map = all.iter().cloned().collect();
        Body { map, all }
    }
    fn s(&self, k: &str) -> String {
        self.map.get(k).map(|v| v.trim().to_string()).unwrap_or_default()
    }
    fn raw(&self, k: &str) -> String {
        self.map.get(k).cloned().unwrap_or_default()
    }
    fn on(&self, k: &str) -> bool {
        self.map.get(k).map(|v| !v.is_empty() && v != "0").unwrap_or(false)
    }
    fn n<T: std::str::FromStr>(&self, k: &str, default: T) -> T {
        self.s(k).parse().unwrap_or(default)
    }
    fn many(&self, k: &str) -> Vec<String> {
        self.all.iter().filter(|(key, _)| key == k).map(|(_, v)| v.clone()).collect()
    }
}

fn key_index(k: &str) -> usize {
    KEYS.iter().position(|x| *x == k).unwrap_or(3)
}

const TABS: [(&str, &str, &str); 10] = [
    ("general", "gear", "General"),
    ("filetypes", "file", "File Types"),
    ("saveto", "folder", "Save To"),
    ("downloads", "download", "Downloads"),
    ("connection", "wifi", "Connection"),
    ("proxy", "shield", "Proxy / Server"),
    ("logins", "lock", "Site Logins"),
    ("sounds", "volume", "Sounds"),
    ("limiter", "speed", "Speed Limiter"),
    ("appearance", "palette", "Appearance"),
];

/// Three colors of every theme for the pickers: window, toolbar, accent.
const SWATCH: [(&str, &str, &str); 9] = [
    ("#f7f6fa", "#fbf9fe", "#8800ff"),
    ("#1e1f22", "#2e2b3a", "#a64dff"),
    ("#e8f1fb", "#2f80ed", "#1565c0"),
    ("#15131d", "#2b2545", "#a64dff"),
    ("#eef5ef", "#2f9e5b", "#1f8a4c"),
    ("#000000", "#141414", "#a64dff"),
    ("#fbf1ea", "#f07b3f", "#d4513a"),
    ("#0f1c1e", "#1b3a3d", "#2ec4b6"),
    ("#fbf0f4", "#e05a8a", "#c23d6e"),
];

fn servers_list(rows: &[ServerLimit], oob: bool) -> Markup {
    html! {
        div id="srv-list" hx-swap-oob=[oob.then_some("true")] {
            div class="tbl" style="--tcols:1fr 110px 34px;max-height:150px" {
                div class="tr th" { span { (t("Server")) } span class="td-r" { (t("Connections")) } span {} }
                @for (i, s) in rows.iter().enumerate() {
                    div class="tr" {
                        input type="hidden" name="server" value=(format!("{}\t{}", s.host, s.connections));
                        span class="ell" { (s.host) }
                        span class="td-r" { (s.connections) }
                        button type="button" class="mini" title=(t("Delete")) hx-post=(format!("/dlg/settings/server/del?i={i}")) hx-include="#srv-list input" hx-target="#srv-list" hx-swap="outerHTML" { (icon("close")) }
                    }
                }
                @if rows.is_empty() { div class="tr muted" { span { "—" } } }
            }
        }
    }
}

fn logins_list(rows: &[SiteLogin], oob: bool) -> Markup {
    html! {
        div id="login-list" hx-swap-oob=[oob.then_some("true")] {
            div class="tbl" style="--tcols:1fr 1fr 60px;max-height:150px" {
                div class="tr th" { span { (t("Site / path")) } span { (t("User name")) } span {} }
                @for (i, l) in rows.iter().enumerate() {
                    div class="tr" {
                        input type="hidden" name="login" value=(format!("{}\t{}\t{}", l.site, l.user, l.password));
                        span class="ell" { (l.site) }
                        span class="ell" { (l.user) }
                        span class="row-flex" style="gap:2px;justify-content:flex-end" {
                            button type="button" class="mini" title=(t("Edit")) hx-post=(format!("/dlg/settings/login/edit?i={i}")) hx-include="#login-list input" hx-swap="none" { (icon("edit")) }
                            button type="button" class="mini" title=(t("Remove")) hx-post=(format!("/dlg/settings/login/del?i={i}")) hx-include="#login-list input" hx-target="#login-list" hx-swap="outerHTML" { (icon("close")) }
                        }
                    }
                }
                @if rows.is_empty() { div class="tr muted" { span { "—" } } }
            }
        }
    }
}

fn parse_servers(b: &Body) -> Vec<ServerLimit> {
    b.many("server")
        .iter()
        .filter_map(|v| {
            let (h, c) = v.split_once('\t')?;
            Some(ServerLimit { host: h.to_string(), connections: c.parse().ok()? })
        })
        .collect()
}

fn parse_logins(b: &Body) -> Vec<SiteLogin> {
    b.many("login")
        .iter()
        .filter_map(|v| {
            let mut it = v.splitn(3, '\t');
            Some(SiteLogin { site: it.next()?.to_string(), user: it.next()?.to_string(), password: it.next().unwrap_or("").to_string() })
        })
        .collect()
}

fn sound_row(name: &str, label: &str, value: &str, which: &str) -> Markup {
    let id = format!("p-{name}");
    html! {
        div class="fld" {
            span class="fld-l" { (label) }
            div class="pathbox" id=(id) {
                input class="in" type="text" name=(name) value=(value) autocomplete="off" spellcheck="false";
                button type="button" class="btn" title=(t("Browse…")) hx-post="/pick/sound" hx-include=(format!("#{id} input")) hx-target=(format!("#{id} input")) hx-swap="outerHTML" hx-vals=(format!(r#"{{"name":"{name}"}}"#)) { (icon("folder")) }
                button type="button" class="btn" title=(t("Play")) hx-post=(format!("/dlg/settings/play/{which}")) hx-include=(format!("#{id} input")) hx-swap="none" { (icon("play")) }
            }
        }
    }
}

fn panel(body: Markup) -> Markup {
    html! { div class="tabpanel" { (body) } }
}

fn form_markup(app: &App, s: &Settings, tab: &str) -> Markup {
    let tab_i = TABS.iter().position(|(k, _, _)| *k == tab).unwrap_or(0);
    let lang_cur = crate::i18n::effective(&s.language);
    let langs: Vec<(String, String)> = crate::i18n::LANGS.iter().map(|(c, l)| (c.to_string(), l.to_string())).collect();
    let fonts: Vec<(String, String)> = FONTS.iter().map(|f| (f.to_string(), if f.is_empty() { t("Default (Segoe UI)") } else { f.to_string() })).collect();
    let keys = || -> Vec<(String, String)> { ["Alt", "Ctrl", "Shift", "None"].iter().enumerate().map(|(i, l)| (KEYS[i].to_string(), t(l))).collect() };
    let quota_used = if s.quota_on {
        tf("Downloaded in the current period: {}", &[&fmt_size(app.mgr.quota_used())])
    } else {
        t("When the limit is reached the downloads stop, and go on by themselves later.")
    };
    let theme_names = ["Light", "Dark", "Ocean blue", "Midnight purple", "Forest green", "Black", "Sunset", "Teal night", "Rose"];
    html! {
        form id="st-form" class="tabs-form" hx-post="/dlg/settings/save" hx-swap="none" {
            div class="tabs" {
                @for (i, _) in TABS.iter().enumerate() { input type="radio" class="tabr" name="tab" id={"st-tab-" (i)} value=(TABS[i].0) checked[i == tab_i]; }
                div class="tablist" role="tablist" {
                    @for (i, (_, ico, label)) in TABS.iter().enumerate() { label for={"st-tab-" (i)} role="tab" { (icon(ico)) (t(label)) } }
                }
                div class="tabpanels" {
                    // ---- General
                    (panel(html! {
                        (field(&t("Language"), select("language", &langs, lang_cur)))
                        (check("start_with_windows", &t("Launch Batiie on Windows startup"), autostart::is_enabled()))
                        (check("close_to_tray", &t("Keep running next to the clock when the window is closed"), s.close_to_tray))
                        (check("clipboard", &t("Watch the clipboard for download links"), s.clipboard_monitor))
                        (check("auto_clipboard", &t("Automatically start downloading links placed in the clipboard"), s.auto_start_clipboard))
                        (check("show_tips", &t("Show \"Tip of the day\" at startup"), s.show_tips))
                        (field(&t("On double click on a download"), select("double_click", &opts(&["Open the file / progress window", "Show properties"]), &s.double_click.min(1).to_string())))
                        (section(&t("Browser integration"), html! {
                            (hint(&t("Chrome and Edge: load the \"extension\" folder in chrome://extensions (Developer mode > Load unpacked). Downloads, videos and the right click menu then go to Batiie.")))
                            div { button type="button" class="btn" hx-get="/dlg/browsers" hx-target="#modal-host" hx-swap="beforeend" { (icon("puzzle")) (t("Add Batiie to my browser…")) } }
                            (field(&t("Hold this key to let the browser download by itself"), select("skip_key", &keys(), KEYS[key_index(&s.skip_key)])))
                            (field(&t("Hold this key to force Batiie to take a link"), select("force_key", &keys(), KEYS[key_index(&s.force_key)])))
                            (field(&t("Browser extension port"), number("port", s.browser_port, 1024, 65535)))
                        }))
                    }))
                    // ---- File types
                    (panel(html! {
                        (hint(&t("Automatically start downloading these file types (from the browser):")))
                        div id="capture-box" { textarea class="in mono" name="capture_exts" rows="4" spellcheck="false" { (s.capture_exts) } }
                        div { button type="button" class="btn sm" hx-post="/dlg/settings/defaultexts" hx-target="#capture-box" hx-swap="innerHTML" { (t("Default")) } }
                        (hint(&t("Don't start downloading automatically from these sites:")))
                        input class="in" type="text" name="skip_sites" value=(s.skip_sites) placeholder=(t("*.update.microsoft.com  windowsupdate.com")) autocomplete="off" spellcheck="false";
                        (hint(&t("Don't start downloading these addresses (one per line, * allowed):")))
                        textarea class="in mono" name="skip_addresses" rows="3" spellcheck="false" { (s.skip_addresses.join("\n")) }
                        (check("panel_enabled", &t("Show the \"Download this video\" panel on web players"), s.panel_enabled))
                        (field(&t("Don't show it on these sites"), text("panel_skip_sites", &s.panel_skip_sites)))
                        div class="row-flex" {
                            (field(&t("Smallest video (KB)"), number("panel_min_video", s.panel_min_video_kb.min(100_000), 0, 100_000)))
                            (field(&t("Smallest audio (KB)"), number("panel_min_audio", s.panel_min_audio_kb.min(100_000), 0, 100_000)))
                        }
                    }))
                    // ---- Save to
                    (panel(html! {
                        (field(&t("Main download folder"), path("download_dir", &s.download_dir.display().to_string(), "folder")))
                        (check("category_folders", &t("Sort files into folders by category (Video, Music, Programs, Compressed, Documents, Images…)"), s.category_folders))
                        (hint(&t("Each category's own folder and file types: right click it in the sidebar > Properties.")))
                        div { button type="button" class="btn sm" hx-post="/dlg/settings/resetpaths" hx-swap="none" { (t("Forget remembered category paths")) } }
                        (field(&t("Temporary folder (parts while downloading)"), html! {
                            div class="pathbox" id="p-temp_dir" {
                                input class="in" type="text" name="temp_dir" value=(s.temp_dir.as_ref().map(|p| p.display().to_string()).unwrap_or_default()) placeholder=(t("next to the file (default)")) autocomplete="off" spellcheck="false";
                                button type="button" class="btn" hx-post="/pick/folder" hx-include="#p-temp_dir input" hx-target="#p-temp_dir input" hx-swap="outerHTML" hx-vals=r#"{"name":"temp_dir"}"# { (icon("folder")) }
                            }
                        }))
                        (hint(&t("Tip: a temporary folder on another drive than the download folder makes joining the parts faster.")))
                    }))
                    // ---- Downloads
                    (panel(html! {
                        (check("show_start_dialog", &t("Show the \"Download File Info\" window for downloads from the browser"), s.show_start_dialog))
                        (check("start_while_info", &t("Start downloading immediately while showing \"Download File Info\""), s.start_while_info))
                        (check("complete_dialog", &t("Show the \"Download complete\" window"), s.show_complete_dialog))
                        (field(&t("Show the download progress window"), select("progress_mode", &opts(&["Normally", "Minimized", "Don't show"]), &s.progress_mode.min(2).to_string())))
                        (field(&t("If a duplicate link is added"), select("duplicate_action", &opts(&["Ask me", "Add it with a numbered file name", "Add it and replace the old download", "Show the finished one / resume the unfinished one"]), &s.duplicate_action.min(3).to_string())))
                        (check("set_file_date", &t("Give finished files the date from the server"), s.set_file_date))
                        (check("virus_scan", &t("Check every finished file for viruses with this program"), s.virus_scan))
                        div data-enable="#st-form [name=virus_scan]" class="col" {
                            (path("virus_program", &s.virus_program, "file"))
                            (field(&t("Command line parameters"), html! { input class="in" type="text" name="virus_args" value=(s.virus_args) placeholder=(t("-Scan -ScanType 3 -File [File]")) autocomplete="off" spellcheck="false"; }))
                            (hint(&t("[File] is replaced by the downloaded file.")))
                        }
                        (field(&t("User-Agent for downloads added by hand"), text("user_agent", &s.user_agent)))
                    }))
                    // ---- Connection
                    (panel(html! {
                        (field(&t("Connection type / speed"), select("connection_type", &opts(&["Low speed (modem, mobile)", "Medium speed (DSL)", "High speed (fiber, cable, LAN)"]), &s.connection_type.min(2).to_string())))
                        div class="row-flex" {
                            (field(&t("Default max. connections number"), number("connections", s.connections(), 1, 32)))
                            (field(&t("Downloads at the same time (0 = no limit)"), number("max_active", s.max_active.min(100), 0, 100)))
                            (field(&t("Retries on error"), number("retries", s.retries.min(100), 0, 100)))
                        }
                        (section(&t("Exceptions: servers with their own number of connections"), html! {
                            (servers_list(&s.server_limits, false))
                            div class="row-flex" {
                                input class="in" style="flex:1" type="text" name="server_host" placeholder=(t("example.com")) autocomplete="off" spellcheck="false";
                                input class="in num" type="number" name="server_conns" value="4" min="1" max="32";
                                button type="button" class="btn" hx-post="/dlg/settings/server/add" hx-include="#srv-list input, [name=server_host], [name=server_conns]" hx-target="#srv-list" hx-swap="outerHTML" { (t("Add")) }
                            }
                        }))
                        (section(&t("Download limits"), html! {
                            div class="row-flex" {
                                (check("quota_on", &t("Download no more than"), s.quota_on))
                                (number("quota_mb", s.quota_mb.clamp(1, 10_000_000), 1, 10_000_000))
                                span { (t("MB every")) }
                                (number("quota_hours", s.quota_hours.clamp(1, 720), 1, 720))
                                span { (t("hours")) }
                            }
                            (hint(&quota_used))
                        }))
                        (section(&t("Torrents (BitTorrent)"), html! {
                            (check("torrent_seed", &t("Share finished torrents (seeding)"), s.torrent_seed))
                            (field(&t("Download limit for all torrents (KB/s, 0 = off)"), number("torrent_dl", s.torrent_dl_limit_kb.min(1_000_000), 0, 1_000_000)))
                            (field(&t("Upload limit for all torrents (KB/s, 0 = off)"), number("torrent_ul", s.torrent_ul_limit_kb.min(1_000_000), 0, 1_000_000)))
                            (hint(&t("Torrents connect to many computers; the limits apply to all of them together.")))
                        }))
                    }))
                    // ---- Proxy
                    (panel(html! {
                        @for (v, label) in [("0", "No proxy"), ("1", "Use the system settings (like your browser)"), ("2", "Manual proxy configuration (HTTP / HTTPS)")] {
                            label class="chk" { input type="radio" name="proxy_mode" value=(v) checked[s.proxy_mode.min(2).to_string() == v]; span class="box" { (icon("check")) } span class="chk-l" { (t(label)) } }
                        }
                        div class="col" id="proxy-fields" {
                            div class="row-flex" {
                                (field(&t("Proxy address"), html! { input class="in" type="text" name="proxy_host" value=(s.proxy_host) placeholder=(t("proxy.example.com")) autocomplete="off" spellcheck="false"; }))
                                (field(&t("Port"), number("proxy_port", s.proxy_port.max(1), 1, 65535)))
                            }
                            div class="row-flex" {
                                (field(&t("User name"), text("proxy_user", &s.proxy_user)))
                                (field(&t("Password"), password("proxy_password", &s.proxy_password)))
                            }
                            (field(&t("Don't use the proxy for"), html! { input class="in" type="text" name="proxy_bypass" value=(s.proxy_bypass) placeholder=(t("localhost 127.0.0.1 *.local")) autocomplete="off" spellcheck="false"; }))
                            (hint(&t("Addresses separated by spaces. SOCKS proxies: use the system settings.")))
                        }
                    }))
                    // ---- Site logins
                    (panel(html! {
                        (hint(&t("User names and passwords for sites that ask for them:")))
                        (logins_list(&s.logins, false))
                        div class="col" id="login-fields" {
                            (field(&t("Site / path"), html! { input class="in" type="text" id="lg-site" name="login_site" placeholder=(t("example.com/members")) autocomplete="off" spellcheck="false"; }))
                            (field(&t("User name"), html! { input class="in" type="text" id="lg-user" name="login_user" autocomplete="off"; }))
                            (field(&t("Password"), html! { input class="in" type="password" id="lg-pass" name="login_password" autocomplete="off"; }))
                            div class="row-flex" {
                                button type="button" class="btn" hx-post="/dlg/settings/login/add" hx-include="#login-list input, #login-fields input" hx-target="#login-list" hx-swap="outerHTML" { (t("Add / Update")) }
                            }
                        }
                        (hint(&t("Saved in the settings file of your Windows user account.")))
                    }))
                    // ---- Sounds
                    (panel(html! {
                        (check("sounds_on", &t("Play sounds"), s.sounds_on))
                        (sound_row("sound_complete", &t("Download complete"), &s.sound_complete, "complete"))
                        (sound_row("sound_failed", &t("Download failed"), &s.sound_failed, "failed"))
                        (sound_row("sound_qstart", &t("Queue started"), &s.sound_queue_start, "qstart"))
                        (sound_row("sound_qdone", &t("Queue finished"), &s.sound_queue_done, "qdone"))
                        (hint(&t(".wav files. Empty = no sound for that event.")))
                    }))
                    // ---- Speed limiter
                    (panel(html! {
                        (check("limiter_on", &t("Turn the speed limiter on"), s.speed_limit_on))
                        (field(&t("Maximum download speed (KB/s)"), number("speed_limit", s.speed_limit_kb.clamp(1, 1_000_000), 1, 1_000_000)))
                        (check("limiter_startup", &t("Always turn on the speed limiter when Batiie starts"), s.limiter_on_startup))
                        (hint(&t("Every download can also have its own limit: its progress window > Speed Limiter tab.")))
                    }))
                    // ---- Appearance
                    (panel(html! {
                        div class="fld top" {
                            span class="fld-l" { (t("Theme")) }
                            div class="themes" {
                                @for (i, name) in theme_names.iter().enumerate() {
                                    label class="theme" title=(t(name)) {
                                        input type="radio" name="theme" value=(i) checked[s.theme as usize == i] hx-post="/dlg/settings/preview" hx-include="#st-form [name=theme],#st-form [name=text_size],#st-form [name=font],#st-form [name=bg_image],#st-form [name=bg_strength]" hx-swap="none";
                                        span class="sw" style=(format!("background:{}", SWATCH[i].0)) { i style=(format!("background:{}", SWATCH[i].1)) {} b style=(format!("background:{}", SWATCH[i].2)) {} }
                                        span class="tn" { (t(name)) }
                                    }
                                }
                            }
                        }
                        div class="fld" {
                            span class="fld-l" { (t("Text size")) }
                            div class="row-flex" style="flex-wrap:nowrap" { input class="range" type="range" name="text_size" min="80" max="150" step="5" value=(s.font_scale.clamp(80, 150)) hx-post="/dlg/settings/preview" hx-trigger="input changed delay:80ms" hx-include="#st-form [name=theme],#st-form [name=text_size],#st-form [name=font],#st-form [name=bg_image],#st-form [name=bg_strength]" hx-swap="none"; output class="muted" { (format!("{}%", s.font_scale.clamp(80, 150))) } }
                        }
                        div class="fld" {
                            span class="fld-l" { (t("Font")) }
                            select class="in sel" name="font" hx-post="/dlg/settings/preview" hx-include="#st-form [name=theme],#st-form [name=text_size],#st-form [name=font],#st-form [name=bg_image],#st-form [name=bg_strength]" hx-swap="none" {
                                @for (v, l) in &fonts { option value=(v) selected[*v == s.font_family] { (l) } }
                            }
                        }
                        div class="fld" {
                            span class="fld-l" { (t("Background picture")) }
                            div class="pathbox" id="p-bg_image" {
                                input class="in" type="text" name="bg_image" value=(s.background_image) placeholder=(t("none (the theme's colors)")) autocomplete="off" spellcheck="false" hx-post="/dlg/settings/preview" hx-trigger="change" hx-include="#st-form [name=theme],#st-form [name=text_size],#st-form [name=font],#st-form [name=bg_image],#st-form [name=bg_strength]" hx-swap="none";
                                button type="button" class="btn" hx-post="/pick/image" hx-include="#p-bg_image input" hx-target="#p-bg_image input" hx-swap="outerHTML" hx-vals=r#"{"name":"bg_image"}"# { (icon("image")) (t("Browse…")) }
                                button type="button" class="btn" data-clear="#p-bg_image input" { (t("Remove")) }
                            }
                        }
                        div class="fld" {
                            span class="fld-l" { (t("Picture visibility")) }
                            input class="range" type="range" name="bg_strength" min="10" max="80" step="5" value=(s.background_strength.clamp(10, 80)) hx-post="/dlg/settings/preview" hx-trigger="input changed delay:80ms" hx-include="#st-form [name=theme],#st-form [name=text_size],#st-form [name=font],#st-form [name=bg_image],#st-form [name=bg_strength]" hx-swap="none";
                        }
                    }))
                }
            }
            div class="st-msg" id="st-msg" {}
            (footer(html! {
                span class="sp" {}
                (primary(&t("OK")))
                button type="button" class="btn" data-close { (t("Cancel")) }
            }))
        }
    }
}

async fn open_dialog(State(app): State<Shared>, Query(q): Query<HashMap<String, String>>) -> Response {
    let s = app.mgr.settings();
    let tab = q.get("tab").map(|s| s.as_str()).unwrap_or("general");
    modal(dialog("d-settings", &t("Batiie – Options"), "xl", Some("/dlg/settings/cancel"), form_markup(&app, &s, tab)))
}

fn look_oob(s: &Settings) -> String {
    format!(r#"<div id="look" hx-swap-oob="true" data-look="{}" hidden></div>"#, super::page::look_attrs(s))
}

/// What the look would be with the values in the window (not saved).
async fn preview(State(app): State<Shared>, body: Bytes) -> Response {
    let b = Body::new(&body);
    let mut s = app.mgr.settings();
    s.theme = b.n::<u32>("theme", s.theme).clamp(0, 8);
    s.font_scale = b.n::<u32>("text_size", s.font_scale).clamp(80, 150);
    let f = b.raw("font");
    s.font_family = if FONTS.contains(&f.as_str()) { f } else { String::new() };
    s.background_image = b.s("bg_image");
    s.background_strength = b.n::<u32>("bg_strength", s.background_strength).clamp(10, 80);
    maud::PreEscaped(look_oob(&s)).into_response()
}

/// Cancel / the cross: the saved look comes back.
async fn cancel(State(app): State<Shared>) -> Response {
    let s = app.mgr.settings();
    maud::PreEscaped(look_oob(&s)).into_response()
}

async fn save(State(app): State<Shared>, body: Bytes) -> Response {
    let b = Body::new(&body);
    let err = |m: String| html! { div id="st-msg" hx-swap-oob="true" class="st-msg" { (msg_err(&m)) } }.into_response();
    let old = app.mgr.settings();
    let mut s = old.clone();
    let dir = b.s("download_dir");
    if dir.is_empty() {
        return err(t("Choose a download folder (Save To)"));
    }
    if b.n::<u32>("proxy_mode", 0) == 2 && b.s("proxy_host").is_empty() {
        return err(t("Type the proxy address (Proxy / Server)"));
    }
    // General
    let lang = b.s("language");
    s.language = if crate::i18n::LANGS.iter().any(|(c, _)| *c == lang) { lang } else { String::new() };
    s.theme = b.n::<u32>("theme", s.theme).clamp(0, 8);
    s.font_scale = b.n::<u32>("text_size", s.font_scale).clamp(80, 150);
    let f = b.raw("font");
    s.font_family = if FONTS.contains(&f.as_str()) { f } else { String::new() };
    s.background_image = b.s("bg_image");
    s.background_strength = b.n::<u32>("bg_strength", s.background_strength).clamp(10, 80);
    s.close_to_tray = b.on("close_to_tray");
    s.clipboard_monitor = b.on("clipboard");
    s.auto_start_clipboard = b.on("auto_clipboard");
    s.double_click = b.n::<u32>("double_click", 0).clamp(0, 1);
    s.show_tips = b.on("show_tips");
    s.browser_port = b.n::<u32>("port", old.browser_port as u32).clamp(1024, 65535) as u16;
    s.skip_key = KEYS[key_index(&b.s("skip_key"))].into();
    s.force_key = KEYS[key_index(&b.s("force_key"))].into();
    if b.on("start_with_windows") != autostart::is_enabled() {
        autostart::set(b.on("start_with_windows"));
    }
    // File types
    s.capture_exts = b.raw("capture_exts").split_whitespace().collect::<Vec<_>>().join(" ");
    s.skip_sites = b.s("skip_sites");
    s.skip_addresses = b.raw("skip_addresses").lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect();
    s.panel_enabled = b.on("panel_enabled");
    s.panel_skip_sites = b.s("panel_skip_sites");
    s.panel_min_video_kb = b.n::<u64>("panel_min_video", 0);
    s.panel_min_audio_kb = b.n::<u64>("panel_min_audio", 0);
    // Save to
    s.download_dir = PathBuf::from(dir);
    s.category_folders = b.on("category_folders");
    let temp = b.s("temp_dir");
    s.temp_dir = (!temp.is_empty()).then(|| PathBuf::from(temp));
    // Downloads
    s.show_start_dialog = b.on("show_start_dialog");
    s.start_while_info = b.on("start_while_info");
    s.show_complete_dialog = b.on("complete_dialog");
    s.progress_mode = b.n::<u32>("progress_mode", 0).clamp(0, 2);
    s.show_progress_window = s.progress_mode != 2;
    s.duplicate_action = b.n::<u32>("duplicate_action", 0).clamp(0, 3);
    s.set_file_date = b.on("set_file_date");
    s.virus_scan = b.on("virus_scan");
    s.virus_program = b.s("virus_program");
    s.virus_args = b.s("virus_args");
    let ua = b.s("user_agent");
    if !ua.is_empty() {
        s.user_agent = ua;
    }
    // Connection
    s.connection_type = b.n::<u32>("connection_type", 0).clamp(0, 2);
    s.connections = b.n::<u32>("connections", 8).clamp(1, 32);
    s.max_active = b.n::<u32>("max_active", 0).clamp(0, 100);
    s.retries = b.n::<u32>("retries", 0);
    s.quota_on = b.on("quota_on");
    s.quota_mb = b.n::<u64>("quota_mb", 1).max(1);
    s.quota_hours = b.n::<u32>("quota_hours", 1).max(1);
    s.torrent_seed = b.on("torrent_seed");
    s.torrent_dl_limit_kb = b.n::<u64>("torrent_dl", 0);
    s.torrent_ul_limit_kb = b.n::<u64>("torrent_ul", 0);
    // Proxy
    s.proxy_mode = b.n::<u32>("proxy_mode", 0).clamp(0, 2);
    s.proxy_host = b.s("proxy_host");
    s.proxy_port = b.n::<u32>("proxy_port", 8080).clamp(1, 65535) as u16;
    s.proxy_user = b.s("proxy_user");
    s.proxy_password = b.raw("proxy_password");
    s.proxy_bypass = b.s("proxy_bypass");
    // Lists
    s.server_limits = parse_servers(&b);
    s.logins = parse_logins(&b);
    // Sounds
    s.sounds_on = b.on("sounds_on");
    s.sound_complete = b.s("sound_complete");
    s.sound_failed = b.s("sound_failed");
    s.sound_queue_start = b.s("sound_qstart");
    s.sound_queue_done = b.s("sound_qdone");
    // Speed limiter
    s.speed_limit_on = b.on("limiter_on");
    s.speed_limit_kb = b.n::<u64>("speed_limit", 500).max(1);
    s.limiter_on_startup = b.on("limiter_startup");

    let port_changed = s.browser_port != old.browser_port;
    let lang_changed = crate::i18n::effective(&s.language) != crate::i18n::effective(&old.language);
    let eff = crate::i18n::effective(&s.language).to_string();
    if let Err(e) = app.mgr.set_settings(s) {
        return err(tf("Could not save: {}", &[&format!("{e:#}")]));
    }
    if lang_changed {
        crate::i18n::apply(&eff);
        app.push(crate::state::Push::Open("reload".into()));
    }
    if port_changed {
        app.alert(t("Restart Batiie to use the new port. Set the same port in the browser extension."));
    }
    // the dialog is closed by the server; a saved look stays
    let mut r = closing(&app, "d-settings");
    r.headers_mut().insert("hx-reswap", "none".parse().unwrap());
    r
}

async fn server_op(State(app): State<Shared>, Path(op): Path<String>, Query(q): Query<HashMap<String, String>>, body: Bytes) -> Response {
    let _ = &app;
    let b = Body::new(&body);
    let mut rows = parse_servers(&b);
    match op.as_str() {
        "add" => {
            let host = b.s("server_host");
            let host = host.trim_start_matches("https://").trim_start_matches("http://").split('/').next().unwrap_or("").to_string();
            if host.is_empty() {
                app.alert(t("Type the server, e.g. example.com"));
            } else {
                let conns = b.n::<u32>("server_conns", 4).clamp(1, 32);
                match rows.iter_mut().find(|s| s.host.eq_ignore_ascii_case(&host)) {
                    Some(s) => s.connections = conns,
                    None => rows.push(ServerLimit { host, connections: conns }),
                }
            }
        }
        "del" => {
            if let Some(i) = q.get("i").and_then(|i| i.parse::<usize>().ok()) {
                if i < rows.len() {
                    rows.remove(i);
                }
            }
        }
        _ => {}
    }
    servers_list(&rows, false).into_response()
}

async fn login_op(State(app): State<Shared>, Path(op): Path<String>, Query(q): Query<HashMap<String, String>>, body: Bytes) -> Response {
    let b = Body::new(&body);
    let mut rows = parse_logins(&b);
    let idx = q.get("i").and_then(|i| i.parse::<usize>().ok());
    match op.as_str() {
        "add" => {
            let (site, user, password) = (b.s("login_site"), b.s("login_user"), b.raw("login_password"));
            if site.is_empty() || user.is_empty() {
                app.alert(t("Type the site and the user name"));
            } else {
                match rows.iter_mut().find(|x| x.site.eq_ignore_ascii_case(&site)) {
                    Some(x) => {
                        x.user = user;
                        x.password = password;
                    }
                    None => rows.push(SiteLogin { site, user, password }),
                }
                // the fields are cleared
                return html! {
                    (logins_list(&rows, false))
                    input class="in" type="text" id="lg-site" name="login_site" hx-swap-oob="true" placeholder=(t("example.com/members")) autocomplete="off" spellcheck="false";
                    input class="in" type="text" id="lg-user" name="login_user" hx-swap-oob="true" autocomplete="off";
                    input class="in" type="password" id="lg-pass" name="login_password" hx-swap-oob="true" autocomplete="off";
                }
                .into_response();
            }
        }
        "del" => {
            if let Some(i) = idx.filter(|i| *i < rows.len()) {
                rows.remove(i);
            }
        }
        // put a row back into the fields
        "edit" => {
            if let Some(l) = idx.and_then(|i| rows.get(i)) {
                return html! {
                    input class="in" type="text" id="lg-site" name="login_site" value=(l.site) hx-swap-oob="true" autocomplete="off" spellcheck="false";
                    input class="in" type="text" id="lg-user" name="login_user" value=(l.user) hx-swap-oob="true" autocomplete="off";
                    input class="in" type="password" id="lg-pass" name="login_password" value=(l.password) hx-swap-oob="true" autocomplete="off";
                }
                .into_response();
            }
            return done(&app);
        }
        _ => {}
    }
    logins_list(&rows, false).into_response()
}

async fn reset_paths(State(app): State<Shared>) -> Response {
    let mut s = app.mgr.settings();
    s.category_dirs.clear();
    let _ = app.mgr.set_settings(s);
    app.alert(t("The remembered category folders were cleared."));
    done(&app)
}

async fn default_exts() -> Markup {
    html! { textarea class="in mono" name="capture_exts" rows="4" spellcheck="false" { (DEFAULT_CAPTURE_EXTS) } }
}

async fn play(Path(_which): Path<String>, body: Bytes) -> Response {
    let b = Body::new(&body);
    // the box that was asked is the only sound field in the request
    let path = ["sound_complete", "sound_failed", "sound_qstart", "sound_qdone"].iter().map(|k| b.s(k)).find(|v| !v.is_empty()).unwrap_or_default();
    sound::play(&path);
    axum::http::StatusCode::NO_CONTENT.into_response()
}
