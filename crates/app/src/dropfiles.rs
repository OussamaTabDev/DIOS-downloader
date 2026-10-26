//! Files or links dragged from outside the program onto the main window.
//!
//! Slint's window (winit) knows nothing about OS drag & drop, so this is
//! done with Windows itself: an OLE `IDropTarget` is registered on the main
//! window (`RegisterDragDrop`), like classic download managers do. While a
//! drag is over the window the UI shows its "drop here" veil; on release
//! the dropped files / text go to the same places the Paste button uses.
//!
//! The OLE calls arrive on the UI thread (the thread that runs the message
//! loop), but in the middle of message dispatch — so the real work is put
//! in a small tray and done a moment later from the event loop, the same
//! way the browser-extension server hands its events over.

#![allow(clippy::upper_case_acronyms)]

/// What a drop (or a hover) wants the UI to do.
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)] // on Linux the platform part is a stub, so Text is never built
pub(crate) enum DropAction {
    /// a drag entered (true) or left (false) the window
    Hover(bool),
    /// files were dropped
    Files(Vec<std::path::PathBuf>),
    /// text (a link, a magnet…) was dropped
    Text(String),
}

#[cfg(windows)]
mod imp {
    use super::DropAction;
    use std::cell::{Cell, RefCell};
    use std::ffi::c_void;
    use std::path::PathBuf;
    use std::sync::atomic::AtomicU32;

    use windows_sys::core::GUID;
    use windows_sys::Win32::Foundation::{HGLOBAL, POINTL};
    use windows_sys::Win32::System::Com::{FORMATETC, STGMEDIUM, DVASPECT_CONTENT};
    use windows_sys::Win32::System::Memory::{GlobalLock, GlobalUnlock};
    use windows_sys::Win32::System::Ole::{
        RegisterDragDrop, ReleaseStgMedium, CF_HDROP, CF_UNICODETEXT, DROPEFFECT_COPY, DROPEFFECT_NONE,
    };
    use windows_sys::Win32::UI::Shell::DragQueryFileW;

    // standard values (objidl.h); TYMED_HGLOBAL is an i32 in windows-sys,
    // the structures want it as u32
    const TYMED_HGLOBAL: u32 = 1;

    /// GUID by hand (windows-sys's GUID has no ==)
    fn guid_eq(a: &GUID, b: &GUID) -> bool {
        a.data1 == b.data1 && a.data2 == b.data2 && a.data3 == b.data3 && a.data4 == b.data4
    }

    // ---------------------------------------------------------------- data object

    /// The `IDataObject` vtable, first half (IUnknown + the two calls used).
    #[repr(C)]
    struct DataObjectVtbl {
        _query_interface: usize,
        _add_ref: usize,
        _release: usize,
        get_data: unsafe extern "system" fn(*mut c_void, *const FORMATETC, *mut STGMEDIUM) -> i32,
        _get_data_here: usize,
        query_get_data: unsafe extern "system" fn(*mut c_void, *const FORMATETC) -> i32,
        _rest: [usize; 6],
    }

    /// `GetData(CF_HDROP)` / `GetData(CF_UNICODETEXT)` of the dragged data.
    unsafe fn fetch(obj: *mut c_void, cf: u16, medium: *mut STGMEDIUM) -> bool {
        let vt = *(obj as *const *const DataObjectVtbl);
        let fmt = FORMATETC { cfFormat: cf, ptd: std::ptr::null_mut(), dwAspect: DVASPECT_CONTENT, lindex: -1, tymed: TYMED_HGLOBAL };
        ((*vt).get_data)(obj, &fmt, medium) >= 0 && !(*medium).u.hGlobal.is_null()
    }

    /// Does the dragged data hold this format at all?
    unsafe fn has_format(obj: *mut c_void, cf: u16) -> bool {
        let vt = *(obj as *const *const DataObjectVtbl);
        let fmt = FORMATETC { cfFormat: cf, ptd: std::ptr::null_mut(), dwAspect: DVASPECT_CONTENT, lindex: -1, tymed: TYMED_HGLOBAL };
        ((*vt).query_get_data)(obj, &fmt) >= 0
    }

