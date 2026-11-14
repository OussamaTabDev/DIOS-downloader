//! "Add DIOS Downloader to my browser": opens the browser on the right page
//! (its store page once the extension is published there, otherwise its
//! extensions page with the folder path ready in the clipboard).
//!
//! The browsers are *found*, not listed: on Linux every program registered as a
//! web browser (.desktop files, the usual folders, Snap, Flatpak, /opt) is
//! offered, on Windows the registry (Start menu browsers, App Paths) and the
//! usual folders are searched. Any browser the program does not know by name
//! is treated as a Chromium-style one (Load unpacked) unless it is Firefox-based.

use crate::i18n::{t, tf};
use crate::open_window;
use crate::*;
use std::collections::HashSet;

/// Store pages of the published extension. Empty = not published yet:
/// then the extension is added from its folder (developer mode).
const STORE_CHROME: &str = "";
const STORE_EDGE: &str = "";
const STORE_FIREFOX: &str = "";
const STORE_OPERA: &str = "";
const STORE_BRAVE: &str = "";

/// The Firefox add-on signed by Mozilla (built by the release): installed
/// from this file it stays in Firefox for good.
const FIREFOX_XPI: &str = "dios-downloader-firefox.xpi";

#[derive(Clone)]
pub(crate) struct Browser {
    name: String,
    /// The program and the arguments that come before the address to open
    /// (`flatpak run org.mozilla.firefox`, ...). Empty for the "Other browser" card.
    cmd: Vec<String>,
    extensions_page: String,
    store: &'static str,
    firefox: bool,
    /// The "Other browser" card: no program, just the folder and the steps.
    generic: bool,
    tint: slint::Color,
}

/// What the program knows about a browser by name.
#[allow(dead_code)]
struct Known {
    name: &'static str,
    /// Lower-case pieces of a program / desktop file / Flatpak name that identify it.
    keys: &'static [&'static str],
    firefox: bool,
    extensions_page: &'static str,
    store: &'static str,
    tint: (u8, u8, u8),
    /// Programs looked for in the folders on PATH (Linux).
    bins: &'static [&'static str],
    /// Flatpak application ids (Linux).
    flatpak: &'static [&'static str],
    /// Fixed places (Linux).
    paths: &'static [&'static str],
    /// (environment variable, path below it) (Windows).
    win: &'static [(&'static str, &'static str)],
    /// App Paths entry (Windows).
    win_exe: &'static str,
}

const GECKO_PAGE: &str = "about:debugging#/runtime/this-firefox";

