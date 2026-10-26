//! Paste & download: what Ctrl+V (the toolbar's Paste button, a menu item,
//! a link dropped on the window) does with the clipboard's content.
//!
//! The clipboard holds one thing, but many kinds of it. `classify` tells
//! them apart, `paste_from_clipboard` and `handle_drop` send each kind
//! where it belongs:
//!
//! * a magnet link (or a bare info hash) -> "Add torrent", checked at once
//! * a path to a .torrent file -> "Add torrent" with the file
//! * one direct file link (a known extension) -> the list, started at once
//! * one other link -> "Download File Info" filled in, the check running
//! * several links -> "Add Batch" filled in
//! * nothing usable -> a quiet word in the status bar (no message box)

use crate::clipboard as clip;
use crate::i18n::{t, tf};
use crate::*;
use std::path::PathBuf;

/// What the clipboard (or a drop) holds.
#[derive(Debug, PartialEq, Clone)]
pub(crate) enum Clip {
    /// a magnet: link, or a bare 40-character info hash (sent as a magnet)
    Magnet(String),
    /// a .torrent file that exists on the disk
    Torrent(PathBuf),
    /// http(s) links (one, or several for the batch window)
    Links(Vec<String>),
    Nothing,
}

/// What the text is: a magnet, a .torrent file, links, or nothing.
pub(crate) fn classify(text: &str) -> Clip {
    let t = text.trim();
    if t.is_empty() || t.len() > 4096 {
        return Clip::Nothing;
    }
    // one line only? a magnet, a hash or a single link; several lines can
    // still be one link wrapped, so try the whole text as a link first
    if let Some(u) = is_link(t) {
        return Clip::Links(vec![u]);
    }
    if t.starts_with("magnet:") && t.len() < 4096 {
        return Clip::Magnet(t.to_string());
    }
    if torrent::is_info_hash(t) {
        return Clip::Magnet(format!("magnet:?xt=urn:btih:{t}"));
    }
    // a path to a .torrent file (quoted paths from Explorer keep their quotes off)
    let path = t.trim_matches('"');
    if !path.contains('\n') && path.to_ascii_lowercase().ends_with(".torrent") && Path::new(path).is_file() {
        return Clip::Torrent(PathBuf::from(path));
    }
    // several links in one block of text
    let links = extract_links(t);
    if links.len() > 1 {
        return Clip::Links(links);
    }
    Clip::Nothing
}

/// A single http(s) link, with quotes and <> stripped from the ends.
fn is_link(t: &str) -> Option<String> {
    let s = t.trim_matches(|c| c == '"' || c == '\'' || c == '<' || c == '>');
    (s.starts_with("http://") || s.starts_with("https://"))
        .then(|| s.to_string())
        .filter(|s| !s.contains(char::is_whitespace) && url::Url::parse(s).is_ok())
}

/// Ctrl+V / the Paste button: read the clipboard and act on it.
pub(crate) fn paste_from_clipboard(ui: &Ui) {
    let text = match arboard::Clipboard::new() {
        Ok(mut c) => c.get_text().unwrap_or_default(),
        Err(_) => {
            flash(ui, t("The clipboard is not available"));
            return;
        }
    };
    dispatch(ui, &text);
}

/// A link (or several, or a magnet) was dropped on the window as text.
pub(crate) fn handle_drop_text(ui: &Ui, text: &str) {
    dispatch(ui, text);
}

/// Files dropped on the window: .torrent files open in "Add torrent",
/// everything else gets a word in the status bar.
pub(crate) fn handle_drop_files(ui: &Ui, paths: &[PathBuf]) {
    let torrents: Vec<&PathBuf> =
        paths.iter().filter(|p| p.to_string_lossy().to_ascii_lowercase().ends_with(".torrent")).collect();
    if !torrents.is_empty() {
        for p in torrents.iter().take(1) {
            torrent::open_add(ui, Some(dios_engine::TorrentSource::Torrent((*p).clone())));
        }
        if torrents.len() > 1 {
            flash(ui, tf("{} torrent files dropped — the first one opens", &[&torrents.len()]));
        }
        return;
    }
    let n = paths.len();
    if n > 0 {
        flash(ui, t("Only .torrent files can be dropped on the window"));
    }
}

/// The one dispatcher for the clipboard and for drops.
fn dispatch(ui: &Ui, text: &str) {
    match classify(text) {
        Clip::Magnet(m) => {
            torrent::open_add(ui, Some(dios_engine::TorrentSource::Magnet(m)));
        }
        Clip::Torrent(p) => {
            torrent::open_add(ui, Some(dios_engine::TorrentSource::Torrent(p)));
        }
        Clip::Links(links) => {
            if links.len() > 1 {
                open_batch(ui, text.to_string());
                flash(ui, tf("{} links pasted — pick the folder and start them", &[&links.len()]));
                return;
            }
            let url = links.into_iter().next().unwrap_or_default();
            one_link(ui, url);
        }
        Clip::Nothing => {
            flash(ui, t("No download link found in the clipboard"));
        }
    }
}

