//! OS drop target for the notch (files and selected text).
//!
//! Tauri/wry registers its own drop targets on the WebView2 child windows that
//! exist when the webview is built. The notch window is created hidden and
//! WebView2 creates/recreates its render child later, so those targets can be
//! stale — and OLE does not fall back to ancestors, so a drag over the notch
//! finds nothing. This module revokes any existing target and registers ours on
//! the toplevel and every current descendant, after the webview has painted.

use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager};
use windows::core::{implement, Ref, BOOL};
use windows::Win32::Foundation::{HWND, LPARAM, POINT, POINTL};
use windows::Win32::Graphics::Gdi::ScreenToClient;
use windows::Win32::System::Com::{
    CoInitializeEx, IDataObject, COINIT_APARTMENTTHREADED, DVASPECT_CONTENT, FORMATETC,
    TYMED_HGLOBAL,
};
use windows::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};
use windows::Win32::System::Ole::{
    IDropTarget, IDropTarget_Impl, RegisterDragDrop, ReleaseStgMedium, RevokeDragDrop, CF_HDROP,
    CF_UNICODETEXT, DROPEFFECT, DROPEFFECT_COPY, DROPEFFECT_NONE,
};
use windows::Win32::System::SystemServices::MODIFIERKEYS_FLAGS;
use windows::Win32::UI::Shell::{DragFinish, DragQueryFileW, HDROP};
use windows::Win32::UI::WindowsAndMessaging::EnumChildWindows;

/// Upper bound on accepted text drops (~128k UTF-16 units). Anything larger is
/// treated as unsupported rather than truncated.
const MAX_TEXT_UNITS: usize = 1 << 17;