// The order matters twice: it is the order of the cards, and the first entry
// whose `keys` appear in a program's name wins (so the forks come before the
// browsers they are made from).
const KNOWN: &[Known] = &[
    Known {
        name: "LibreWolf",
        keys: &["librewolf"],
        firefox: true,
        extensions_page: GECKO_PAGE,
        store: "",
        tint: (0x00, 0x9e, 0xe6),
        bins: &["librewolf"],
        flatpak: &["io.gitlab.librewolf-community"],
        paths: &["/opt/librewolf/librewolf"],
        win: &[("ProgramFiles", r"LibreWolf\librewolf.exe"), ("LOCALAPPDATA", r"LibreWolf\librewolf.exe")],
        win_exe: "librewolf.exe",
    },
    Known {
        name: "Waterfox",
        keys: &["waterfox"],
        firefox: true,
        extensions_page: GECKO_PAGE,
        store: "",
        tint: (0x1a, 0xa7, 0xec),
        bins: &["waterfox", "waterfox-g", "waterfox-bin"],
        flatpak: &["net.waterfox.waterfox"],
        paths: &["/opt/waterfox/waterfox"],
        win: &[("ProgramFiles", r"Waterfox\waterfox.exe")],
        win_exe: "waterfox.exe",
    },
    Known {
        name: "Zen",
        keys: &["zen-browser", "zen_browser", "app.zen"],
        firefox: true,
        extensions_page: GECKO_PAGE,
        store: "",
        tint: (0xf2, 0x76, 0x4a),
        bins: &["zen-browser", "zen"],
        flatpak: &["app.zen_browser.zen"],
        paths: &["/opt/zen/zen", "/opt/zen-browser-bin/zen-bin"],
        win: &[("ProgramFiles", r"Zen Browser\zen.exe"), ("LOCALAPPDATA", r"Zen Browser\zen.exe")],
        win_exe: "zen.exe",
    },
    Known {
        name: "Floorp",
        keys: &["floorp"],
        firefox: true,
        extensions_page: GECKO_PAGE,
        store: "",
        tint: (0x3a, 0x7b, 0xf2),
        bins: &["floorp"],
        flatpak: &["one.ablaze.floorp"],
        paths: &[],
        win: &[("ProgramFiles", r"Ablaze Floorp\floorp.exe")],
        win_exe: "floorp.exe",
    },
    Known {
        name: "Mullvad Browser",
        keys: &["mullvad"],
        firefox: true,
        extensions_page: GECKO_PAGE,
        store: "",
        tint: (0xf0, 0xc0, 0x40),
        bins: &["mullvad-browser"],
        flatpak: &[],
        paths: &["/opt/mullvad-browser/mullvadbrowser"],
        win: &[],
        win_exe: "",
    },
    Known {
        name: "IceCat",
        keys: &["icecat"],
        firefox: true,
        extensions_page: GECKO_PAGE,
        store: "",
        tint: (0x5b, 0x8d, 0xd6),
        bins: &["icecat"],
        flatpak: &[],
        paths: &[],
        win: &[],
        win_exe: "",
    },
    Known {
        name: "Firefox",
        keys: &["firefox"],
        firefox: true,
        extensions_page: GECKO_PAGE,
        store: STORE_FIREFOX,
        tint: (0xe8, 0x6a, 0x10),
        bins: &["firefox", "firefox-esr", "firefox-developer-edition", "firefox-beta", "firefox-nightly", "firefox-bin"],
        flatpak: &["org.mozilla.firefox"],
        paths: &["/usr/lib/firefox/firefox", "/usr/lib64/firefox/firefox", "/opt/firefox/firefox", "/snap/bin/firefox"],
        win: &[
            ("ProgramFiles", r"Mozilla Firefox\firefox.exe"),
            ("ProgramFiles(x86)", r"Mozilla Firefox\firefox.exe"),
            ("LOCALAPPDATA", r"Mozilla Firefox\firefox.exe"),
        ],
        win_exe: "firefox.exe",
    },
    Known {
        name: "Brave",
        keys: &["brave"],
        firefox: false,
        extensions_page: "brave://extensions/",
        store: STORE_BRAVE,
        tint: (0xfb, 0x54, 0x2b),
        bins: &["brave-browser", "brave", "brave-browser-stable", "brave-browser-beta", "brave-origin"],
        flatpak: &["com.brave.Browser"],
        paths: &["/opt/brave.com/brave/brave-browser", "/opt/brave-bin/brave", "/snap/bin/brave"],
        win: &[
            ("ProgramFiles", r"BraveSoftware\Brave-Browser\Application\brave.exe"),
            ("ProgramFiles(x86)", r"BraveSoftware\Brave-Browser\Application\brave.exe"),
            ("LOCALAPPDATA", r"BraveSoftware\Brave-Browser\Application\brave.exe"),
        ],
        win_exe: "brave.exe",
    },
    Known {
        name: "Edge",
        keys: &["microsoft-edge", "msedge", "com.microsoft.edge"],
        firefox: false,
        extensions_page: "edge://extensions/",
        store: STORE_EDGE,
        tint: (0x0c, 0x8c, 0xd8),
        bins: &["microsoft-edge", "microsoft-edge-stable", "microsoft-edge-beta", "microsoft-edge-dev", "msedge"],
        flatpak: &["com.microsoft.Edge"],
        paths: &["/opt/microsoft/msedge/microsoft-edge", "/opt/microsoft/msedge/msedge"],
        win: &[
            ("ProgramFiles(x86)", r"Microsoft\Edge\Application\msedge.exe"),
            ("ProgramFiles", r"Microsoft\Edge\Application\msedge.exe"),
        ],
        win_exe: "msedge.exe",
    },
    Known {
        name: "Vivaldi",
        keys: &["vivaldi"],
        firefox: false,
        extensions_page: "vivaldi://extensions",
        store: "",
        tint: (0xef, 0x39, 0x39),
        bins: &["vivaldi", "vivaldi-stable", "vivaldi-snapshot"],
        flatpak: &["com.vivaldi.Vivaldi"],
        paths: &["/opt/vivaldi/vivaldi", "/opt/vivaldi/vivaldi-bin"],
        win: &[("LOCALAPPDATA", r"Vivaldi\Application\vivaldi.exe"), ("ProgramFiles", r"Vivaldi\Application\vivaldi.exe")],
        win_exe: "vivaldi.exe",
    },
    Known {
        name: "Opera",
        keys: &["opera"],
        firefox: false,
        extensions_page: "opera://extensions",
        store: STORE_OPERA,
        tint: (0xd6, 0x1f, 0x2b),
        bins: &["opera", "opera-stable", "opera-beta", "opera-developer"],
        flatpak: &["com.opera.Opera"],
        paths: &["/usr/lib/x86_64-linux-gnu/opera/opera", "/snap/bin/opera"],
        win: &[
            ("LOCALAPPDATA", r"Programs\Opera\opera.exe"),
            ("LOCALAPPDATA", r"Programs\Opera GX\opera.exe"),
            ("ProgramFiles", r"Opera\opera.exe"),
        ],
        win_exe: "opera.exe",
    },
    Known {
        name: "Yandex Browser",
        keys: &["yandex"],
        firefox: false,
        extensions_page: "browser://extensions",
        store: "",
        tint: (0xfc, 0x3f, 0x1d),
        bins: &["yandex-browser", "yandex-browser-stable", "yandex-browser-beta"],
        flatpak: &[],
        paths: &["/opt/yandex/browser/yandex-browser"],
        win: &[("LOCALAPPDATA", r"Yandex\YandexBrowser\Application\browser.exe")],
        win_exe: "browser.exe",
    },
    Known {
        name: "Thorium",
        keys: &["thorium"],
        firefox: false,
        extensions_page: "chrome://extensions/",
        store: "",
        tint: (0x6c, 0x4d, 0xd9),
        bins: &["thorium-browser", "thorium"],
        flatpak: &[],
        paths: &["/opt/chromium.org/thorium/thorium-browser"],
        win: &[("LOCALAPPDATA", r"Thorium\Application\thorium.exe")],
        win_exe: "thorium.exe",
    },
    Known {
        name: "Chromium",
        keys: &["chromium", "ungoogled"],
        firefox: false,
        extensions_page: "chrome://extensions/",
        store: "",
        tint: (0x42, 0x85, 0xf4),
        bins: &["chromium", "chromium-browser", "ungoogled-chromium"],
        flatpak: &["org.chromium.Chromium", "io.github.ungoogled_software.ungoogled_chromium"],
        paths: &["/snap/bin/chromium", "/usr/lib/chromium/chromium", "/usr/lib/chromium-browser/chromium-browser"],
        win: &[("LOCALAPPDATA", r"Chromium\Application\chrome.exe")],
        win_exe: "",
    },
    Known {
        name: "Chrome",
        keys: &["google-chrome", "com.google.chrome", "chrome"],
        firefox: false,
        extensions_page: "chrome://extensions/",
        store: STORE_CHROME,
        tint: (0x1f, 0xa4, 0x63),
        bins: &["google-chrome", "google-chrome-stable", "google-chrome-beta", "google-chrome-unstable", "chrome"],
        flatpak: &["com.google.Chrome"],
        paths: &["/opt/google/chrome/chrome", "/opt/google/chrome/google-chrome"],
        win: &[
            ("ProgramFiles", r"Google\Chrome\Application\chrome.exe"),
            ("ProgramFiles(x86)", r"Google\Chrome\Application\chrome.exe"),
            ("LOCALAPPDATA", r"Google\Chrome\Application\chrome.exe"),
        ],
        win_exe: "chrome.exe",
    },
];

