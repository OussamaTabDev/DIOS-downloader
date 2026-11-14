//! The torrent windows: "Add torrent" (a .torrent file or a magnet link)
//! and the torrent's own progress window with its file list.

use crate::i18n::{t, tf};
use crate::*;
use dios_engine::fmt_size;
use dios_engine::model::TorrentRequest;
use dios_engine::torrent::{TorrentLimits, TorrentMeta};
use dios_engine::TorrentSource;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// A torrent whose contents were read and that waits in the "Add torrent"
/// window for Start / Download Later.
#[derive(Clone)]
pub(crate) struct PendingTorrent {
    pub source: TorrentSource,
    pub meta: TorrentMeta,
    /// indices of the ticked files
    pub files: Vec<usize>,
}

/// The torrent being added, shared with the background thread that reads
/// it (Arc, like the "Download File Info" window's add_state).
#[derive(Default)]
pub(crate) struct TorrentShared {
    pending: Mutex<Option<PendingTorrent>>,
    /// the outcome of the last "Check": (its number, where it came from,
    /// what it found). The UI timer shows it (the models live on the UI
    /// thread only)
    resolved: Mutex<Option<(u64, TorrentSource, Result<TorrentMeta, anyhow::Error>)>>,
    seq: std::sync::atomic::AtomicU64,
}

impl TorrentShared {
    fn next_seq(&self) -> u64 {
        self.seq.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1
    }
    fn seq_now(&self) -> u64 {
        self.seq.load(std::sync::atomic::Ordering::Relaxed)
    }
    fn take_pending(&self) -> Option<PendingTorrent> {
        self.pending.lock().unwrap().take()
    }
    fn set_pending(&self, p: PendingTorrent) {
        *self.pending.lock().unwrap() = Some(p);
    }
    fn pending(&self) -> Option<PendingTorrent> {
        self.pending.lock().unwrap().clone()
    }
    fn set_resolved(&self, seq: u64, source: TorrentSource, r: Result<TorrentMeta, anyhow::Error>) {
        *self.resolved.lock().unwrap() = Some((seq, source, r));
    }
    fn take_resolved(&self) -> Option<(u64, TorrentSource, Result<TorrentMeta, anyhow::Error>)> {
        self.resolved.lock().unwrap().take()
    }
}

/// One torrent progress window and its file list.
pub(crate) type TorrentWin = (TorrentDetailsDialog, Rc<VecModel<TorrentFileRow>>);

// ====================================================================== add

/// Open "Add torrent". `preset` comes from a .torrent opened with the app,
/// a magnet: link (menu, browser, command line) or the clipboard.
pub(crate) fn open_add(ui: &Ui, preset: Option<TorrentSource>) {
    let Some(d) = ui.torrent_add.upgrade() else { return };
    let s = ui.ctx.mgr.settings();
    let (mode, magnet, path) = match &preset {
        Some(TorrentSource::Magnet(m)) => (1, m.clone(), String::new()),
        Some(TorrentSource::Torrent(p)) => (0, String::new(), p.display().to_string()),
        None => {
            // a magnet link waiting in the clipboard? start with it
            let clip =
                arboard::Clipboard::new().ok().and_then(|mut c| c.get_text().ok()).unwrap_or_default();
            let clip = clip.trim().to_string();
            if clip.starts_with("magnet:") {
                (1, clip, String::new())
            } else {
                (0, String::new(), String::new())
            }
        }
    };
    reset_add(ui);
    d.set_mode(mode);
    d.set_magnet(magnet.into());
    d.set_torrent_path(path.into());
    d.set_folder(s.download_dir.display().to_string().into());
    d.set_queue_names(queue_names(&s));
    d.set_queue_index(0);
    open_window!(d);
    if preset.is_some() {
        resolve(ui);
    }
}

/// Empty the "Add torrent" window (a fresh one, or another torrent came).
fn reset_add(ui: &Ui) {
    let Some(d) = ui.torrent_add.upgrade() else { return };
    ui.torrent_shared.next_seq();
    ui.torrent_shared.take_pending();
    ui.torrent_rows.set_vec(Vec::new());
    d.set_resolved(0);
    d.set_name("—".into());
    d.set_size_text(SharedString::new());
    d.set_files_summary(SharedString::new());
    d.set_info(t("Choose a torrent file, or paste a magnet link.").into());
    d.set_info_error(false);
    d.set_description(SharedString::new());
    d.set_start_now(true);
}

