//! Icon next to the clock (Windows) / in the system tray (Linux): the app keeps
//! running there when its window is closed, like other download managers.

#[allow(dead_code)]
pub enum TrayAction {
    Show,
    AddUrl,
    ResumeAll,
    StopAll,
    ToggleLimiter,
    Scheduler,
    Options,
    About,
    Exit,
}

#[cfg(windows)]
mod imp {
    use super::TrayAction;
    use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
    use tray_icon::{Icon, MouseButton, TrayIcon, TrayIconBuilder, TrayIconEvent};

    pub struct Tray {
        _icon: TrayIcon,
        limiter: CheckMenuItem,
        ids: Vec<(MenuId, fn() -> TrayAction)>,
    }

    impl Tray {
        pub fn new() -> Option<Tray> {
            let menu = Menu::new();
            let show = MenuItem::new("Restore DIOS Downloader", true, None);
            let add = MenuItem::new("Add new download…", true, None);
            let resume = MenuItem::new("Resume all downloads", true, None);
            let stop = MenuItem::new("Stop all downloads", true, None);
            let limiter = CheckMenuItem::new("Speed Limiter", true, false, None);
            let sched = MenuItem::new("Scheduler…", true, None);
            let options = MenuItem::new("Options…", true, None);
            let about = MenuItem::new("About", true, None);
            let exit = MenuItem::new("Exit", true, None);
            menu.append(&show).ok()?;
            menu.append(&PredefinedMenuItem::separator()).ok()?;
            menu.append(&add).ok()?;
            menu.append(&resume).ok()?;
            menu.append(&stop).ok()?;
            menu.append(&PredefinedMenuItem::separator()).ok()?;
            menu.append(&limiter).ok()?;
            menu.append(&sched).ok()?;
            menu.append(&options).ok()?;
            menu.append(&PredefinedMenuItem::separator()).ok()?;
            menu.append(&about).ok()?;
            menu.append(&exit).ok()?;
            let icon = Icon::from_rgba(include_bytes!("../ui/tray32.rgba").to_vec(), 32, 32).ok()?;
            let tray = TrayIconBuilder::new()
                .with_tooltip("DIOS Downloader")
                .with_icon(icon)
                .with_menu(Box::new(menu))
                // the menu on a right click only; a double click opens the window
                .with_menu_on_left_click(false)
                .build()
                .ok()?;
            let ids: Vec<(MenuId, fn() -> TrayAction)> = vec![
                (show.id().clone(), || TrayAction::Show),
                (add.id().clone(), || TrayAction::AddUrl),
                (resume.id().clone(), || TrayAction::ResumeAll),
                (stop.id().clone(), || TrayAction::StopAll),
                (limiter.id().clone(), || TrayAction::ToggleLimiter),
                (sched.id().clone(), || TrayAction::Scheduler),
                (options.id().clone(), || TrayAction::Options),
                (about.id().clone(), || TrayAction::About),
                (exit.id().clone(), || TrayAction::Exit),
            ];
            Some(Tray { _icon: tray, limiter, ids })
        }

        /// Clicks since the last call.
        pub fn poll(&self) -> Vec<TrayAction> {
            let mut out = Vec::new();
            while let Ok(ev) = TrayIconEvent::receiver().try_recv() {
                // left double click: the window (the menu is on the right button)
                if let TrayIconEvent::DoubleClick { button: MouseButton::Left, .. } = ev {
                    out.push(TrayAction::Show);
                }
            }
            while let Ok(ev) = MenuEvent::receiver().try_recv() {
                if let Some((_, make)) = self.ids.iter().find(|(id, _)| *id == ev.id) {
                    out.push(make());
                }
            }
            out
        }

        /// Keep the check mark of "Speed Limiter" in step with the app.
        pub fn set_limiter(&self, on: bool) {
            if self.limiter.is_checked() != on {
                self.limiter.set_checked(on);
            }
        }
    }
}

/// Linux / BSD: a StatusNotifierItem (the "app indicator" of KDE Plasma, GNOME with
/// the AppIndicator extension, XFCE, Cinnamon, MATE, Budgie, ...). No GTK needed.
/// When the desktop has no tray host, `Tray::new()` is `None` and the program
/// behaves as if there were no icon (closing the window quits).
#[cfg(all(unix, not(target_os = "macos")))]
mod imp {
    use super::TrayAction;
    use ksni::blocking::{Handle, TrayMethods};
    use ksni::menu::{CheckmarkItem, MenuItem, StandardItem};
    use std::cell::Cell;
    use std::sync::{Arc, Mutex};