/// Browsers that have no Chrome / Firefox style extension support: not offered.
const NO_EXTENSIONS: &[&str] = &[
    "epiphany", "org.gnome.web", "falkon", "konqueror", "qutebrowser", "midori", "luakit", "nyxt", "surf", "dillo",
    "netsurf", "lynx", "w3m", "links", "elinks", "tor-browser-launcher", "torbrowser-launcher", "browsh", "badwolf",
    "internet explorer", "iexplore", "palemoon", "pale moon", "basilisk",
];

fn brand_of(text: &str) -> Option<usize> {
    let text = text.to_lowercase();
    KNOWN.iter().position(|k| k.keys.iter().any(|key| text.contains(key)))
}

fn is_unsupported(text: &str) -> bool {
    let text = text.to_lowercase();
    NO_EXTENSIONS.iter().any(|k| text.contains(k))
}

fn looks_like_firefox(text: &str) -> bool {
    let text = text.to_lowercase();
    ["firefox", "gecko", "librewolf", "waterfox", "icecat", "floorp"].iter().any(|k| text.contains(k))
}

/// A steady colour for a browser the program does not know.
fn tint_for(name: &str) -> slint::Color {
    let mut h: u32 = 2166136261;
    for b in name.bytes() {
        h = (h ^ b as u32).wrapping_mul(16777619);
    }
    let hue = (h % 360) as f32;
    // HSL (s = 0.6, l = 0.45) to RGB
    let (s, l) = (0.6f32, 0.45f32);
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((hue / 60.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;
    let (r, g, b) = match (hue / 60.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    slint::Color::from_rgb_u8(((r + m) * 255.0) as u8, ((g + m) * 255.0) as u8, ((b + m) * 255.0) as u8)
}

fn from_known(i: usize, cmd: Vec<String>) -> Browser {
    let k = &KNOWN[i];
    Browser {
        name: k.name.to_string(),
        cmd,
        extensions_page: k.extensions_page.to_string(),
        store: k.store,
        firefox: k.firefox,
        generic: false,
        tint: slint::Color::from_rgb_u8(k.tint.0, k.tint.1, k.tint.2),
    }
}

/// A browser found by its own name only (not one of KNOWN).
fn from_unknown(name: &str, cmd: Vec<String>, hint: &str) -> Browser {
    let firefox = looks_like_firefox(hint) || looks_like_firefox(name);
    Browser {
        name: name.to_string(),
        cmd,
        extensions_page: if firefox { GECKO_PAGE.into() } else { "chrome://extensions/".into() },
        store: "",
        firefox,
        generic: false,
        tint: tint_for(name),
    }
}

/// The last card: any browser, added by hand.
fn other_browser() -> Browser {
    Browser {
        name: t("Other browser"),
        cmd: Vec::new(),
        extensions_page: String::new(),
        store: "",
        firefox: false,
        generic: true,
        tint: slint::Color::from_rgb_u8(0x6b, 0x7c, 0x90),
    }
}

// ------------------------------------------------------------------ finding them

#[cfg(windows)]
fn detect_all() -> Vec<Browser> {
    let mut found: Vec<Option<Browser>> = (0..KNOWN.len()).map(|_| None).collect();
    let mut others: Vec<Browser> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    // 1. the usual folders and the App Paths of every known browser
    for (i, k) in KNOWN.iter().enumerate() {
        let mut exe: Option<PathBuf> = None;
        for (var, rest) in k.win {
            if let Some(p) = std::env::var_os(var).map(|v| PathBuf::from(v).join(rest)) {
                if p.is_file() {
                    exe = Some(p);
                    break;
                }
            }
        }
        if exe.is_none() && !k.win_exe.is_empty() {
            exe = winreg::app_path(k.win_exe);
        }
        if let Some(exe) = exe {
            seen.insert(exe.to_string_lossy().to_lowercase());
            found[i] = Some(from_known(i, vec![exe.display().to_string()]));
        }
    }
    // 2. every browser the system lists under "Start menu internet" (default programs)
    for (name, exe) in winreg::start_menu_browsers() {
        let key = exe.to_string_lossy().to_lowercase();
        if seen.contains(&key) || is_unsupported(&name) || is_unsupported(&key) {
            continue;
        }
        seen.insert(key.clone());
        let cmd = vec![exe.display().to_string()];
        match brand_of(&format!("{name} {key}")) {
            Some(i) if found[i].is_none() => found[i] = Some(from_known(i, cmd)),
            Some(_) => {}
            None => others.push(from_unknown(&name, cmd, &key)),
        }
    }
    finish(found, others)
}

#[cfg(not(windows))]
fn detect_all() -> Vec<Browser> {
    let dirs = search_dirs();
    let mut found: Vec<Option<Browser>> = (0..KNOWN.len()).map(|_| None).collect();
    let mut others: Vec<Browser> = Vec::new();

    // 1. known browsers by their program names, Flatpak ids and fixed places
    for (i, k) in KNOWN.iter().enumerate() {
        let mut cmd: Option<Vec<String>> = None;
        for bin in k.bins {
            if let Some(p) = find_in(&dirs, bin) {
                cmd = Some(vec![p.display().to_string()]);
                break;
            }
        }
        if cmd.is_none() {
            for id in k.flatpak {
                if let Some(p) = find_in(&flatpak_bin_dirs(), id) {
                    cmd = Some(vec![p.display().to_string()]);
                    break;
                }
            }
        }
        if cmd.is_none() {
            for p in k.paths {
                if is_executable(Path::new(p)) {
                    cmd = Some(vec![p.to_string()]);
                    break;
                }
            }
        }
        if let Some(cmd) = cmd {
            found[i] = Some(from_known(i, cmd));
        }
    }

    // 2. every program that registered itself as a web browser (.desktop files)
    for e in desktop_entries(&dirs) {
        let hint = format!("{} {} {}", e.file, e.name, e.cmd.join(" "));
        if is_unsupported(&hint) {
            continue;
        }
        match brand_of(&hint) {
            Some(i) => {
                if found[i].is_none() {
                    found[i] = Some(from_known(i, e.cmd));
                }
            }
            None => {
                let name = clean_name(&e.name);
                if name.is_empty() || others.iter().any(|b| b.name.eq_ignore_ascii_case(&name)) {
                    continue;
                }
                others.push(from_unknown(&name, e.cmd, &hint));
            }
        }
    }
    finish(found, others)
}

/// The cards in their final order: the system's default browser first, then the
/// popular ones, then the rest by name.
fn finish(found: Vec<Option<Browser>>, mut others: Vec<Browser>) -> Vec<Browser> {
    others.sort_by_key(|b| b.name.to_lowercase());
    let mut list: Vec<Browser> = found.into_iter().flatten().collect();
    // the table lists forks before their parents; show the popular ones first
    const POPULAR: [&str; 5] = ["Chrome", "Edge", "Firefox", "Brave", "Opera"];
    list.sort_by_key(|b| POPULAR.iter().position(|p| *p == b.name).unwrap_or(POPULAR.len()));
    list.extend(others);
    #[cfg(not(windows))]
    if let Some(default) = default_browser_hint() {
        let key = default.to_lowercase();
        let key = key.trim_end_matches(".desktop").to_string();
        let brand = brand_of(&key).map(|i| KNOWN[i].name);
        if let Some(pos) = list.iter().position(|b| {
            let hay = format!("{} {}", b.name, b.cmd.join(" ")).to_lowercase();
            hay.contains(&key) || brand == Some(b.name.as_str())
        }) {
            let b = list.remove(pos);
            list.insert(0, b);
        }
    }
    list.truncate(11);
    list
}

// ---- Linux / BSD ----

#[cfg(not(windows))]
struct DesktopEntry {
    /// file name, e.g. "firefox.desktop"
    file: String,
    name: String,
    /// program and its arguments, field codes removed
    cmd: Vec<String>,
}

#[cfg(not(windows))]
fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from).filter(|p| p.is_absolute())
}

#[cfg(not(windows))]
fn is_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    p.metadata().map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0).unwrap_or(false)
}

