//! Small building blocks every page uses: icons, the dialog frame, form fields.
use crate::i18n::t;
use maud::{html, Markup, PreEscaped};

/// The icons, drawn once as an SVG sprite (24 x 24, outline style).
const ICONS: &[(&str, &str)] = &[
    ("add", r#"<circle cx="12" cy="12" r="9"/><path d="M12 8v8M8 12h8"/>"#),
    ("plus", r#"<path d="M12 5v14M5 12h14"/>"#),
    ("batch", r#"<rect x="4" y="3" width="13" height="16" rx="2"/><path d="M8 8h5M8 12h5M20 7v14H8"/>"#),
    ("play", r#"<path d="M7 4.5v15l12-7.5z"/>"#),
    ("pause", r#"<rect x="6" y="5" width="4" height="14" rx="1"/><rect x="14" y="5" width="4" height="14" rx="1"/>"#),
    ("stop", r#"<rect x="6" y="6" width="12" height="12" rx="2"/>"#),
    ("stopall", r#"<circle cx="12" cy="12" r="9"/><rect x="9" y="9" width="6" height="6" rx="1"/>"#),
    ("playall", r#"<circle cx="12" cy="12" r="9"/><path d="M10 8.5v7l6-3.5z"/>"#),
    ("trash", r#"<path d="M4 7h16M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3"/>"#),
    ("folder", r#"<path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>"#),
    ("open", r#"<path d="M14 4h6v6M20 4l-9 9M18 14v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1h5"/>"#),
    ("link", r#"<path d="M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1 1M14 10a4 4 0 0 0-5.7 0l-3 3A4 4 0 0 0 11 18.7l1-1"/>"#),
    ("gear", r#"<circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z"/>"#),
    ("clock", r#"<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/>"#),
    ("queue", r#"<path d="M4 6h16M4 12h16M4 18h10"/><path d="M18 15l3 3-3 3"/>"#),
    ("speed", r#"<path d="M4 18a8 8 0 1 1 16 0"/><path d="M12 18l4-6"/>"#),
    ("search", r#"<circle cx="11" cy="11" r="7"/><path d="M20 20l-4-4"/>"#),
    ("copy", r#"<rect x="9" y="9" width="11" height="11" rx="2"/><path d="M5 15V6a2 2 0 0 1 2-2h9"/>"#),
    ("refresh", r#"<path d="M20 11a8 8 0 0 0-14.6-3M4 4v4h4M4 13a8 8 0 0 0 14.6 3M20 20v-4h-4"/>"#),
    ("edit", r#"<path d="M4 20h4L19 9a2.8 2.8 0 0 0-4-4L4 16z"/>"#),
    ("check", r#"<path d="M5 12.5l4.5 4.5L19 7.5"/>"#),
    ("close", r#"<path d="M6 6l12 12M18 6L6 18"/>"#),
    ("magnet", r#"<path d="M5 3v8a7 7 0 0 0 14 0V3h-4v8a3 3 0 0 1-6 0V3z"/><path d="M5 7h4M15 7h4"/>"#),
    ("hide", r#"<path d="M3 3l18 18M10.6 6.2A9 9 0 0 1 12 6c5 0 8.5 4.5 9.5 6a14 14 0 0 1-3 3.4M6.5 7.6A14 14 0 0 0 2.5 12C3.5 13.5 7 18 12 18a8.6 8.6 0 0 0 3.4-.7"/>"#),
    ("info", r#"<circle cx="12" cy="12" r="9"/><path d="M12 11v5M12 8h.01"/>"#),
    ("columns", r#"<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M9 4v16M15 4v16"/>"#),
    ("up", r#"<path d="M6 14l6-6 6 6"/>"#),
    ("down", r#"<path d="M6 10l6 6 6-6"/>"#),
    ("right", r#"<path d="M10 6l6 6-6 6"/>"#),
    ("left", r#"<path d="M14 6l-6 6 6 6"/>"#),
    ("arrowup", r#"<path d="M12 19V5M6 11l6-6 6 6"/>"#),
    ("arrowdown", r#"<path d="M12 5v14M6 13l6 6 6-6"/>"#),
    ("download", r#"<path d="M12 4v11M7 11l5 5 5-5M5 20h14"/>"#),
    ("upload", r#"<path d="M12 16V5M7 9l5-5 5 5M5 20h14"/>"#),
    ("globe", r#"<circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3a14 14 0 0 1 0 18M12 3a14 14 0 0 0 0 18"/>"#),
    ("list", r#"<path d="M8 6h12M8 12h12M8 18h12M4 6h.01M4 12h.01M4 18h.01"/>"#),
    ("grid", r#"<rect x="4" y="4" width="7" height="7" rx="1"/><rect x="13" y="4" width="7" height="7" rx="1"/><rect x="4" y="13" width="7" height="7" rx="1"/><rect x="13" y="13" width="7" height="7" rx="1"/>"#),
    ("video", r#"<rect x="3" y="5" width="18" height="14" rx="2"/><path d="M10 9.5v5l4.5-2.5z"/>"#),
    ("audio", r#"<path d="M9 18V6l10-2v12"/><circle cx="6.5" cy="18" r="2.5"/><circle cx="16.5" cy="16" r="2.5"/>"#),
    ("archive", r#"<rect x="4" y="3" width="16" height="18" rx="2"/><path d="M12 3v8M10 7h4M10 11h4"/>"#),
    ("doc", r#"<path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z"/><path d="M14 3v5h5M9 13h6M9 17h6"/>"#),
    ("image", r#"<rect x="3" y="4" width="18" height="16" rx="2"/><circle cx="9" cy="10" r="1.7"/><path d="M21 16l-5-5-8 8"/>"#),
    ("app", r#"<rect x="4" y="4" width="16" height="16" rx="3"/><path d="M8 9h8M8 13h5"/>"#),
    ("file", r#"<path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z"/><path d="M14 3v5h5"/>"#),
    ("torrent", r#"<path d="M12 3v11M7 10l5 5 5-5"/><path d="M5 19h14"/><circle cx="12" cy="19" r="0.6"/>"#),
    ("star", r#"<path d="M12 3l2.6 5.6 6.1.7-4.5 4.2 1.2 6L12 16.6 6.6 19.5l1.2-6L3.3 9.3l6.1-.7z"/>"#),
    ("moon", r#"<path d="M20 14.5A8 8 0 0 1 9.5 4 8 8 0 1 0 20 14.5z"/>"#),
    ("power", r#"<path d="M12 3v9M6.3 6.8a8 8 0 1 0 11.4 0"/>"#),
    ("home", r#"<path d="M4 11l8-7 8 7v8a1 1 0 0 1-1 1h-4v-6H9v6H5a1 1 0 0 1-1-1z"/>"#),
    ("export", r#"<path d="M12 15V4M8 8l4-4 4 4M5 14v5a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1v-5"/>"#),
    ("import", r#"<path d="M12 4v11M8 11l4 4 4-4M5 14v5a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1v-5"/>"#),
    ("tag", r#"<path d="M3 12V4h8l10 10-8 8z"/><circle cx="7.5" cy="8.5" r="1"/>"#),
    ("help", r#"<circle cx="12" cy="12" r="9"/><path d="M9.5 9.5a2.5 2.5 0 1 1 3.6 2.2c-.7.4-1.1.9-1.1 1.8M12 17h.01"/>"#),
    ("bulb", r#"<path d="M9 18h6M10 21h4M12 3a6 6 0 0 0-3.5 10.9c.6.5 1 1.2 1 2.1h5c0-.9.4-1.6 1-2.1A6 6 0 0 0 12 3z"/>"#),
    ("warn", r#"<path d="M12 3l10 18H2z"/><path d="M12 10v5M12 18h.01"/>"#),
    ("dots", r#"<path d="M5 12h.01M12 12h.01M19 12h.01"/>"#),
    ("sort", r#"<path d="M7 4v16M3 8l4-4 4 4M17 20V4M13 16l4 4 4-4"/>"#),
    ("select", r#"<rect x="4" y="4" width="16" height="16" rx="2"/><path d="M8 12l3 3 5-6"/>"#),
    ("wifi", r#"<path d="M2 9a15 15 0 0 1 20 0M5 12.5a10 10 0 0 1 14 0M8.5 16a5 5 0 0 1 7 0M12 19.5h.01"/>"#),
    ("shield", r#"<path d="M12 3l8 3v6c0 5-3.5 8-8 9-4.5-1-8-4-8-9V6z"/>"#),
    ("lock", r#"<rect x="5" y="11" width="14" height="9" rx="2"/><path d="M8 11V8a4 4 0 0 1 8 0v3"/>"#),
    ("palette", r#"<path d="M12 3a9 9 0 1 0 0 18c1.2 0 1.8-.9 1.5-1.8-.4-1.2.5-2.2 1.7-2.2H18a3 3 0 0 0 3-3A9 9 0 0 0 12 3z"/><circle cx="7.5" cy="11" r=".8"/><circle cx="10" cy="7.5" r=".8"/><circle cx="15" cy="7.5" r=".8"/>"#),
    ("volume", r#"<path d="M4 9v6h4l5 4V5L8 9zM16.5 8.5a5 5 0 0 1 0 7M19 6a8.5 8.5 0 0 1 0 12"/>"#),
    ("keyboard", r#"<rect x="2" y="6" width="20" height="12" rx="2"/><path d="M6 10h.01M10 10h.01M14 10h.01M18 10h.01M7 14h10"/>"#),
    ("calendar", r#"<rect x="4" y="5" width="16" height="15" rx="2"/><path d="M4 10h16M9 3v4M15 3v4"/>"#),
    ("rss", r#"<path d="M5 5a14 14 0 0 1 14 14M5 11a8 8 0 0 1 8 8M6 19h.01"/>"#),
    ("puzzle", r#"<path d="M10 4a2 2 0 1 1 4 0v2h4a1 1 0 0 1 1 1v4h-2a2 2 0 1 0 0 4h2v4a1 1 0 0 1-1 1h-4v-2a2 2 0 1 0-4 0v2H6a1 1 0 0 1-1-1v-4h2a2 2 0 1 0 0-4H5V7a1 1 0 0 1 1-1h4z"/>"#),
    ("bolt", r#"<path d="M13 3L5 14h6l-1 7 8-11h-6z"/>"#),
    ("spider", r#"<circle cx="12" cy="12" r="3"/><path d="M12 3v6M12 15v6M3 12h6M15 12h6M5.6 5.6l3.5 3.5M14.9 14.9l3.5 3.5M18.4 5.6l-3.5 3.5M9.1 14.9l-3.5 3.5"/>"#),
    ("minus", r#"<path d="M5 12h14"/>"#),
    ("min", r#"<path d="M6 18h12"/>"#),
];

/// The SVG sprite every page carries once.
pub fn sprite() -> Markup {
    let mut s = String::from(r#"<svg xmlns="http://www.w3.org/2000/svg" style="display:none" aria-hidden="true">"#);
    for (name, body) in ICONS {
        s.push_str(&format!(
            r#"<symbol id="i-{name}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">{body}</symbol>"#
        ));
    }
    s.push_str("</svg>");
    PreEscaped(s)
}

pub fn icon(name: &str) -> Markup {
    html! { svg class="ic" aria-hidden="true" { use href=(format!("#i-{name}")) {} } }
}

/// A window of its own, shown above the page. `onclose` is asked (POST) when the
/// person closes it with the cross or Escape.
pub fn dialog(id: &str, title: &str, class: &str, onclose: Option<&str>, body: Markup) -> Markup {
    html! {
        dialog id=(id) class={"dlg " (class)} data-modal data-onclose=[onclose] data-title=(title) aria-labelledby={(id) "-t"} {
            div class="dlg-head" {
                span class="dlg-title" id={(id) "-t"} { (title) }
                button type="button" class="dlg-x" data-close title=(t("Close")) aria-label=(t("Close")) { (icon("close")) }
            }
            (body)
        }
    }
}

/// The row of buttons at the bottom of a window.
pub fn footer(buttons: Markup) -> Markup {
    html! { div class="dlg-foot" { (buttons) } }
}

pub fn btn_close(label: &str) -> Markup {
    html! { button type="button" class="btn" data-close { (label) } }
}

pub fn field(label: &str, input: Markup) -> Markup {
    html! { label class="fld" { span class="fld-l" { (label) } (input) } }
}

pub fn text(name: &str, value: &str) -> Markup {
    html! { input class="in" type="text" name=(name) value=(value) autocomplete="off" spellcheck="false"; }
}

pub fn password(name: &str, value: &str) -> Markup {
    html! { input class="in" type="password" name=(name) value=(value) autocomplete="off"; }
}

pub fn number(name: &str, value: impl std::fmt::Display, min: i64, max: i64) -> Markup {
    html! { input class="in num" type="number" name=(name) value=(value.to_string()) min=(min) max=(max) step="1"; }
}

pub fn check(name: &str, label: &str, on: bool) -> Markup {
    html! {
        label class="chk" {
            input type="checkbox" name=(name) value="1" checked[on];
            span class="box" { (icon("check")) }
            span class="chk-l" { (label) }
        }
    }
}

pub fn select(name: &str, options: &[(String, String)], current: &str) -> Markup {
    html! {
        select class="in sel" name=(name) {
            @for (v, l) in options { option value=(v) selected[v == current] { (l) } }
        }
    }
}

pub fn opts(list: &[&str]) -> Vec<(String, String)> {
    list.iter().enumerate().map(|(i, l)| (i.to_string(), t(l))).collect()
}

/// A text box with a "Browse…" button that opens the system's folder or file picker.
pub fn path(name: &str, value: &str, kind: &str) -> Markup {
    let id = format!("p-{name}");
    html! {
        div class="pathbox" id=(id) {
            input class="in" type="text" name=(name) value=(value) autocomplete="off" spellcheck="false";
            button type="button" class="btn" title=(t("Browse…"))
                hx-post=(format!("/pick/{kind}")) hx-include=(format!("#{id} input")) hx-target=(format!("#{id} input"))
                hx-swap="outerHTML" hx-vals=(format!(r#"{{"name":"{name}"}}"#)) { (icon("folder")) }
        }
    }
}

pub fn hint(text: &str) -> Markup {
    html! { p class="hint" { (text) } }
}

pub fn section(title: &str, body: Markup) -> Markup {
    html! { fieldset class="grp" { legend { (title) } (body) } }
}

pub fn primary(label: &str) -> Markup {
    html! { button type="submit" class="btn primary" { (label) } }
}

pub fn msg_ok(text: &str) -> Markup {
    html! { div class="note ok" { (icon("check")) span { (text) } } }
}

pub fn msg_err(text: &str) -> Markup {
    html! { div class="note err" { (icon("warn")) span { (text) } } }
}