    type Queue = Arc<Mutex<Vec<TrayAction>>>;

    struct DiosTray {
        queue: Queue,
        limiter: bool,
        icon: Vec<ksni::Icon>,
    }

    impl DiosTray {
        fn push(&self, a: TrayAction) {
            if let Ok(mut q) = self.queue.lock() {
                q.push(a);
            }
        }

        fn entry(label: &str, make: fn() -> TrayAction) -> MenuItem<DiosTray> {
            StandardItem {
                label: label.into(),
                activate: Box::new(move |t: &mut DiosTray| t.push(make())),
                ..Default::default()
            }
            .into()
        }
    }

    impl ksni::Tray for DiosTray {
        fn id(&self) -> String {
            "dios-downloader".into()
        }
        fn title(&self) -> String {
            "DIOS Downloader".into()
        }
        fn icon_pixmap(&self) -> Vec<ksni::Icon> {
            self.icon.clone()
        }
        fn tool_tip(&self) -> ksni::ToolTip {
            ksni::ToolTip { title: "DIOS Downloader".into(), ..Default::default() }
        }
        // a click on the icon shows the window (the menu is on the right button)
        fn activate(&mut self, _x: i32, _y: i32) {
            self.push(TrayAction::Show);
        }
        fn secondary_activate(&mut self, _x: i32, _y: i32) {
            self.push(TrayAction::Show);
        }
        fn menu(&self) -> Vec<MenuItem<Self>> {
            vec![
                Self::entry("Restore DIOS Downloader", || TrayAction::Show),
                MenuItem::Separator,
                Self::entry("Add new download…", || TrayAction::AddUrl),
                Self::entry("Resume all downloads", || TrayAction::ResumeAll),
                Self::entry("Stop all downloads", || TrayAction::StopAll),
                MenuItem::Separator,
                CheckmarkItem {
                    label: "Speed Limiter".into(),
                    checked: self.limiter,
                    activate: Box::new(|t: &mut DiosTray| t.push(TrayAction::ToggleLimiter)),
                    ..Default::default()
                }
                .into(),
                Self::entry("Scheduler…", || TrayAction::Scheduler),
                Self::entry("Options…", || TrayAction::Options),
                MenuItem::Separator,
                Self::entry("About", || TrayAction::About),
                Self::entry("Exit", || TrayAction::Exit),
            ]
        }
    }

    pub struct Tray {
        handle: Handle<DiosTray>,
        queue: Queue,
        limiter: Cell<bool>,
    }

    impl Tray {
        pub fn new() -> Option<Tray> {
            // tray32.rgba is RGBA; the StatusNotifierItem wants ARGB32 (network byte order)
            let rgba = include_bytes!("../ui/tray32.rgba");
            let mut argb = Vec::with_capacity(rgba.len());
            for px in rgba.chunks_exact(4) {
                argb.extend_from_slice(&[px[3], px[0], px[1], px[2]]);
            }
            let queue: Queue = Arc::new(Mutex::new(Vec::new()));
            let tray = DiosTray {
                queue: queue.clone(),
                limiter: false,
                icon: vec![ksni::Icon { width: 32, height: 32, data: argb }],
            };
            // Err = no session bus or no tray host on this desktop
            let handle = tray.spawn().ok()?;
            Some(Tray { handle, queue, limiter: Cell::new(false) })
        }

        /// Clicks since the last call.
        pub fn poll(&self) -> Vec<TrayAction> {
            match self.queue.lock() {
                Ok(mut q) => std::mem::take(&mut *q),
                Err(_) => Vec::new(),
            }
        }

        /// Keep the check mark of "Speed Limiter" in step with the app.
        pub fn set_limiter(&self, on: bool) {
            if self.limiter.get() != on {
                self.limiter.set(on);
                let _ = self.handle.update(|t| t.limiter = on);
            }
        }
    }
}

#[cfg(not(any(windows, all(unix, not(target_os = "macos")))))]
mod imp {
    use super::TrayAction;
    pub struct Tray;
    impl Tray {
        pub fn new() -> Option<Tray> {
            None
        }
        pub fn poll(&self) -> Vec<TrayAction> {
            Vec::new()
        }
        pub fn set_limiter(&self, _on: bool) {}
    }
}

pub use imp::Tray;