    /// The file paths inside a dropped `CF_HDROP`.
    unsafe fn files_of(obj: *mut c_void) -> Vec<PathBuf> {
        let mut out = Vec::new();
        let mut medium = STGMEDIUM {
            tymed: TYMED_HGLOBAL,
            u: windows_sys::Win32::System::Com::STGMEDIUM_0 { hGlobal: std::ptr::null_mut() },
            pUnkForRelease: std::ptr::null_mut(),
        };
        if !fetch(obj, CF_HDROP, &mut medium) {
            return out;
        }
        let hdrop = medium.u.hGlobal as HGLOBAL;
        // how many files came with the drop
        let count = DragQueryFileW(hdrop as _, 0xFFFF_FFFF, std::ptr::null_mut(), 0);
        for i in 0..count {
            let len = DragQueryFileW(hdrop as _, i, std::ptr::null_mut(), 0);
            if len == 0 {
                continue;
            }
            let mut buf = vec![0u16; len as usize + 1];
            let n = DragQueryFileW(hdrop as _, i, buf.as_mut_ptr(), buf.len() as u32);
            buf.truncate(n as usize);
            if let Ok(s) = String::from_utf16(&buf) {
                out.push(PathBuf::from(s));
            }
        }
        ReleaseStgMedium(&mut medium);
        out
    }

    /// The text inside a dropped `CF_UNICODETEXT`.
    unsafe fn text_of(obj: *mut c_void) -> String {
        let mut medium = STGMEDIUM {
            tymed: TYMED_HGLOBAL,
            u: windows_sys::Win32::System::Com::STGMEDIUM_0 { hGlobal: std::ptr::null_mut() },
            pUnkForRelease: std::ptr::null_mut(),
        };
        if !fetch(obj, CF_UNICODETEXT, &mut medium) {
            return String::new();
        }
        let hglobal = medium.u.hGlobal as HGLOBAL;
        let mut out = String::new();
        // SAFETY: hGlobal comes from OLE with the HGLOBAL medium type
        let p = GlobalLock(hglobal);
        if !p.is_null() {
            let mut wide: Vec<u16> = Vec::new();
            let mut i = 0usize;
            loop {
                let c = *(p as *const u16).add(i);
                if c == 0 {
                    break;
                }
                wide.push(c);
                i += 1;
                if i > 64 * 1024 {
                    break; // a drop is one link or a short list, never a book
                }
            }
            GlobalUnlock(hglobal);
            out = String::from_utf16_lossy(&wide);
        }
        ReleaseStgMedium(&mut medium);
        out
    }

    // ---------------------------------------------------------------- drop target

    /// The `IDropTarget` vtable (IUnknown + DragEnter/Over/Leave/Drop).
    #[repr(C)]
    struct DropTargetVtbl {
        query_interface: unsafe extern "system" fn(*mut DropTarget, *const GUID, *mut *mut c_void) -> i32,
        add_ref: unsafe extern "system" fn(*mut DropTarget) -> u32,
        release: unsafe extern "system" fn(*mut DropTarget) -> u32,
        drag_enter: unsafe extern "system" fn(*mut DropTarget, *mut c_void, u32, POINTL, *mut u32) -> i32,
        drag_over: unsafe extern "system" fn(*mut DropTarget, u32, POINTL, *mut u32) -> i32,
        drag_leave: unsafe extern "system" fn(*mut DropTarget) -> i32,
        drop: unsafe extern "system" fn(*mut DropTarget, *mut c_void, u32, POINTL, *mut u32) -> i32,
    }

    struct DropTarget {
        vtbl: &'static DropTargetVtbl,
        refs: AtomicU32,
    }

    const IID_IUNKNOWN: GUID = GUID::from_u128(0x00000000_0000_0000_c000_000000000046);
    const E_NOINTERFACE: i32 = 0x8000_4002u32 as i32; // 0x80004002
    const E_POINTER: i32 = 0x8000_4003u32 as i32;

    unsafe extern "system" fn qi(this: *mut DropTarget, iid: *const GUID, out: *mut *mut c_void) -> i32 {
        if out.is_null() || iid.is_null() {
            return E_POINTER;
        }
        *out = std::ptr::null_mut();
        if guid_eq(&*iid, &IID_IUNKNOWN) {
            *out = this as *mut c_void;
            ((*(*this).vtbl).add_ref)(this);
            return 0;
        }
        E_NOINTERFACE
    }
    unsafe extern "system" fn add_ref(_this: *mut DropTarget) -> u32 {
        // the target lives as long as the program does; the count is a formality
        1
    }
    unsafe extern "system" fn release(_this: *mut DropTarget) -> u32 {
        1
    }

    unsafe extern "system" fn drag_enter(_this: *mut DropTarget, obj: *mut c_void, _keys: u32, _pt: POINTL, effect: *mut u32) -> i32 {
        let take = !obj.is_null() && (has_format(obj, CF_HDROP) || has_format(obj, CF_UNICODETEXT));
        ACCEPTED.with(|a| a.set(take));
        *effect = if take { DROPEFFECT_COPY } else { DROPEFFECT_NONE };
        if take {
            hover(true);
        }
        0
    }

    unsafe extern "system" fn drag_over(_this: *mut DropTarget, _keys: u32, _pt: POINTL, effect: *mut u32) -> i32 {
        let take = ACCEPTED.with(|a| a.get());
        *effect = if take { DROPEFFECT_COPY } else { DROPEFFECT_NONE };
        if take {
            hover(true);
        }
        0
    }