/// Folders programs live in: PATH first, then the places a desktop session
/// may not have on its PATH (Snap, Flatpak exports, ~/.local/bin).
#[cfg(not(windows))]
fn search_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    for d in ["/usr/local/bin", "/usr/bin", "/bin", "/snap/bin", "/var/lib/flatpak/exports/bin"] {
        dirs.push(PathBuf::from(d));
    }
    if let Some(h) = home() {
        dirs.push(h.join(".local/bin"));
        dirs.push(h.join(".local/share/flatpak/exports/bin"));
        dirs.push(h.join("bin"));
    }
    let mut seen = HashSet::new();
    dirs.retain(|d| d.is_absolute() && seen.insert(d.clone()));
    dirs
}

#[cfg(not(windows))]
fn flatpak_bin_dirs() -> Vec<PathBuf> {
    let mut v = vec![PathBuf::from("/var/lib/flatpak/exports/bin")];
    if let Some(h) = home() {
        v.push(h.join(".local/share/flatpak/exports/bin"));
    }
    v
}

#[cfg(not(windows))]
fn find_in(dirs: &[PathBuf], name: &str) -> Option<PathBuf> {
    dirs.iter().map(|d| d.join(name)).find(|p| is_executable(p))
}

/// Where the .desktop files are (user first, then the system ones).
#[cfg(not(windows))]
fn application_dirs() -> Vec<PathBuf> {
    let mut v = Vec::new();
    let data_home = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| home().map(|h| h.join(".local/share")));
    if let Some(d) = data_home {
        v.push(d.join("applications"));
    }
    let data_dirs = std::env::var("XDG_DATA_DIRS").unwrap_or_default();
    let data_dirs = if data_dirs.trim().is_empty() { "/usr/local/share:/usr/share".to_string() } else { data_dirs };
    for d in data_dirs.split(':').filter(|d| !d.is_empty()) {
        v.push(PathBuf::from(d).join("applications"));
    }
    v.push(PathBuf::from("/var/lib/flatpak/exports/share/applications"));
    v.push(PathBuf::from("/var/lib/snapd/desktop/applications"));
    if let Some(h) = home() {
        v.push(h.join(".local/share/flatpak/exports/share/applications"));
    }
    let mut seen = HashSet::new();
    v.retain(|d| seen.insert(d.clone()));
    v
}

