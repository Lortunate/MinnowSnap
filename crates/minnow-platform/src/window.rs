use minnow_application::ports::{ErrorKind, OperationError, OperationResult};
use raw_window_handle::WindowHandle;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WindowCapabilities {
    pub always_on_top: bool,
    pub click_through: bool,
    pub system_drag: bool,
}

pub const fn capabilities() -> WindowCapabilities {
    WindowCapabilities {
        always_on_top: cfg!(any(target_os = "windows", target_os = "macos")),
        click_through: cfg!(any(target_os = "windows", target_os = "macos")),
        system_drag: cfg!(target_os = "windows"),
    }
}

/// The borrowed handle keeps native access tied to a live host window.
pub fn set_always_on_top(handle: WindowHandle<'_>, enabled: bool) -> OperationResult<()> {
    platform::set_always_on_top(handle, enabled)
}

pub fn set_click_through(handle: WindowHandle<'_>, enabled: bool) -> OperationResult<()> {
    platform::set_click_through(handle, enabled)
}

pub fn start_system_drag(handle: WindowHandle<'_>) -> OperationResult<()> {
    platform::start_system_drag(handle)
}

#[cfg(not(target_os = "windows"))]
fn unsupported(operation: &str) -> OperationError {
    OperationError::new(ErrorKind::Unavailable, format!("{operation} is not supported by this platform"))
}

#[cfg(target_os = "windows")]
mod platform {
    use super::*;
    use raw_window_handle::RawWindowHandle;
    use windows::Win32::Foundation::{GetLastError, HWND, LPARAM, SetLastError, WIN32_ERROR, WPARAM};
    use windows::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture;
    use windows::Win32::UI::WindowsAndMessaging::{
        GWL_EXSTYLE, GetWindowLongPtrW, HTCAPTION, HWND_NOTOPMOST, HWND_TOPMOST, PostMessageW, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
        SetWindowLongPtrW, SetWindowPos, WM_NCLBUTTONDOWN, WS_EX_LAYERED, WS_EX_TRANSPARENT,
    };

    fn hwnd(handle: WindowHandle<'_>) -> OperationResult<HWND> {
        match handle.as_raw() {
            RawWindowHandle::Win32(handle) => Ok(HWND(handle.hwnd.get() as *mut _)),
            _ => Err(OperationError::new(ErrorKind::InvalidInput, "expected a Win32 window handle")),
        }
    }

    pub(super) fn set_always_on_top(handle: WindowHandle<'_>, enabled: bool) -> OperationResult<()> {
        let window = hwnd(handle)?;
        unsafe {
            SetWindowPos(
                window,
                Some(if enabled { HWND_TOPMOST } else { HWND_NOTOPMOST }),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            )
            .map_err(|error| native_error("set window level", error))
        }
    }

    pub(super) fn set_click_through(handle: WindowHandle<'_>, enabled: bool) -> OperationResult<()> {
        let window = hwnd(handle)?;
        unsafe {
            SetLastError(WIN32_ERROR(0));
            let current = GetWindowLongPtrW(window, GWL_EXSTYLE);
            let last_error = GetLastError();
            if current == 0 && last_error != WIN32_ERROR(0) {
                return Err(native_error("read window style", last_error));
            }
            let next = if enabled {
                (current as u32 | WS_EX_LAYERED.0 | WS_EX_TRANSPARENT.0) as isize
            } else {
                (current as u32 & !WS_EX_TRANSPARENT.0) as isize
            };
            if current != next {
                SetLastError(WIN32_ERROR(0));
                let previous = SetWindowLongPtrW(window, GWL_EXSTYLE, next);
                let last_error = GetLastError();
                if previous == 0 && last_error != WIN32_ERROR(0) {
                    return Err(native_error("set click-through style", last_error));
                }
            }
        }
        Ok(())
    }

    pub(super) fn start_system_drag(handle: WindowHandle<'_>) -> OperationResult<()> {
        let window = hwnd(handle)?;
        unsafe {
            // No capture is also a valid starting state for a titlebar drag.
            let _ = ReleaseCapture();
            PostMessageW(Some(window), WM_NCLBUTTONDOWN, WPARAM(HTCAPTION as usize), LPARAM(0))
                .map_err(|error| native_error("begin native window drag", error))
        }
    }

    fn native_error(operation: &str, error: impl std::fmt::Display) -> OperationError {
        OperationError::new(ErrorKind::Native, format!("failed to {operation}: {error}"))
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use super::*;
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSFloatingWindowLevel, NSNormalWindowLevel, NSView, NSWindow};
    use raw_window_handle::RawWindowHandle;

    fn with_window<R>(handle: WindowHandle<'_>, operation: impl FnOnce(&NSWindow) -> R) -> OperationResult<R> {
        let _main_thread =
            MainThreadMarker::new().ok_or_else(|| OperationError::new(ErrorKind::Native, "native window operations must run on the main thread"))?;
        let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
            return Err(OperationError::new(ErrorKind::InvalidInput, "expected an AppKit window handle"));
        };
        // WindowHandle guarantees that the native object remains live for this
        // borrow. GPUI supplies an NSView, from which we retain its NSWindow.
        let view = unsafe { handle.ns_view.cast::<NSView>().as_ref() };
        let window = view
            .window()
            .ok_or_else(|| OperationError::new(ErrorKind::Unavailable, "the native view is not attached to a window"))?;
        Ok(operation(&window))
    }

    pub(super) fn set_always_on_top(handle: WindowHandle<'_>, enabled: bool) -> OperationResult<()> {
        with_window(handle, |window| {
            window.setLevel(if enabled { NSFloatingWindowLevel } else { NSNormalWindowLevel });
        })
    }

    pub(super) fn set_click_through(handle: WindowHandle<'_>, enabled: bool) -> OperationResult<()> {
        with_window(handle, |window| {
            window.setIgnoresMouseEvents(enabled);
        })
    }

    pub(super) fn start_system_drag(_handle: WindowHandle<'_>) -> OperationResult<()> {
        Err(unsupported("raw-handle system drag; use the host's native drag region"))
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
mod platform {
    use super::*;

    pub(super) fn set_always_on_top(_handle: WindowHandle<'_>, _enabled: bool) -> OperationResult<()> {
        Err(unsupported("window levels"))
    }

    pub(super) fn set_click_through(_handle: WindowHandle<'_>, _enabled: bool) -> OperationResult<()> {
        Err(unsupported("click-through"))
    }

    pub(super) fn start_system_drag(_handle: WindowHandle<'_>) -> OperationResult<()> {
        Err(unsupported("raw-handle system drag"))
    }
}