fn wire_add(ui: &Ui) {
    let Some(d) = ui.torrent_add.upgrade() else { return };
    {
        let ui = ui.clone();
        d.on_choose_file(move || {
            let Some(d) = ui.torrent_add.upgrade() else { return };
            let mut dlg = rfd::FileDialog::new().set_title(t("Choose a torrent file"));
            dlg = dlg.add_filter("Torrent (.torrent)", &["torrent"]);
            if let Some(dir) = PathBuf::from(d.get_folder().as_str()).parent().filter(|p| p.exists()) {
                dlg = dlg.set_directory(dir);
            }
            if let Some(p) = dlg.pick_file() {
                d.set_torrent_path(p.display().to_string().into());
                resolve(&ui);
            }
        });
    }
    {
        let ui = ui.clone();
        d.on_resolve(move || resolve(&ui));
    }
    {
        let w = d.as_weak();
        d.on_paste_magnet(move || {
            let Some(d) = w.upgrade() else { return };
            if let Some(text) = arboard::Clipboard::new().ok().and_then(|mut c| c.get_text().ok()) {
                let text = text.trim().to_string();
                if text.starts_with("magnet:") || is_info_hash(&text) {
                    d.set_magnet(text.into());
                }
            }
        });
    }
    {
        let ui = ui.clone();
        d.on_mode_changed(move || {
            // switching the tab: the torrent read for the other one is gone
            reset_add(&ui);
        });
    }
    {
        let ui = ui.clone();
        d.on_browse_folder(move || {
            let Some(d) = ui.torrent_add.upgrade() else { return };
            let mut dlg = rfd::FileDialog::new();
            let cur = PathBuf::from(d.get_folder().as_str());
            if cur.is_dir() {
                dlg = dlg.set_directory(&cur);
            } else if let Some(dir) = cur.parent().filter(|p| p.exists()) {
                dlg = dlg.set_directory(dir);
            }
            if let Some(p) = dlg.pick_folder() {
                d.set_folder(p.display().to_string().into());
            }
        });
    }
    {
        let ui = ui.clone();
        d.on_file_toggled(move |index| toggle_add_file(&ui, index as usize, None));
    }
    {
        let ui = ui.clone();
        d.on_all_files(move |on| {
            let files: Option<Vec<usize>> = ui.torrent_shared.pending().map(|p| {
                if on {
                    (0..p.meta.files.len()).collect()
                } else {
                    Vec::new()
                }
            });
            if let Some(files) = files {
                set_add_files(&ui, files);
            }
        });
    }
    {
        let ui = ui.clone();
        d.on_start_torrent(move || confirm(&ui, true));
    }
    {
        let ui = ui.clone();
        d.on_later(move || confirm(&ui, false));
    }
    {
        let ui = ui.clone();
        d.on_cancel(move || {
            drop_pending(&ui.ctx, &ui.torrent_shared);
            if let Some(d) = ui.torrent_add.upgrade() {
                let _ = d.hide();
            }
        });
    }
    {
        let (ctx, shared) = (ui.ctx.clone(), ui.torrent_shared.clone());
        d.window().on_close_requested(move || {
            drop_pending(&ctx, &shared);
            slint::CloseRequestResponse::HideWindow
        });
    }
}

/// Forget the torrent that was read for the window. It leaves the session,
/// unless the download list already has it (its torrent must stay there).
fn drop_pending(ctx: &Ctx, shared: &Arc<TorrentShared>) {
    if let Some(p) = shared.take_pending() {
        if ctx.mgr.find_url(&p.source.display()).is_none() {
            let hash = p.meta.info_hash.clone();
            let engine = ctx.mgr.torrent_engine();
            ctx.rt.spawn(async move { engine.remove(&hash, false).await });
        }
    }
}

/// The magnet link or .torrent file of the window, ready for the engine.
fn source_of(d: &TorrentAddDialog) -> Option<TorrentSource> {
    match d.get_mode() {
        1 => {
            let m = d.get_magnet().trim().to_string();
            if m.starts_with("magnet:") {
                Some(TorrentSource::Magnet(m))
            } else if is_info_hash(&m) {
                Some(TorrentSource::Magnet(format!("magnet:?xt=urn:btih:{m}")))
            } else {
                None
            }
        }
        _ => {
            let p = PathBuf::from(d.get_torrent_path().as_str());
            p.is_file().then_some(TorrentSource::Torrent(p))
        }
    }
}

/// A bare info hash: 40 hex characters.
pub(crate) fn is_info_hash(s: &str) -> bool {
    s.len() == 40 && s.chars().all(|c| c.is_ascii_hexdigit())
}