/// Keeps every registered target alive for as long as OLE holds it. The COM
/// pointers are only ever used by OLE, which marshals calls itself.
struct TargetStore(#[allow(dead_code)] Vec<IDropTarget>);
unsafe impl Send for TargetStore {}

static SHELF_DROP_TARGETS: Mutex<Option<TargetStore>> = Mutex::new(None);

/// Set while an external drag is in flight over the notch, so the freshness
/// watchdog never revokes a target out from under an active drop.
static SHELF_DRAG_ACTIVE: AtomicBool = AtomicBool::new(false);

/// The sorted set of HWNDs the drop target is currently registered on. Compared
/// against the live child windows to detect a WebView2 rebuild.
static SHELF_REGISTERED: Mutex<Option<Vec<isize>>> = Mutex::new(None);

/// Guards starting the freshness watchdog exactly once.
static WATCHDOG_STARTED: AtomicBool = AtomicBool::new(false);

/// What a drag carries that we accept.
enum DragPayload {
    Files(Vec<String>, HDROP),
    Text(String),
}

#[implement(IDropTarget)]
struct ShelfDropTarget {
    app: AppHandle,
    hwnd: HWND,
    enter_valid: UnsafeCell<bool>,
    cursor_effect: UnsafeCell<DROPEFFECT>,
}

impl ShelfDropTarget {
    /// Files first (CF_HDROP), then selected text (CF_UNICODETEXT). `None` when
    /// the drag carries neither.
    unsafe fn extract(data: &IDataObject) -> Option<DragPayload> {
        if let Some((paths, hdrop)) = unsafe { Self::extract_files(data) } {
            return Some(DragPayload::Files(paths, hdrop));
        }
        unsafe { Self::extract_text(data) }.map(DragPayload::Text)
    }

    /// Reads CF_HDROP from the drag data. `None` when the drag carries no files.
    unsafe fn extract_files(data: &IDataObject) -> Option<(Vec<String>, HDROP)> {
        let format = FORMATETC {
            cfFormat: CF_HDROP.0,
            ptd: std::ptr::null_mut(),
            dwAspect: DVASPECT_CONTENT.0,
            lindex: -1,
            tymed: TYMED_HGLOBAL.0 as u32,
        };
        let medium = data.GetData(&format).ok()?;
        let hdrop = HDROP(medium.u.hGlobal.0 as _);
        let count = DragQueryFileW(hdrop, 0xFFFFFFFF, None);
        if count == 0 {
            unsafe { DragFinish(hdrop) };
            return None;
        }
        let mut paths = Vec::with_capacity(count as usize);
        for i in 0..count {
            let len = DragQueryFileW(hdrop, i, None) as usize;
            let mut buf = vec![0u16; len + 1];
            DragQueryFileW(hdrop, i, Some(&mut buf));
            paths.push(String::from_utf16_lossy(&buf[..len]));
        }
        Some((paths, hdrop))
    }

    /// Reads CF_UNICODETEXT (selected text dragged from another app). `None`
    /// when absent, empty, or unreasonably large.
    unsafe fn extract_text(data: &IDataObject) -> Option<String> {
        let format = FORMATETC {
            cfFormat: CF_UNICODETEXT.0,
            ptd: std::ptr::null_mut(),
            dwAspect: DVASPECT_CONTENT.0,
            lindex: -1,
            tymed: TYMED_HGLOBAL.0 as u32,
        };
        let mut medium = data.GetData(&format).ok()?;
        let ptr = GlobalLock(medium.u.hGlobal) as *const u16;
        if ptr.is_null() {
            ReleaseStgMedium(&mut medium);
            return None;
        }

        // Bound the scan by the allocation: a malformed data object could
        // offer a buffer without a terminator.
        let alloc_units = (GlobalSize(medium.u.hGlobal) / 2) as usize;
        let max_units = alloc_units.min(MAX_TEXT_UNITS);
        let mut len = 0usize;
        while len < max_units && *ptr.add(len) != 0 {
            len += 1;
        }
        let text = if len >= MAX_TEXT_UNITS || len >= max_units {
            None
        } else {
            Some(String::from_utf16_lossy(std::slice::from_raw_parts(
                ptr, len,
            )))
        };
        let _ = GlobalUnlock(medium.u.hGlobal);
        ReleaseStgMedium(&mut medium);

        match text {
            Some(t) if !t.trim().is_empty() => Some(t),
            _ => None,
        }
    }

    unsafe fn client_point(&self, pt: &POINTL) -> POINT {
        let mut point = POINT { x: pt.x, y: pt.y };
        let _ = ScreenToClient(self.hwnd, &mut point);
        point
    }
}

#[allow(non_snake_case)]
impl IDropTarget_Impl for ShelfDropTarget_Impl {
    fn DragEnter(
        &self,
        pdata: Ref<'_, IDataObject>,
        _keys: MODIFIERKEYS_FLAGS,
        pt: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        // A drag is live over the notch; hold off the freshness watchdog until
        // it resolves so we never revoke a target mid-drag.
        SHELF_DRAG_ACTIVE.store(true, Ordering::Relaxed);
        let payload = pdata
            .as_ref()
            .and_then(|data| unsafe { ShelfDropTarget::extract(data) });
        let Some(payload) = payload else {
            crate::diagnostics::record("shelf_drop", "DragEnter rejected (no files/text)");
            unsafe {
                *self.enter_valid.get() = false;
                *self.cursor_effect.get() = DROPEFFECT_NONE;
                *effect = DROPEFFECT_NONE;
            }
            return Ok(());
        };

        unsafe {
            *self.enter_valid.get() = true;
            *self.cursor_effect.get() = DROPEFFECT_COPY;
            *effect = DROPEFFECT_COPY;
        }

        let point = unsafe { self.client_point(pt) };
        match payload {
            DragPayload::Files(paths, hdrop) => {
                crate::diagnostics::record(
                    "shelf_drop",
                    &format!("DragEnter files={}", paths.len()),
                );
                let _ = self.app.emit(
                    "shelf-drag-enter",
                    serde_json::json!({ "paths": paths, "position": { "x": point.x, "y": point.y } }),
                );
                unsafe { DragFinish(hdrop) };
            }
            DragPayload::Text(text) => {
                crate::diagnostics::record("shelf_drop", "DragEnter text");
                let _ = self.app.emit(
                    "shelf-drag-enter",
                    serde_json::json!({ "text": text, "position": { "x": point.x, "y": point.y } }),
                );
            }
        }
        Ok(())
    }

    fn DragOver(
        &self,
        _keys: MODIFIERKEYS_FLAGS,
        _pt: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        unsafe { *effect = *self.cursor_effect.get() };
        Ok(())
    }

    fn DragLeave(&self) -> windows::core::Result<()> {
        SHELF_DRAG_ACTIVE.store(false, Ordering::Relaxed);
        if unsafe { *self.enter_valid.get() } {
            unsafe { *self.enter_valid.get() = false };
            let _ = self.app.emit("shelf-drag-leave", ());
        }
        Ok(())
    }

    fn Drop(
        &self,
        pdata: Ref<'_, IDataObject>,
        _keys: MODIFIERKEYS_FLAGS,
        _pt: &POINTL,
        _effect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        SHELF_DRAG_ACTIVE.store(false, Ordering::Relaxed);
        if unsafe { *self.enter_valid.get() } {
            unsafe { *self.enter_valid.get() = false };
            let payload = pdata
                .as_ref()
                .and_then(|data| unsafe { ShelfDropTarget::extract(data) });
            match payload {
                Some(DragPayload::Files(paths, hdrop)) => {
                    crate::diagnostics::record(
                        "shelf_drop",
                        &format!("Drop files={}", paths.len()),
                    );
                    let _ = self
                        .app
                        .emit("shelf-drag-drop", serde_json::json!({ "paths": paths }));
                    unsafe { DragFinish(hdrop) };
                }
                Some(DragPayload::Text(text)) => {
                    crate::diagnostics::record("shelf_drop", "Drop text");
                    let _ = self
                        .app
                        .emit("shelf-drag-drop", serde_json::json!({ "text": text }));
                }
                None => {}
            }
        } else {
            crate::diagnostics::record("shelf_drop", "Drop ignored (not entered)");
        }
        Ok(())
    }
}

/// (Re)registers the shelf drop target on the main window and all its current
/// descendants. Safe to call repeatedly; existing registrations are replaced.
/// Also records the set of windows it registered so the freshness watchdog can
/// tell when WebView2 has rebuilt the child windows underneath us.
pub fn setup(app: &AppHandle) {
    let Some(win) = app.get_webview_window("main") else {
        return;
    };
    let Ok(hwnd) = win.hwnd() else {
        return;
    };

    struct Ctx {
        app: AppHandle,
        targets: Vec<IDropTarget>,
        hwnds: Vec<isize>,
    }

    unsafe extern "system" fn child_proc(child: HWND, lparam: LPARAM) -> BOOL {
        let ctx = &mut *(lparam.0 as *mut Ctx);
        unsafe { inject(&ctx.app, child, &mut ctx.targets, &mut ctx.hwnds) };
        BOOL(1)
    }

    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let mut ctx = Ctx {
            app: app.clone(),
            targets: Vec::new(),
            hwnds: Vec::new(),
        };
        inject(app, hwnd, &mut ctx.targets, &mut ctx.hwnds);
        let _ = EnumChildWindows(
            Some(hwnd),
            Some(child_proc),
            LPARAM(&mut ctx as *mut Ctx as isize),
        );
        let registered = ctx.targets.len();
        crate::diagnostics::record("shelf_drop", &format!("registered targets={registered}"));
        let mut hwnds = ctx.hwnds.clone();
        hwnds.sort_unstable();
        if let Ok(mut guard) = SHELF_REGISTERED.lock() {
            *guard = Some(hwnds);
        }
        if let Ok(mut guard) = SHELF_DROP_TARGETS.lock() {
            *guard = Some(TargetStore(ctx.targets));
        }
    }

    start_watchdog(app);
}