/// Split an `Exec=` line the way the desktop entry rules say (quotes, backslashes).
#[cfg(not(windows))]
fn split_exec(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut has = false;
    let mut quote: Option<char> = None;
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), '\\') => {
                if let Some(n) = chars.next() {
                    cur.push(n);
                }
            }
            (Some(_), c) => cur.push(c),
            (None, '"') | (None, '\'') => {
                quote = Some(c);
                has = true;
            }
            (None, '\\') => {
                if let Some(n) = chars.next() {
                    cur.push(n);
                    has = true;
                }
            }
            (None, c) if c.is_whitespace() => {
                if has || !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                    has = false;
                }
            }
            (None, c) => {
                cur.push(c);
                has = true;
            }
        }
    }
    if has || !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// The program + arguments of an `Exec=` line, without field codes (%u, %F, ...),
/// Flatpak's file markers (@@u) and a leading `env VAR=value`. None when the
/// program is not there.
#[cfg(not(windows))]
fn exec_command(exec: &str, dirs: &[PathBuf]) -> Option<Vec<String>> {
    let mut parts: Vec<String> = split_exec(exec)
        .into_iter()
        .filter(|a| !(a.len() == 2 && a.starts_with('%')) && !a.starts_with("@@"))
        .collect();
    if parts.first().is_some_and(|p| p == "env" || p.ends_with("/env")) {
        parts.remove(0);
        while parts.first().is_some_and(|p| p.contains('=') && !p.starts_with('-') && !p.starts_with('/')) {
            parts.remove(0);
        }
    }
    let prog = parts.first()?.clone();
    let resolved = if prog.contains('/') {
        PathBuf::from(&prog).is_file().then(|| PathBuf::from(&prog))
    } else {
        find_in(dirs, &prog)
    }?;
    parts[0] = resolved.display().to_string();
    Some(parts)
}

/// The web browsers among the .desktop files.
#[cfg(not(windows))]
fn desktop_entries(dirs: &[PathBuf]) -> Vec<DesktopEntry> {
    let mut out = Vec::new();
    let mut seen_files = HashSet::new();
    for dir in application_dirs() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let path = e.path();
            let Some(file) = path.file_name().and_then(|f| f.to_str()).map(str::to_string) else { continue };
            if !file.ends_with(".desktop") || !seen_files.insert(file.clone()) {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else { continue };
            if !(text.contains("WebBrowser") || text.contains("x-scheme-handler/http")) {
                continue;
            }
            if let Some(entry) = parse_desktop(&file, &text, dirs) {
                out.push(entry);
            }
        }
    }
    out
}

#[cfg(not(windows))]
fn parse_desktop(file: &str, text: &str, dirs: &[PathBuf]) -> Option<DesktopEntry> {
    let (mut name, mut exec) = (String::new(), String::new());
    let (mut web, mut hidden, mut terminal) = (false, false, false);
    let mut in_main = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            // only the main group: not the "new window" actions below it
            in_main = line == "[Desktop Entry]";
            continue;
        }
        if !in_main {
            continue;
        }
        if let Some(v) = line.strip_prefix("Name=") {
            name = v.to_string();
        } else if let Some(v) = line.strip_prefix("Exec=") {
            exec = v.to_string();
        } else if let Some(v) = line.strip_prefix("Categories=") {
            web |= v.split(';').any(|c| c == "WebBrowser");
        } else if let Some(v) = line.strip_prefix("MimeType=") {
            web |= v.split(';').any(|c| c == "x-scheme-handler/http" || c == "x-scheme-handler/https");
        } else if line == "NoDisplay=true" || line == "Hidden=true" {
            hidden = true;
        } else if line == "Terminal=true" {
            terminal = true;
        }
    }
    if !web || hidden || terminal || exec.is_empty() {
        return None;
    }
    let cmd = exec_command(&exec, dirs)?;
    Some(DesktopEntry { file: file.to_string(), name, cmd })
}