/// Read what is inside the torrent (its metadata) in the background. A
/// .torrent file has it at once; a magnet link asks the DHT / trackers and
/// can take a minute.
fn resolve(ui: &Ui) {
    let Some(d) = ui.torrent_add.upgrade() else { return };
    let Some(source) = source_of(&d) else {
        d.set_resolved(0);
        d.set_info(t("Choose a torrent file, or paste a magnet link.").into());
        d.set_info_error(true);
        return;
    };
    let shared = ui.torrent_shared.clone();
    let seq = shared.next_seq();
    shared.take_pending();
    ui.torrent_rows.set_vec(Vec::new());
    let folder = PathBuf::from(d.get_folder().as_str());
    let _ = std::fs::create_dir_all(&folder);
    d.set_resolved(1);
    d.set_info(t("Fetching the torrent info…").into());
    d.set_info_error(false);
    d.set_name("—".into());
    d.set_size_text(SharedString::new());
    d.set_files_summary(SharedString::new());

    let engine = ui.ctx.mgr.torrent_engine();
    let settings = ui.ctx.mgr.settings();
    let limits =
        TorrentLimits { download_kb: settings.torrent_dl_limit_kb, upload_kb: settings.torrent_ul_limit_kb };
    let shared2 = ui.torrent_shared.clone();
    ui.ctx.rt.spawn(async move {
        let result = engine
            .add_paused(&source, &folder, Vec::new(), limits)
            .await
            .and_then(|handle| dios_engine::torrent::meta_of(&handle));
        // the UI timer shows the answer (the models are UI-thread only)
        shared2.set_resolved(seq, source, result);
    });
}

/// Show the answer of the last "Check" (the UI timer calls this).
pub(crate) fn poll_add(ui: &Ui) {
    let Some((seq, source, result)) = ui.torrent_shared.take_resolved() else { return };
    // reset_add also bumps the number: only the newest answer is shown
    if ui.torrent_shared.seq_now() != seq {
        return;
    }
    let Some(d) = ui.torrent_add.upgrade() else { return };
    match result {
        Ok(meta) => {
            d.set_name(meta.name.clone().into());
            d.set_size_text(fmt_size(meta.total_size).into());
            d.set_info(tf("{} files  \u{b7}  {}", &[&meta.files.len(), &fmt_size(meta.total_size)]).into());
            d.set_info_error(false);
            d.set_resolved(2);
            let all: Vec<usize> = (0..meta.files.len()).collect();
            ui.torrent_shared.set_pending(PendingTorrent { source, meta, files: all });
            refresh_add_files(ui);
        }
        Err(e) => {
            d.set_resolved(0);
            d.set_info(format!("{e:#}").into());
            d.set_info_error(true);
        }
    }
}

/// Fill the file list of the "Add torrent" window from the pending torrent.
fn refresh_add_files(ui: &Ui) {
    let Some(p) = ui.torrent_shared.pending() else { return };
    let rows: Vec<TorrentFileRow> = p
        .meta
        .files
        .iter()
        .map(|f| TorrentFileRow {
            index: f.index as i32,
            name: f.path.clone().into(),
            size: fmt_size(f.size).into(),
            progress: 0.0,
            checked: p.files.contains(&f.index),
        })
        .collect();
    let selected = p.meta.size_of(&p.files);
    let text = tf(
        "{} of {} files  ·  {} of {}",
        &[&p.files.len(), &p.meta.files.len(), &fmt_size(selected), &fmt_size(p.meta.total_size)],
    );
    update_model(&ui.torrent_rows, rows);
    if let Some(d) = ui.torrent_add.upgrade() {
        d.set_files_summary(text.into());
    }
}

fn set_add_files(ui: &Ui, files: Vec<usize>) {
    if let Some(p) = ui.torrent_shared.pending.lock().unwrap().as_mut() {
        p.files = files;
    }
    refresh_add_files(ui);
}

fn toggle_add_file(ui: &Ui, index: usize, want: Option<bool>) {
    if let Some(p) = ui.torrent_shared.pending.lock().unwrap().as_mut() {
        let has = p.files.contains(&index);
        let wanted = want.unwrap_or(!has);
        if wanted && !has {
            p.files.push(index);
        } else if !wanted {
            p.files.retain(|&f| f != index);
        }
    }
    refresh_add_files(ui);
}

