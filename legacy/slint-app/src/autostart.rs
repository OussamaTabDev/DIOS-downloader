//! "Start with Windows": a value under HKCU\...\Run that starts the app in
//! the background (next to the clock) when the user signs in.

#[cfg(windows)]
mod imp {
    // The registry is read and written directly: starting reg.exe took a
    // second or more on some computers (antivirus) every time Options opened.
    use windows_sys::Win32::System::Registry::{
        RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ,
    };

    const KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const NAME: &str = "DIOSDownloader";

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn exists(key: &str, name: &str) -> bool {
        let (key, name) = (wide(key), wide(name));
        let mut size: u32 = 0;
        // SAFETY: NUL-terminated wide strings; only the size of the value is asked for
        unsafe {
            RegGetValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr(), RRF_RT_REG_SZ, std::ptr::null_mut(), std::ptr::null_mut(), &mut size) == 0
        }
    }

    pub fn is_enabled() -> bool {
        exists(KEY, NAME)
    }

    /// Installed with the setup program (which asked about autostart itself).
    pub fn installed_by_setup() -> bool {
        exists(r"Software\DIOS\DIOSDownloader", "Installed")
    }

    pub fn set(enabled: bool) -> bool {
        let (key, name) = (wide(KEY), wide(NAME));
        if enabled {
            let Ok(exe) = std::env::current_exe() else { return false };
            let value = wide(&format!("\"{}\" --background", exe.display()));
            // SAFETY: the value is a NUL-terminated wide string, its size in bytes is passed
            unsafe {
                RegSetKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr(), REG_SZ, value.as_ptr().cast(), (value.len() * 2) as u32) == 0
            }
        } else {
            // SAFETY: NUL-terminated wide strings
            !is_enabled() || unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr()) == 0 }
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