    unsafe extern "system" fn drag_leave(_this: *mut DropTarget) -> i32 {
        ACCEPTED.with(|a| a.set(false));
        hover(false);
        0
    }

    unsafe extern "system" fn drop(_this: *mut DropTarget, obj: *mut c_void, _keys: u32, _pt: POINTL, effect: *mut u32) -> i32 {
        *effect = DROPEFFECT_COPY;
        let take = ACCEPTED.with(|a| a.replace(false));
        hover(false);
        if obj.is_null() || !take {
            return 0;
        }
        let files = files_of(obj);
        let text = if files.is_empty() { text_of(obj) } else { String::new() };
        put(DropAction::Files(files));
        put(DropAction::Text(text));
        0
    }

    static VT: DropTargetVtbl = DropTargetVtbl {
        query_interface: qi,
        add_ref,
        release,
        drag_enter,
        drag_over,
        drag_leave,
        drop,
    };
    static TARGET: DropTarget = DropTarget { vtbl: &VT, refs: AtomicU32::new(1) };

    // ---------------------------------------------------------------- to the event loop

    thread_local! {
        /// actions waiting for the event loop (drop events land in the middle
        /// of message dispatch; the UI work waits for a calm moment)
        static QUEUE: RefCell<Vec<DropAction>> = const { RefCell::new(Vec::new()) };
        /// what the UI was last told about the hover veil
        static SHOWN: Cell<bool> = const { Cell::new(false) };
        /// the drag on the window carries something we can use
        static ACCEPTED: Cell<bool> = const { Cell::new(false) };
        /// the Ui's answer (set once, from the UI thread)
        static HANDLER: RefCell<Option<std::rc::Rc<dyn Fn(DropAction)>>> = const { RefCell::new(None) };
        /// the main window's handle (set once, from the UI thread)
        static HWND: Cell<isize> = const { Cell::new(0) };
        /// install() already ran
        static DONE: Cell<bool> = const { Cell::new(false) };
    }

    fn hover(on: bool) {
        // only a change is worth waking the event loop for (DragOver comes
        // many times a second)
        if SHOWN.with(|s| s.replace(on)) == on {
            return;
        }
        put(DropAction::Hover(on));
    }

    fn put(action: DropAction) {
        QUEUE.with(|q| q.borrow_mut().push(action));
        let _ = slint::invoke_from_event_loop(|| apply());
    }

    /// The event loop side: hand everything collected to the UI.
    fn apply() {
        let Some(f) = HANDLER.with(|h| h.borrow().clone()) else { return };
        for action in QUEUE.with(|q| std::mem::take(&mut *q.borrow_mut())) {
            // an empty text after files means "the files were the message"
            if let DropAction::Text(t) = &action {
                if t.is_empty() {
                    continue;
                }
            }
            f(action);
        }
    }

    /// Remember what to do with a drop (called once, before the window shows).
    pub(crate) fn set_handler(f: std::rc::Rc<dyn Fn(DropAction)>) {
        HANDLER.with(|h| *h.borrow_mut() = Some(f));
    }

    /// Accept drops on the main window (called when it is shown; later calls
    /// change nothing).
    pub(crate) fn register_now() {
        if DONE.with(|d| d.get()) {
            return;
        }
        let hwnd = HWND.with(|h| h.get());
        if hwnd == 0 {
            return;
        }
        // SAFETY: hwnd is the live main window; TARGET is a static COM object
        // that stays valid for the life of the program
        unsafe {
            let _ = windows_sys::Win32::System::Ole::OleInitialize(std::ptr::null_mut());
            let hr = RegisterDragDrop(hwnd as _, &TARGET as *const DropTarget as *mut c_void);
            if hr >= 0 {
                DONE.with(|d| d.set(true));
            }
        }
    }

    /// Remember the window's handle (the caller knows it from the shown window).
    pub(crate) fn set_hwnd(hwnd: isize) {
        HWND.with(|h| h.set(hwnd));
    }
}

#[cfg(windows)]
pub(crate) use imp::{register_now as register_main, set_handler, set_hwnd};

#[cfg(not(windows))]
mod imp {
    use super::DropAction;
    /// Linux/BSD: OS drag & drop is not wired up yet (X11's XDND would need
    /// its own protocol implementation); paste and the clipboard monitor
    /// cover adding downloads there.
    pub(crate) fn set_handler(_f: std::rc::Rc<dyn Fn(DropAction)>) {}
    pub(crate) fn register_main() {}
    pub(crate) fn set_hwnd(_hwnd: isize) {}
}

#[cfg(not(windows))]
pub(crate) use imp::{register_main, set_handler, set_hwnd};