/// "Start Download" / "Download Later": the torrent becomes a list entry.
fn confirm(ui: &Ui, start: bool) {
    let Some(d) = ui.torrent_add.upgrade() else { return };
    let Some(p) = ui.torrent_shared.pending() else { return };
    let folder = PathBuf::from(d.get_folder().as_str().trim());
    if folder.as_os_str().is_empty() {
        d.set_info(t("Choose a folder to save the torrent into.").into());
        d.set_info_error(true);
        return;
    }
    let url = p.source.display();
    if ui.ctx.mgr.find_url(&url).is_some() {
        let _g = ModalGuard::new();
        let go = rfd::MessageDialog::new()
            .set_title(APP)
            .set_description(t("This torrent is already in the download list. Add it again?"))
            .set_buttons(rfd::MessageButtons::OkCancel)
            .show();
        if !matches!(go, rfd::MessageDialogResult::Ok | rfd::MessageDialogResult::Yes) {
            return;
        }
    }
    let description = d.get_description().trim().to_string();
    let settings = ui.ctx.mgr.settings();
    let queue = settings.queues.get(d.get_queue_index().max(0) as usize).map(|q| q.id);
    let req = TorrentRequest {
        source: p.source.clone(),
        meta: p.meta.clone(),
        files: p.files.clone(),
        save_dir: folder,
        queue: if start { None } else { queue },
        description: (!description.is_empty()).then_some(description),
        start,
    };
    match ui.ctx.mgr.add_torrent(req) {
        Ok(id) => {
            // the torrent stays in the session; only the window's copy goes
            ui.torrent_shared.take_pending();
            let _ = d.hide();
            if start {
                ui.ctx.mgr.start(id);
                open_files(ui, id);
            }
            refresh(ui);
        }
        Err(e) => {
            d.set_info(format!("{e:#}").into());
            d.set_info_error(true);
        }
    }
}

// =========================================================== progress windows

/// The torrent's file window (like the progress window of other downloads).
pub(crate) fn open_files(ui: &Ui, id: u64) {
    let Some(v) = ui.ctx.mgr.get(id) else { return };
    if let Some((d, rows)) = ui.torrent_wins.borrow().get(&id) {
        let have = ui.ctx.mgr.torrent_have(id);
        fill_details(d, rows, &v, have);
        open_window!(d);
        d.window().set_minimized(false);
        return;
    }
    let Ok(d) = TorrentDetailsDialog::new() else { return };
    let settings = ui.ctx.mgr.settings();
    set_theme_of(d.global::<Theme>(), &Look::of(&settings));
    {
        let mgr = ui.ctx.mgr.clone();
        d.on_pause(move || mgr.pause(id));
    }
    {
        let mgr = ui.ctx.mgr.clone();
        d.on_resume(move || mgr.start(id));
    }
    {
        let (mgr, wins) = (ui.ctx.mgr.clone(), ui.torrent_wins.clone());
        d.on_cancel_download(move || {
            mgr.pause(id);
            close_file_window(&wins, id);
        });
    }
    {
        let wins = ui.torrent_wins.clone();
        d.on_close_window(move || close_file_window(&wins, id));
    }
    {
        let wins = ui.torrent_wins.clone();
        let w = d.as_weak();
        d.on_hide_to_tray(move || {
            if let Some(d) = w.upgrade() {
                if HAS_TRAY.with(|h| h.get()) {
                    wins.borrow_mut().remove(&id);
                    HIDDEN_PROGRESS.with(|h| h.borrow_mut().insert(id));
                    let _ = d.hide();
                } else {
                    d.window().set_minimized(true);
                }
            }
        });
    }
    {
        let mgr = ui.ctx.mgr.clone();
        d.on_open_folder(move || {
            if let Some(v) = mgr.get(id) {
                reveal_in_folder(&v);
            }
        });
    }
    {
        let mgr = ui.ctx.mgr.clone();
        d.on_file_toggled(move |index| {
            // flip that file in the record's selection
            let Some(v) = mgr.get(id) else { return };
            let Some(t) = v.torrent else { return };
            let mut files = t.files.clone();
            if files.contains(&(index as usize)) {
                files.retain(|&f| f != index as usize);
            } else {
                files.push(index as usize);
            }
            mgr.set_torrent_files(id, files);
        });
    }
    {
        let mgr = ui.ctx.mgr.clone();
        d.on_all_files(move |on| {
            let count = mgr.get(id).and_then(|v| v.torrent).map(|t| t.file_list.len()).unwrap_or(0);
            let files: Vec<usize> = if on { (0..count).collect() } else { Vec::new() };
            mgr.set_torrent_files(id, files);
        });
    }
    let rows = Rc::new(VecModel::<TorrentFileRow>::default());
    d.set_files(ModelRc::from(rows.clone()));
    let have = ui.ctx.mgr.torrent_have(id);
    fill_details(&d, &rows, &v, have);
    open_window!(d);
    ui.torrent_wins.borrow_mut().insert(id, (d, rows));
}

