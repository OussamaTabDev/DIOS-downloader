//! Clipboard monitor: when the user copies a link to a downloadable file,
//! offer to download it (like classic download managers).

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

const EXTENSIONS: &[&str] = &[
    "zip", "rar", "7z", "tar", "gz", "tgz", "bz2", "xz", "iso", "exe", "msi", "msix", "apk", "dmg",
    "deb", "rpm", "mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "m4v", "mp3", "wav", "flac",
    "aac", "m4a", "ogg", "pdf", "epub", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "m3u8", "bin",
    "img", "torrent",
];

/// When the program itself last put something in the clipboard (ms tick):
/// the "Copy URL" menu, the SHA-256 window... The monitor keeps quiet for a
/// moment after that, or it would offer the user their own copy back.
static SELF_COPY_MS: AtomicU64 = AtomicU64::new(0);

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Something was copied by the program itself: don't offer it back.
pub fn mark_self_copy() {
    SELF_COPY_MS.store(now_ms(), Ordering::Relaxed);
}

fn self_copy_recent() -> bool {
    let at = SELF_COPY_MS.load(Ordering::Relaxed);
    at != 0 && now_ms().saturating_sub(at) < 3000
}

/// Returns the URL when `text` is a single http(s) link to a known file type.
pub fn downloadable_url(text: &str) -> Option<String> {
    let t = text.trim();
    if t.is_empty() || t.len() > 4096 || t.contains(char::is_whitespace) {
        return None;
    }
    let u = url::Url::parse(t).ok()?;
    if u.scheme() != "http" && u.scheme() != "https" {
        return None;
    }
    let path = u.path().to_ascii_lowercase();
    let ext = path.rsplit('.').next()?;
    if path.contains('.') && EXTENSIONS.contains(&ext) {
        Some(t.to_string())
    } else {
        None
    }
}

/// What the monitor offers when the user copies something anywhere:
/// a direct file link, or a magnet link (a torrent in waiting).
pub fn monitor_pickup(text: &str) -> Option<String> {
    if let Some(u) = downloadable_url(text) {
        return Some(u);
    }
    let t = text.trim();
    (t.starts_with("magnet:") && t.len() <= 4096 && !t.contains(char::is_whitespace)).then(|| t.to_string())
}

/// Polls the clipboard once a second on its own thread. `on_pickup` gets a
/// link or a magnet the user copied somewhere else.
pub fn spawn(enabled: impl Fn() -> bool + Send + 'static, on_pickup: impl Fn(String) + Send + 'static) {
    std::thread::Builder::new()
        .name("dios-clipboard".into())
        .spawn(move || {
            let Ok(mut clip) = arboard::Clipboard::new() else { return };
            // ignore whatever was already copied before we started
            let mut last = clip.get_text().unwrap_or_default();
            loop {
                std::thread::sleep(Duration::from_millis(1000));
                let Ok(text) = clip.get_text() else { continue };
                if text == last {
                    continue;
                }
                last = text.clone();
                if !enabled() || self_copy_recent() {
                    continue;
                }
                if let Some(u) = monitor_pickup(&text) {
                    on_pickup(u);
                }
            }
        })
        .ok();
}