/// One link: a direct file goes to the list at once; a page opens in
/// "Download File Info" (the check starts by itself); an old friend is
/// brought back instead of added twice.
fn one_link(ui: &Ui, url: String) {
    // already in the list: show it / continue it, never add it twice
    if let Some(id) = ui.ctx.mgr.find_url(&url) {
        if let Some(v) = ui.ctx.mgr.get(id) {
            if v.status.is_running() || v.status == Status::Completed {
                flash(ui, t("This link is already in the download list"));
            } else {
                ui.ctx.mgr.start(id);
                if v.kind == DownloadKind::Torrent {
                    torrent::open_files(ui, id);
                } else {
                    open_progress(ui, id);
                }
                flash(ui, t("This link is already in the list — it continues now"));
            }
        }
        return;
    }
    if clip::downloadable_url(&url).is_some() {
        // a file with a known extension: add it and go
        let id = ui.ctx.mgr.add(NewDownload { url: url.clone(), start: true, ..Default::default() });
        show_progress_for_start(ui, id);
        refresh(ui);
        flash(ui, tf("Download added from the clipboard: {}", &[&filename_from(&url)]));
        return;
    }
    // a page (or an unknown thing): let the user look at it first —
    // the "Download File Info" window opens with the check already running
    open_add_dialog(
        &ui.ctx,
        url,
        RequestHeaders::default(),
        None,
        VideoExtra::default(),
    );
    flash(ui, t("Link pasted — press Start to download it"));
}

/// The file name of a link (for the status bar).
fn filename_from(url: &str) -> String {
    dios_engine::filename::from_url(url)
        .map(|n| dios_engine::filename::sanitize(&n))
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| url.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn magnets_and_hashes() {
        assert_eq!(
            classify("magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567&dn=x"),
            Clip::Magnet("magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567&dn=x".into())
        );
        // a bare 40-character hash becomes a magnet link
        assert_eq!(
            classify("0123456789ABCDEF0123456789ABCDEF01234567"),
            Clip::Magnet("magnet:?xt=urn:btih:0123456789ABCDEF0123456789ABCDEF01234567".into())
        );
        // 39 or 41 characters are not a hash
        assert_eq!(classify("0123456789abcdef0123456789abcdef0123456"), Clip::Nothing);
        assert_eq!(classify("0123456789abcdef0123456789abcdef012345678"), Clip::Nothing);
    }

    #[test]
    fn links() {
        assert_eq!(
            classify("https://example.com/setup.exe"),
            Clip::Links(vec!["https://example.com/setup.exe".into()])
        );
        // < > quotes and spaces around it are stripped
        assert_eq!(
            classify("  <https://example.com/a.mp4>  "),
            Clip::Links(vec!["https://example.com/a.mp4".into()])
        );
        // a page link is still a link (the window decides what it is)
        assert_eq!(classify("https://example.com/"), Clip::Links(vec!["https://example.com/".into()]));
        // several links go to the batch window
        assert_eq!(
            classify("https://a.com/1.zip\r\nhttps://a.com/2.zip"),
            Clip::Links(vec!["https://a.com/1.zip".into(), "https://a.com/2.zip".into()])
        );
    }

    #[test]
    fn nothing_useful() {
        assert_eq!(classify(""), Clip::Nothing);
        assert_eq!(classify("   \n  "), Clip::Nothing);
        assert_eq!(classify("hello world"), Clip::Nothing);
        assert_eq!(classify("ftp://example.com/f.zip"), Clip::Nothing);
        // text with a link in a sentence is not one clean link
        assert_eq!(classify("look at https://example.com/a.zip please"), Clip::Nothing);
    }

    #[test]
    fn torrent_files() {
        // a path to a real .torrent file
        let dir = std::env::temp_dir().join("dios-paste-test");
        let _ = std::fs::create_dir_all(&dir);
        let p = dir.join("test.torrent");
        std::fs::write(&p, b"d4:infod4:name4:teste").unwrap();
        assert_eq!(classify(p.display().to_string().as_str()), Clip::Torrent(p.clone()));
        // quoted, like Explorer drops paths
        assert_eq!(classify(format!("\"{}\"", p.display()).as_str()), Clip::Torrent(p.clone()));
        // a .torrent that does not exist is not one
        assert_eq!(classify("/no/such/file.torrent"), Clip::Nothing);
        let _ = std::fs::remove_file(&p);
    }
}