unsafe fn inject(
    app: &AppHandle,
    hwnd: HWND,
    targets: &mut Vec<IDropTarget>,
    hwnds: &mut Vec<isize>,
) {
    let target: IDropTarget = ShelfDropTarget {
        app: app.clone(),
        hwnd,
        enter_valid: UnsafeCell::new(false),
        cursor_effect: UnsafeCell::new(DROPEFFECT_NONE),
    }
    .into();
    let _ = unsafe { RevokeDragDrop(hwnd) };
    if unsafe { RegisterDragDrop(hwnd, &target) }.is_ok() {
        targets.push(target);
        hwnds.push(hwnd.0 as isize);
    }
}

/// The main window plus every current descendant HWND, sorted. Compared against
/// the registered set to detect that WebView2 tore down and rebuilt the child
/// windows the drop target lives on.
fn live_target_hwnds(app: &AppHandle) -> Option<Vec<isize>> {
    let win = app.get_webview_window("main")?;
    let hwnd = win.hwnd().ok()?;
    let mut list: Vec<isize> = vec![hwnd.0 as isize];

    unsafe extern "system" fn collect(child: HWND, lparam: LPARAM) -> BOOL {
        let list = &mut *(lparam.0 as *mut Vec<isize>);
        list.push(child.0 as isize);
        BOOL(1)
    }

    unsafe {
        let _ = EnumChildWindows(
            Some(hwnd),
            Some(collect),
            LPARAM(&mut list as *mut Vec<isize> as isize),
        );
    }
    list.sort_unstable();
    Some(list)
}

/// Re-registers the drop target only when the live child windows differ from
/// the set we last registered on. Skipped while a drag is in flight so an
/// active drop is never interrupted by a revoke/re-register cycle.
pub fn ensure_fresh(app: &AppHandle) {
    if SHELF_DRAG_ACTIVE.load(Ordering::Relaxed) {
        return;
    }
    let Some(live) = live_target_hwnds(app) else {
        return;
    };
    let registered = SHELF_REGISTERED.lock().ok().and_then(|g| g.clone());
    if registered.as_deref() == Some(live.as_slice()) {
        return;
    }
    crate::diagnostics::record("shelf_drop", "child windows changed, re-registering");
    setup(app);
}

/// Periodic freshness check. WebView2 recreates its render child on sleep/wake,
/// DPI changes, and webview reloads; without this the target silently goes
/// stale on a long-running notch and drops stop landing. Started once, on the
/// first setup, and always marshalled onto the window's own (main) thread.
fn start_watchdog(app: &AppHandle) {
    if WATCHDOG_STARTED.swap(true, Ordering::Relaxed) {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_secs(4));
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || ensure_fresh(&handle));
    });
}
