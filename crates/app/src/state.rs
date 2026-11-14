//! What the interface needs to remember between two requests: the selection,
//! the open view, the dialogs that are waiting. It lives next to the download
//! manager in one `App` that every request handler shares.
use batiie_engine::{DownloadId, Manager};
use std::collections::VecDeque;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, MutexGuard};
use tokio::sync::broadcast;

/// A window of its own (not a part of the main window): where it opens, how big it is.
#[derive(Debug, Clone)]
pub struct WinSpec {
    /// identifies the window (a second request for the same label brings it to the front)
    pub label: String,
    /// the page it shows, e.g. `/w/dlg/d-add`
    pub path: String,
    pub title: String,
    pub w: f64,
    pub h: f64,
    pub min_w: f64,
    pub min_h: f64,
    /// stays above the other windows (the questions that must not be missed)
    pub on_top: bool,
    /// opens under the mouse pointer (otherwise in the middle of the screen the pointer is on)
    pub at_cursor: bool,
}

/// What the program asks the window to do.
#[derive(Debug, Clone)]
pub enum ShellCmd {
    /// open one more window
    Open(WinSpec),
    /// close a window of its own without asking it
    Close(String),
    /// the page of a window says something (`close`, `title:…`)
    Ipc(String, String),
    /// a window did not answer its close request: close it now
    ForceClose(String),
    /// bring the window to the front (restore it from the tray)
    Show,
    /// hide the window (the program keeps running in the tray)
    Hide,
    Quit,
    #[allow(dead_code)] // the window title is only set by the native window
    Title(String),
}

/// Something the server tells the open page.
#[derive(Debug, Clone)]
pub enum Push {
    /// load this address into the dialog area of the page
    Open(String),
    /// a plain message box
    Alert(String),
    /// a short message that fades away
    Toast(String),
    /// close the dialog with this id
    Close(String),
}

#[derive(Debug, Clone, Default)]
pub struct FindOpts {
    pub text: String,
    pub case: bool,
    pub whole: bool,
    pub in_name: bool,
    pub in_desc: bool,
    pub in_addr: bool,
}

pub struct UiState {
    /// "all", "unf", "fin", "cat:Music", "fin:Music", "q:3", "tor" ...
    pub view: String,
    pub search: String,
    pub sel: Vec<DownloadId>,
    pub anchor: Option<DownloadId>,
    pub focus: Option<DownloadId>,
    pub open_all: bool,
    pub open_unf: bool,
    pub open_fin: bool,
    pub open_queues: bool,
    /// downloads that have their progress window open
    pub progress_open: Vec<DownloadId>,
    pub find: FindOpts,
    /// the window is in the tray
    pub hidden: bool,
}

impl Default for UiState {
    fn default() -> Self {
        UiState {
            view: "all".into(),
            search: String::new(),
            sel: Vec::new(),
            anchor: None,
            focus: None,
            open_all: true,
            open_unf: false,
            open_fin: false,
            open_queues: true,
            progress_open: Vec::new(),
            find: FindOpts { in_name: true, in_desc: true, in_addr: true, ..Default::default() },
            hidden: false,
        }
    }
}

pub struct App {
    pub mgr: Arc<Manager>,
    pub rt: tokio::runtime::Handle,
    /// the secret of this run: only the window that got it may talk to us
    pub token: String,
    pub port: std::sync::atomic::AtomicU16,
    pub ui: Mutex<UiState>,
    pub bus: broadcast::Sender<Push>,
    pub shell: Mutex<Sender<ShellCmd>>,
    pub add: Mutex<crate::web::add::AddHub>,
    pub misc: Mutex<crate::web::misc::MiscState>,
    pub clip_last: Mutex<String>,
    pub started: std::time::Instant,
    /// windows of their own are available (the native web view runs)
    pub windowed: std::sync::atomic::AtomicBool,
    /// pages of the windows that were opened from a dialog (label -> its HTML)
    pub wins: Mutex<std::collections::HashMap<String, WinPage>>,
    /// speed of every running download, once a second (for the graphs)
    pub speed_hist: Mutex<std::collections::HashMap<DownloadId, VecDeque<f32>>>,
    pub total_hist: Mutex<VecDeque<f32>>,
    pub win_seq: std::sync::atomic::AtomicU64,
    /// the live connections look at the changes right now
    pub wake: tokio::sync::Notify,
    /// the app itself (background tasks need a handle that outlives a request)
    pub me: Mutex<std::sync::Weak<App>>,
    /// messages for a page that is not connected yet (start-up)
    pub pending: Mutex<Vec<Push>>,
}

impl App {
    pub fn ui(&self) -> MutexGuard<'_, UiState> {
        self.ui.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn push(&self, p: Push) {
        // nobody listens yet (the window is still loading): keep it for the first page
        if self.bus.receiver_count() == 0 {
            self.pending.lock().unwrap_or_else(|e| e.into_inner()).push(p);
        } else {
            let _ = self.bus.send(p);
        }
    }

    pub fn alert(&self, text: impl Into<String>) {
        let text = text.into();
        if self.windowed.load(std::sync::atomic::Ordering::Relaxed) {
            crate::web::windows::open_alert(self, &text);
        } else {
            self.push(Push::Alert(text));
        }
    }

    pub fn toast(&self, text: impl Into<String>) {
        self.push(Push::Toast(text.into()));
    }

    pub fn open(&self, path: impl Into<String>) {
        self.push(Push::Open(path.into()));
    }

    pub fn shell(&self, c: ShellCmd) {
        if let Ok(s) = self.shell.lock() {
            let _ = s.send(c);
        }
    }
}


/// The page of a window that came from a dialog.
#[derive(Clone)]
pub struct WinPage {
    pub title: String,
    pub html: String,
}