/// "Brave Web Browser" -> "Brave", "Vivaldi (snapshot)" -> "Vivaldi".
#[cfg(not(windows))]
fn clean_name(name: &str) -> String {
    let mut n = name.split(" (").next().unwrap_or(name).trim().to_string();
    for suffix in [" Web Browser", " Browser", " web browser"] {
        if n.len() > suffix.len() && n.ends_with(suffix) {
            n.truncate(n.len() - suffix.len());
        }
    }
    n.trim().to_string()
}

/// The desktop file of the system's default browser, from mimeapps.list.
#[cfg(not(windows))]
fn default_browser_hint() -> Option<String> {
    let mut files: Vec<PathBuf> = Vec::new();
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| home().map(|h| h.join(".config")));
    if let Some(c) = config {
        files.push(c.join("mimeapps.list"));
    }
    if let Some(h) = home() {
        files.push(h.join(".local/share/applications/mimeapps.list"));
    }
    for f in files {
        let Ok(text) = std::fs::read_to_string(f) else { continue };
        let mut in_default = false;
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with('[') {
                in_default = line == "[Default Applications]";
            } else if in_default {
                for key in ["x-scheme-handler/https=", "x-scheme-handler/http=", "text/html="] {
                    if let Some(v) = line.strip_prefix(key) {
                        if let Some(first) = v.split(';').find(|s| !s.is_empty()) {
                            return Some(first.to_string());
                        }
                    }
                }
            }
        }
    }
    None
}

// ---- Windows registry ----

#[cfg(windows)]
mod winreg {
    use std::path::PathBuf;
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegEnumKeyExW, RegGetValueW, RegOpenKeyExW, HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ,
        RRF_RT_REG_EXPAND_SZ, RRF_RT_REG_SZ,
    };

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// The text of a value (`name` = "" is the key's default value).
    fn read_string(root: HKEY, sub: &str, name: &str) -> Option<String> {
        let sub = wide(sub);
        let name_w = wide(name);
        let mut size: u32 = 0;
        // SAFETY: NUL-terminated wide strings; the buffer is sized by the first call
        unsafe {
            let name_ptr = if name.is_empty() { std::ptr::null() } else { name_w.as_ptr() };
            let flags = RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ;
            if RegGetValueW(root, sub.as_ptr(), name_ptr, flags, std::ptr::null_mut(), std::ptr::null_mut(), &mut size) != 0
                || size < 2
            {
                return None;
            }
            let mut buf = vec![0u16; (size as usize).div_ceil(2)];
            if RegGetValueW(root, sub.as_ptr(), name_ptr, flags, std::ptr::null_mut(), buf.as_mut_ptr().cast(), &mut size) != 0 {
                return None;
            }
            let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
            Some(String::from_utf16_lossy(&buf[..len]))
        }
    }

    /// `"C:\path\prog.exe" --args` -> the program's path.
    fn exe_of_command(cmd: &str) -> Option<PathBuf> {
        let cmd = cmd.trim();
        let path = if let Some(rest) = cmd.strip_prefix('"') {
            rest.split('"').next()?.to_string()
        } else {
            match cmd.to_lowercase().find(".exe") {
                Some(i) => cmd[..i + 4].to_string(),
                None => cmd.to_string(),
            }
        };
        // environment variables such as %ProgramFiles%
        let mut out = path;
        while let (Some(a), Some(b)) = (out.find('%'), out.rfind('%')) {
            if a == b {
                break;
            }
            let var = out[a + 1..b].to_string();
            let value = std::env::var(&var).unwrap_or_default();
            out.replace_range(a..=b, &value);
        }
        let p = PathBuf::from(out);
        p.is_file().then_some(p)
    }

    /// The program registered as `exe_name` under "App Paths".
    pub fn app_path(exe_name: &str) -> Option<PathBuf> {
        let sub = format!(r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\{exe_name}");
        for root in [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER] {
            if let Some(v) = read_string(root, &sub, "") {
                if let Some(p) = exe_of_command(&v) {
                    return Some(p);
                }
            }
        }
        None
    }

    fn subkeys(root: HKEY, sub: &str) -> Vec<String> {
        let sub = wide(sub);
        let mut out = Vec::new();
        // SAFETY: the key is closed again; the name buffer is large enough for any key name
        unsafe {
            let mut key: HKEY = std::ptr::null_mut();
            if RegOpenKeyExW(root, sub.as_ptr(), 0, KEY_READ, &mut key) != 0 {
                return out;
            }
            for i in 0..256u32 {
                let mut name = [0u16; 256];
                let mut len = name.len() as u32;
                let r = RegEnumKeyExW(
                    key,
                    i,
                    name.as_mut_ptr(),
                    &mut len,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                );
                if r != 0 {
                    break;
                }
                out.push(String::from_utf16_lossy(&name[..len as usize]));
            }
            RegCloseKey(key);
        }
        out
    }

    /// (name, program) of every browser under "Start menu internet" - the list
    /// Windows itself shows in Default apps.
    pub fn start_menu_browsers() -> Vec<(String, PathBuf)> {
        let base = r"SOFTWARE\Clients\StartMenuInternet";
        let mut out = Vec::new();
        for root in [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER] {
            for key in subkeys(root, base) {
                let path = format!(r"{base}\{key}");
                let Some(cmd) = read_string(root, &format!(r"{path}\shell\open\command"), "") else { continue };
                let Some(exe) = exe_of_command(&cmd) else { continue };
                let name = read_string(root, &path, "").unwrap_or_else(|| key.clone());
                out.push((name, exe));
            }
        }
        out
    }
}