fn close_file_window(wins: &Rc<RefCell<HashMap<u64, TorrentWin>>>, id: u64) {
    if let Some((d, _)) = wins.borrow().get(&id) {
        let _ = d.hide();
    }
    let wins = wins.clone();
    slint::Timer::single_shot(Duration::from_millis(1), move || {
        wins.borrow_mut().remove(&id);
    });
}

/// Refresh every open torrent window (the fast timer calls this).
pub(crate) fn poll_windows(ui: &Ui) {
    let ids: Vec<u64> = ui.torrent_wins.borrow().keys().copied().collect();
    for id in ids {
        match ui.ctx.mgr.get(id) {
            // finished: the "complete" window takes over
            Some(v) if v.status == Status::Completed => close_file_window(&ui.torrent_wins, id),
            Some(v) => {
                let wins = ui.torrent_wins.borrow();
                if let Some((d, rows)) = wins.get(&id) {
                    let have = ui.ctx.mgr.torrent_have(id);
                    fill_details(d, rows, &v, have);
                    winfx::taskbar_progress(d.window(), taskbar_state(&v));
                }
            }
            None => close_file_window(&ui.torrent_wins, id),
        }
    }
}

fn file_rows(
    t: &dios_engine::model::TorrentEntry,
    downloaded: u64,
    have: &[u64],
) -> (Vec<TorrentFileRow>, String) {
    let rows: Vec<TorrentFileRow> = t
        .file_list
        .iter()
        .enumerate()
        .map(|(i, f)| TorrentFileRow {
            index: i as i32,
            name: f.path.clone().into(),
            size: fmt_size(f.size).into(),
            progress: if f.size > 0 {
                have.get(i).copied().unwrap_or(0) as f32 / f.size as f32
            } else {
                1.0
            },
            checked: t.files.is_empty() || t.files.contains(&i),
        })
        .collect();
    let got = if t.files.is_empty() {
        downloaded.min(t.total_size)
    } else {
        t.files.iter().filter_map(|&fi| have.get(fi).copied()).sum::<u64>().min(t.total_size)
    };
    let count = if t.files.is_empty() { t.file_list.len() } else { t.files.len() };
    let text =
        tf("{} of {} files  ·  {} of {}", &[&count, &t.file_list.len(), &fmt_size(got), &fmt_size(t.total_size)]);
    (rows, text)
}

fn fill_details(
    d: &TorrentDetailsDialog,
    rows: &VecModel<TorrentFileRow>,
    v: &DownloadView,
    have: Vec<u64>,
) {
    let pct = v.progress();
    d.set_name(v.file_name.clone().into());
    d.set_percent(format!("{:.0}%", pct * 100.0).into());
    d.set_url(v.url.clone().into());
    let status = if v.checking {
        t("Checking existing files…")
    } else {
        match v.status {
            Status::Downloading => t("Downloading from peers…"),
            Status::Assembling => t("Joining the parts…"),
            Status::Failed => t("Error"),
            other => t(other.label()),
        }
    };
    d.set_status(status.into());
    d.set_size(fmt_size(v.torrent.as_ref().map(|t| t.total_size).unwrap_or(0)).into());
    d.set_downloaded(format!("{} ({:.1} %)", fmt_size(v.downloaded), pct * 100.0).into());
    d.set_uploaded(fmt_size(v.uploaded).into());
    let rate = |s: f64| if s > 0.0 { format!("{}/s", fmt_size(s as u64)) } else { "—".to_string() };
    d.set_down_speed(rate(v.speed).into());
    d.set_up_speed(rate(v.up_speed).into());
    d.set_eta(v.eta_secs.map(fmt_duration).unwrap_or_else(|| "—".into()).into());
    d.set_peers(tf("{} connected  ·  {} seen", &[&v.peers, &v.seen]).into());
    d.set_error(v.error.clone().unwrap_or_default().into());
    d.set_progress(pct);
    d.set_state(state_of(v.status));

    if let Some(t) = &v.torrent {
        let (new_rows, summary) = file_rows(t, v.downloaded, &have);
        update_model(rows, new_rows);
        d.set_files_summary(summary.into());
    }
}

// ===================================================================== wires

pub(crate) fn wire(ui: &Ui) {
    wire_add(ui);
}
