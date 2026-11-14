use super::Shared;
use crate::state::App;
use axum::Router;
use batiie_engine::{DownloadId, TorrentSource};
#[derive(Default)]
pub struct MiscState {
    pub refresh_id: Option<DownloadId>,
    pub batch_text: String,
    pub playlist: super::playlist::PlaylistHub,
    pub grab: super::grab::GrabHub,
    pub torrent: super::torrent::TorrentHub,
    pub browsers: Vec<crate::browsers::Browser>,
    pub update: std::sync::Arc<std::sync::Mutex<super::update::Shown>>,
    /// the shut-down / sleep countdown: what, seconds left
    pub power: Option<(batiie_engine::config::AfterDone, i32)>,
}
/// The clock of the program: completions, sounds, shut-down requests, the scheduler's
/// countdown. The windows themselves are updated by their own live connection.
pub fn spawn_ticker(app: std::sync::Arc<App>) {
    let rt = app.rt.clone();
    rt.spawn(async move {
        let mut iv = tokio::time::interval(std::time::Duration::from_millis(250));
        iv.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut n: u32 = 0;
        loop {
            iv.tick().await;
            n = n.wrapping_add(1);
            tick(&app);
            #[allow(clippy::manual_is_multiple_of)]
            if n % 4 == 0 {
                super::progress::tick_countdown(&app);
                sample_speeds(&app);
            }
        }
    });
}

/// Once a second: the speed of every running download and of all of them together, for the graphs.
fn sample_speeds(app: &App) {
    use batiie_engine::Status;
    const KEEP: usize = 90;
    let views = app.mgr.snapshot();
    let mut hist = app.speed_hist.lock().unwrap_or_else(|e| e.into_inner());
    let mut total = 0.0_f32;
    for v in &views {
        if v.status.is_running() {
            let h = hist.entry(v.id).or_default();
            h.push_back(v.speed as f32);
            while h.len() > KEEP {
                h.pop_front();
            }
            total += v.speed as f32;
        }
    }
    // the history of a download that is not running is kept for a while (pause / resume), then dropped
    hist.retain(|id, _| views.iter().any(|v| v.id == *id && v.status != Status::Completed));
    let mut t = app.total_hist.lock().unwrap_or_else(|e| e.into_inner());
    t.push_back(total);
    while t.len() > KEEP {
        t.pop_front();
    }
}

fn tick(app: &App) {
    use batiie_engine::{Event, Status};
    let early = app.add.lock().unwrap_or_else(|e| e.into_inner()).early_id();
    // a finished download closes its progress window; "Download complete" follows
    let finished: Vec<u64> = app.mgr.take_completed().into_iter().filter(|id| Some(*id) != early).collect();
    if let Some(&last) = finished.last() {
        if app.mgr.peek(|s| s.show_complete_dialog) {
            super::progress::show_complete(app, last);
        }
    }
    // a finished (or removed) download closes its progress window
    let open: Vec<u64> = app.ui().progress_open.clone();
    for id in open {
        let gone = match app.mgr.get(id) {
            None => true,
            Some(v) => v.status == Status::Completed,
        };
        if gone {
            super::progress::close_window(app, id);
        }
    }
    // finished downloads close the windows the shell opened too (they were not in the list above)
    for &id in &finished {
        super::progress::close_window(app, id);
    }
    let events = app.mgr.take_events();
    if !events.is_empty() {
        let s = app.mgr.settings();
        for ev in events {
            match ev {
                Event::Completed(id) | Event::Failed(id) if Some(id) == early => {}
                Event::Completed(_) if s.sounds_on => crate::sound::play(&s.sound_complete),
                Event::Failed(id) => {
                    if s.sounds_on {
                        crate::sound::play(&s.sound_failed);
                    }
                    if let Some(v) = app.mgr.get(id) {
                        app.toast(crate::i18n::tf("That download hit a bump: {}. Try again?", &[&v.file_name]));
                    }
                }
                Event::QueueStarted(_) if s.sounds_on => crate::sound::play(&s.sound_queue_start),
                Event::QueueDone(_) if s.sounds_on => crate::sound::play(&s.sound_queue_done),
                Event::QuotaReached if s.quota_warn => app.alert(crate::i18n::tf(
                    "The download limit of {} MB every {} hours was reached, so the downloads were stopped.\nThey continue by themselves when the limit allows it again (Options > Connection).",
                    &[&s.quota_mb, &s.quota_hours],
                )),
                Event::OpenFile(id) => {
                    if let Some(v) = app.mgr.get(id) {
                        let _ = open::that(&v.save_path);
                    }
                }
                _ => {}
            }
        }
    }
    if let Some(action) = app.mgr.take_power_request() {
        super::progress::start_countdown(app, action);
    }
    if app.mgr.take_exit_request() {
        app.shell(crate::state::ShellCmd::Quit);
    }
}
/// After an update: what is new; the first start: the browser add-in; otherwise a tip.
pub fn first_run_dialogs(app: &App) {
    let mut s = app.mgr.settings();
    let now = env!("CARGO_PKG_VERSION");
    if s.last_version != now {
        // an empty last_version on a copy used before = an update from 1.0.3
        let updated = if s.last_version.is_empty() { s.extension_prompted } else { true };
        s.last_version = now.into();
        let _ = app.mgr.set_settings(s);
        if updated {
            super::info::whats_new(app);
        }
    }
    let mut s = app.mgr.settings();
    if !s.extension_prompted {
        // first start: help adding the browser extension
        s.extension_prompted = true;
        let _ = app.mgr.set_settings(s);
        app.open("/dlg/browsers");
    } else if s.show_tips {
        super::info::show_tip(app, false);
    }
}
pub fn open_torrent_add(app: &App, s: Option<TorrentSource>) {
    super::torrent::open_add(app, s)
}
pub fn open_links(app: &App, l: crate::api::LinksRequest) {
    super::grab::open_links(app, l)
}
pub use super::info::{properties_dialog, refresh_dialog, refresh_received};
pub fn routes(r: Router<Shared>) -> Router<Shared> {
    r
}