// ------------------------------------------------------------------ the extension's files

/// Folders the extension may be in: next to the program, in the user's data
/// folder (what install.sh fills), and the usual system places.
fn extension_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(exe) = std::env::current_exe().ok().map(|e| e.canonicalize().unwrap_or(e)) {
        if let Some(dir) = exe.parent() {
            roots.push(dir.to_path_buf());
            if let Some(up) = dir.parent() {
                roots.push(up.to_path_buf());
                // `cargo run`: target/release -> the project folder
                if let Some(up2) = up.parent() {
                    roots.push(up2.to_path_buf());
                }
            }
        }
    }
    #[cfg(not(windows))]
    {
        let data_home = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| home().map(|h| h.join(".local/share")));
        if let Some(d) = data_home {
            roots.push(d.join("dios-downloader"));
        }
        for d in [
            "/usr/share/dios-downloader",
            "/usr/local/share/dios-downloader",
            "/usr/lib/dios-downloader",
            "/opt/DIOSDownloader",
            "/opt/dios-downloader",
        ] {
            roots.push(PathBuf::from(d));
        }
        // inside an AppImage
        if let Some(appdir) = std::env::var_os("APPDIR") {
            roots.push(PathBuf::from(appdir).join("usr/share/dios-downloader"));
        }
    }
    roots
}

/// The extension's folder (Firefox has its own copy).
fn extension_dir(firefox: bool) -> Option<PathBuf> {
    let name = if firefox { "extension-firefox" } else { "extension" };
    let candidates: Vec<PathBuf> = extension_roots().into_iter().map(|r| r.join(name)).collect();
    candidates
        .iter()
        .find(|p| p.join("manifest.json").exists())
        .or_else(|| candidates.iter().find(|p| p.is_dir()))
        .cloned()
}

/// The signed Firefox add-on next to the program, when the release has one.
fn firefox_xpi() -> Option<PathBuf> {
    extension_roots().into_iter().map(|r| r.join(FIREFOX_XPI)).find(|p| p.exists())
}

// ------------------------------------------------------------------ the window

pub(crate) fn open(ui: &Ui) {
    let Some(d) = ui.browsers.upgrade() else { return };
    let mut list = detect_all();
    let none_found = list.is_empty();
    list.push(other_browser());
    let rows: Vec<BrowserRow> = list
        .iter()
        .map(|b| BrowserRow {
            name: b.name.clone().into(),
            tint: b.tint,
            installed: true,
            action: t(if b.generic {
                "Any other browser: copy the folder"
            } else if b.store.is_empty() {
                "Click to add"
            } else {
                "Click to add from the store"
            })
            .into(),
        })
        .collect();
    *ui.browser_list.borrow_mut() = list;
    d.set_browsers(ModelRc::new(VecModel::from(rows)));
    d.set_steps(if none_found {
        t("No browser was found on this computer automatically. Click \"Other browser\" and follow the steps.").into()
    } else {
        SharedString::new()
    });
    d.set_folder(extension_dir(false).map(|p| p.display().to_string()).unwrap_or_default().into());
    open_window!(d);
    d.window().set_minimized(false);
}

fn launch(cmd: &[String], url: &str) {
    let Some((prog, args)) = cmd.split_first() else { return };
    let mut c = std::process::Command::new(prog);
    c.args(args).arg(url);
    #[cfg(not(windows))]
    c.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    let _ = c.spawn();
}

fn copy_to_clipboard(text: &str) {
    if let Ok(mut c) = arboard::Clipboard::new() {
        let _ = c.set_text(text.to_string());
    }
}

