//! Keeps a frameless Windows window's client area inside its window rect.
//!
//! winit gives a borderless window a drop shadow by shifting the client rect down one pixel, so
//! the bottom row hangs below the window, and a maximized window covers an auto-hide taskbar's
//! edge. [`install`] subclasses the window procedure to clamp both. Call it once from the app
//! creator on every platform; it does nothing off Windows.

#[cfg(windows)]
pub use imp::install;

/// Nothing to install off Windows; returns `true`.
#[cfg(not(windows))]
pub fn install(_cc: &eframe::CreationContext<'_>) -> bool {
    true
}

#[cfg(windows)]
mod imp {
    use std::sync::atomic::{AtomicBool, Ordering};

    use ::windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
    use ::windows::Win32::UI::Shell::{
        ABE_BOTTOM, ABE_LEFT, ABE_RIGHT, ABE_TOP, ABM_GETAUTOHIDEBAREX, APPBARDATA,
        DefSubclassProc, RemoveWindowSubclass, SHAppBarMessage, SetWindowSubclass,
    };
    use ::windows::Win32::UI::WindowsAndMessaging::{
        IsZoomed, NCCALCSIZE_PARAMS, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
        SWP_NOZORDER, SetWindowPos, WM_NCCALCSIZE, WM_NCDESTROY,
    };
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    const SUBCLASS_ID: usize = 0x4554_4B42;
    /// Pixels a maximized window leaves at an auto-hide taskbar's edge so the taskbar can be revealed.
    const AUTO_HIDE_TASKBAR_PX: i32 = 2;

    /// Whether the last `WM_NCCALCSIZE` had to clamp the client rect.
    static CLAMPING: AtomicBool = AtomicBool::new(false);

    /// Subclasses the root window so `WM_NCCALCSIZE` never yields a client rect below the window's bottom edge or over an auto-hide taskbar.
    ///
    /// Returns `false` when the window is not a Win32 window or the subclass could not be set.
    pub fn install(cc: &eframe::CreationContext<'_>) -> bool {
        let Ok(handle) = cc.window_handle() else {
            return false;
        };
        let RawWindowHandle::Win32(win32) = handle.as_raw() else {
            return false;
        };
        let hwnd = HWND(win32.hwnd.get() as *mut _);
        // SAFETY: `hwnd` is the live root window, owned by this (the event-loop) thread.
        unsafe {
            if !SetWindowSubclass(hwnd, Some(client_proc), SUBCLASS_ID, 0).as_bool() {
                return false;
            }
            // Recomputes the client rect through the subclass.
            let _ = SetWindowPos(
                hwnd,
                None,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
            );
        }
        true
    }

    /// Clamps the client rect winit computes to the proposed window rect and clears an auto-hide taskbar's edge.
    unsafe extern "system" fn client_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        _id: usize,
        _data: usize,
    ) -> LRESULT {
        match msg {
            WM_NCCALCSIZE if wparam.0 != 0 => {
                let params = lparam.0 as *mut NCCALCSIZE_PARAMS;
                // SAFETY: for WM_NCCALCSIZE with wparam TRUE, lparam points at a valid NCCALCSIZE_PARAMS.
                unsafe {
                    let window_bottom = (*params).rgrc[0].bottom;
                    let result = DefSubclassProc(hwnd, msg, wparam, lparam);
                    let client = &mut (*params).rgrc[0];
                    log_transition(client.bottom - window_bottom);
                    client.bottom = client.bottom.min(window_bottom);
                    if IsZoomed(hwnd).as_bool() {
                        clear_auto_hide_taskbar(client);
                    }
                    result
                }
            }
            WM_NCDESTROY => {
                // SAFETY: removes this subclass from the window it was installed on.
                unsafe {
                    let _ = RemoveWindowSubclass(hwnd, Some(client_proc), SUBCLASS_ID);
                    DefSubclassProc(hwnd, msg, wparam, lparam)
                }
            }
            // SAFETY: forwards every other message unchanged.
            _ => unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) },
        }
    }

    /// Pulls each edge of `client` in by [`AUTO_HIDE_TASKBAR_PX`] where an auto-hide taskbar sits on the monitor `client` fills.
    fn clear_auto_hide_taskbar(client: &mut RECT) {
        for edge in [ABE_LEFT, ABE_TOP, ABE_RIGHT, ABE_BOTTOM] {
            let mut data = APPBARDATA {
                cbSize: std::mem::size_of::<APPBARDATA>() as u32,
                uEdge: edge,
                rc: *client,
                ..Default::default()
            };
            // SAFETY: `data` is a valid APPBARDATA with `cbSize` set.
            let bar = unsafe { SHAppBarMessage(ABM_GETAUTOHIDEBAREX, &mut data) };
            if bar == 0 {
                continue;
            }
            match edge {
                ABE_LEFT => client.left += AUTO_HIDE_TASKBAR_PX,
                ABE_TOP => client.top += AUTO_HIDE_TASKBAR_PX,
                ABE_RIGHT => client.right -= AUTO_HIDE_TASKBAR_PX,
                _ => client.bottom -= AUTO_HIDE_TASKBAR_PX,
            }
        }
    }

    /// Logs when the client rect starts or stops overhanging the window's bottom edge.
    fn log_transition(overhang: i32) {
        let clamping = overhang > 0;
        if CLAMPING.swap(clamping, Ordering::Relaxed) == clamping {
            return;
        }
        if clamping {
            log::info!(
                "client_area: client rect overhung the window bottom by {overhang}px; clamped"
            );
        } else {
            log::info!("client_area: client rect inside the window; no clamp");
        }
    }
}
