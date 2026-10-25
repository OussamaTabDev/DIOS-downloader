//! "Start with Windows": a value under HKCU\...\Run that starts the app in
//! the background (next to the clock) when the user signs in.

#[cfg(windows)]
mod imp {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    const KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
    const NAME: &str = "DIOSDownloader";
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    fn reg(args: &[&str]) -> bool {
        Command::new(dios_engine::system::windows_tool("reg.exe"))
            .args(args)
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    pub fn is_enabled() -> bool {
        reg(&["query", KEY, "/v", NAME])
    }

    /// Installed with the setup program (which asked about autostart itself).
    pub fn installed_by_setup() -> bool {
        reg(&["query", r"HKCU\Software\DIOS\DIOSDownloader", "/v", "Installed"])
    }

    pub fn set(enabled: bool) -> bool {
        if enabled {
            let Ok(exe) = std::env::current_exe() else { return false };
            let value = format!("\"{}\" --background", exe.display());
            reg(&["add", KEY, "/v", NAME, "/t", "REG_SZ", "/d", &value, "/f"])
        } else {
            !is_enabled() || reg(&["delete", KEY, "/v", NAME, "/f"])
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use std::fs;
    use std::path::PathBuf;

    const FILE: &str = "dios-downloader.desktop";

    fn desktop_path() -> Option<PathBuf> {
        let dir = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| std::env::home_dir().map(|h| h.join(".config")))?;
        Some(dir.join("autostart"))
    }

    fn desktop_entry() -> Option<String> {
        let exe = std::env::current_exe().ok()?;
        Some(format!(
            "[Desktop Entry]\nType=Application\nVersion=1.0\nName=DIOS Downloader\nComment=DIOS Downloader in the background\nExec=\"{}\" --background\nTerminal=false\nX-GNOME-Autostart-enabled=true\n",
            exe.display()
        ))
    }

    /// "Start with Windows" == XDG autostart on Linux: a .desktop file the
    /// session manager runs at sign-in (GNOME, KDE, XFCE, ...).
    pub fn is_enabled() -> bool {
        desktop_path().map(|p| p.join(FILE).exists()).unwrap_or(false)
    }

    pub fn installed_by_setup() -> bool {
        false
    }

    pub fn set(enabled: bool) -> bool {
        let Some(dir) = desktop_path() else { return false };
        if enabled {
            let Some(entry) = desktop_entry() else { return false };
            if fs::create_dir_all(&dir).is_err() {
                return false;
            }
            fs::write(dir.join(FILE), entry).is_ok()
        } else {
            match fs::remove_file(dir.join(FILE)) {
                Ok(()) => true,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => true,
                Err(_) => false,
            }
        }
    }
}

pub use imp::{installed_by_setup, is_enabled, set};