fn add(ui: &Ui, i: usize) {
    let Some(d) = ui.browsers.upgrade() else { return };
    let Some(b) = ui.browser_list.borrow().get(i).cloned() else { return };

    if b.generic {
        // any browser: the folder path is copied, the steps are the same for all of them
        let Some(dir) = extension_dir(false) else {
            d.set_steps(missing_folder_text().into());
            return;
        };
        let path = dir.display().to_string();
        copy_to_clipboard(&path);
        d.set_folder(path.into());
        d.set_steps(
            t("1. Open your browser's extensions page and turn on \"Developer mode\".\n2. Click \"Load unpacked\" (Firefox: \"Load Temporary Add-on…\").\n3. Paste the folder path (Ctrl+V – it is already copied) and choose it.\n\nFirefox-based browsers use the \"extension-firefox\" folder instead.")
                .into(),
        );
        return;
    }
    if !b.store.is_empty() {
        launch(&b.cmd, b.store);
        d.set_steps(tf("{} opened the store page: click \"Add\" / \"Get\" there.", &[&b.name]).into());
        return;
    }
    if b.firefox {
        if let Some(xpi) = firefox_xpi() {
            // the signed add-on: Firefox asks "Add DIOS Downloader?" and keeps it
            launch(&b.cmd, &xpi.display().to_string());
            d.set_folder(xpi.display().to_string().into());
            d.set_steps(t("Firefox opened the DIOS Downloader add-on.\n1. Click \"Add\" in the message Firefox shows.\n2. Then click \"OK\".\n\nThe add-on is signed by Mozilla, so it stays in Firefox after you close it and updates by itself.").into());
            return;
        }
    }
    let Some(dir) = extension_dir(b.firefox) else {
        d.set_steps(missing_folder_text().into());
        return;
    };
    let path = dir.display().to_string();
    copy_to_clipboard(&path);
    launch(&b.cmd, &b.extensions_page);
    d.set_folder(path.clone().into());
    let steps = if b.firefox {
        t("Firefox opened its add-ons debugging page.\n1. Click \"Load Temporary Add-on…\".\n2. Paste the folder path (Ctrl+V, it is already copied) and open the file \"manifest.json\" in it.\n\nFirefox removes add-ons loaded this way when it closes. To keep it for good the extension must be signed by Mozilla (free) – ask for the signed version.")
    } else {
        tf("{} opened its extensions page.\n1. Turn on \"Developer mode\" (a switch on that page).\n2. Click \"Load unpacked\".\n3. Paste the folder path (Ctrl+V – it is already copied) and click \"Select folder\".\n\nThat's all: the DIOS Downloader icon appears next to the address bar. You do this only once.", &[&b.name])
    };
    d.set_steps(steps.into());
}

fn missing_folder_text() -> String {
    t("The extension folder was not found next to DIOSDownloader.exe. Install DIOS Downloader again.")
}

pub(crate) fn wire(ui: &Ui) {
    let Some(d) = ui.browsers.upgrade() else { return };
    {
        let ui = ui.clone();
        d.on_add(move |i| add(&ui, i.max(0) as usize));
    }
    {
        let w = d.as_weak();
        d.on_copy_folder(move || {
            if let Some(d) = w.upgrade() {
                copy_to_clipboard(&d.get_folder().to_string());
            }
        });
    }
    {
        let w = d.as_weak();
        d.on_open_folder(move || {
            if let Some(d) = w.upgrade() {
                let f = d.get_folder().to_string();
                if !f.is_empty() {
                    // a file (the .xpi): its folder
                    let p = PathBuf::from(&f);
                    let target = if p.is_file() { p.parent().map(|x| x.to_path_buf()).unwrap_or(p) } else { p };
                    let _ = open::that(target);
                }
            }
        });
    }
    {
        let w = d.as_weak();
        d.on_close_window(move || {
            if let Some(d) = w.upgrade() {
                let _ = d.hide();
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brands_are_found_by_any_piece_of_their_name() {
        let name = |s: &str| brand_of(s).map(|i| KNOWN[i].name);
        assert_eq!(name("google-chrome-stable"), Some("Chrome"));
        assert_eq!(name("/usr/bin/chromium-browser"), Some("Chromium"));
        assert_eq!(name("org.mozilla.firefox"), Some("Firefox"));
        assert_eq!(name("io.gitlab.librewolf-community"), Some("LibreWolf"));
        assert_eq!(name("brave-browser.desktop"), Some("Brave"));
        assert_eq!(name("microsoft-edge"), Some("Edge"));
        assert_eq!(name("some-unknown-browser"), None);
    }

    #[test]
    fn browsers_without_extension_support_are_skipped() {
        assert!(is_unsupported("org.gnome.Epiphany.desktop"));
        assert!(is_unsupported("Falkon"));
        assert!(!is_unsupported("google-chrome"));
    }

    #[cfg(not(windows))]
    #[test]
    fn exec_lines_lose_their_field_codes() {
        assert_eq!(split_exec(r#"/usr/bin/firefox %u"#), vec!["/usr/bin/firefox", "%u"]);
        assert_eq!(split_exec(r#""/opt/My Browser/b" --flag "a b""#), vec!["/opt/My Browser/b", "--flag", "a b"]);
        let dirs = vec![PathBuf::from("/bin"), PathBuf::from("/usr/bin")];
        let cmd = exec_command("env FOO=1 sh -c %U", &dirs).unwrap();
        assert!(cmd[0].ends_with("/sh"));
        assert_eq!(&cmd[1..], ["-c"]);
    }

    #[cfg(not(windows))]
    #[test]
    fn desktop_files_are_read() {
        let dirs = vec![PathBuf::from("/bin"), PathBuf::from("/usr/bin")];
        let text = "[Desktop Entry]\nName=Test Web Browser\nExec=sh %U\nCategories=Network;WebBrowser;\n\n[Desktop Action new-window]\nName=New\nExec=sh --new\n";
        let e = parse_desktop("test.desktop", text, &dirs).unwrap();
        assert_eq!(clean_name(&e.name), "Test");
        assert_eq!(e.cmd.len(), 1);
        // not a browser
        assert!(parse_desktop("x.desktop", "[Desktop Entry]\nName=X\nExec=sh\nCategories=Utility;\n", &dirs).is_none());
        // a terminal program
        assert!(parse_desktop("l.desktop", "[Desktop Entry]\nName=L\nExec=sh\nCategories=WebBrowser;\nTerminal=true\n", &dirs).is_none());
    }
}
