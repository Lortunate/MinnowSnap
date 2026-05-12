use anyhow::{Result, anyhow};
use gpui::{App, Window};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Level {
    #[default]
    Normal,
    AlwaysOnTop,
    AlwaysOnBottom,
}

pub trait WindowLevelExt {
    fn set_level(&mut self, level: Level) -> Result<()>;
    fn set_click_through(&mut self, enabled: bool) -> Result<()>;
}

impl WindowLevelExt for Window {
    fn set_level(&mut self, level: Level) -> Result<()> {
        platform::set_level(self, level)
    }

    fn set_click_through(&mut self, enabled: bool) -> Result<()> {
        platform::set_click_through(self, enabled)
    }
}

#[cfg_attr(target_os = "windows", allow(dead_code))]
fn unsupported_platform_operation(operation: &str) -> Result<()> {
    Err(anyhow!("{operation} are not implemented for this platform"))
}

fn log_window_level_result(_level: Level, result: Result<()>) {
    if let Err(err) = result {
        tracing::warn!("Failed to apply native window level: {err}");
    }
}

/// cx.open_window(opts, with_level(Level::AlwaysOnTop, |window, cx| { ... }))
pub fn with_level<T>(level: Level, build: impl FnOnce(&mut Window, &mut App) -> T) -> impl FnOnce(&mut Window, &mut App) -> T {
    move |window, cx| {
        log_window_level_result(level, window.set_level(level));
        build(window, cx)
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use super::*;
    use windows::Win32::Foundation::{GetLastError, HWND, SetLastError, WIN32_ERROR};
    use windows::Win32::UI::WindowsAndMessaging::{
        GWL_EXSTYLE, GetWindowLongPtrW, HWND_BOTTOM, HWND_NOTOPMOST, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW,
        SetWindowLongPtrW, SetWindowPos, WS_EX_LAYERED, WS_EX_TRANSPARENT,
    };

    fn hwnd(window: &Window) -> Result<HWND> {
        let raw = HasWindowHandle::window_handle(window)
            .map_err(|e| anyhow!("failed to get native window handle: {e}"))?
            .as_raw();

        match raw {
            RawWindowHandle::Win32(h) => Ok(HWND(h.hwnd.get() as *mut _)),
            other => Err(anyhow!("expected Win32 handle, got {other:?}")),
        }
    }

    pub(super) fn set_level(window: &Window, level: Level) -> Result<()> {
        let hwnd = hwnd(window)?;

        unsafe {
            let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
            let desired_style = match level {
                Level::Normal => ex_style & !WS_EX_LAYERED.0 & !WS_EX_TRANSPARENT.0,
                Level::AlwaysOnTop => ex_style | WS_EX_LAYERED.0 | WS_EX_TRANSPARENT.0,
                Level::AlwaysOnBottom => ex_style | WS_EX_LAYERED.0,
            };

            if desired_style != ex_style {
                SetLastError(WIN32_ERROR(0));
                let _ = SetWindowLongPtrW(hwnd, GWL_EXSTYLE, desired_style as isize);
                let last_error = GetLastError();
                if last_error != WIN32_ERROR(0) {
                    return Err(anyhow!("SetWindowLongPtrW failed: {last_error:?}"));
                }
            }

            let (insert_after, flags) = match level {
                Level::Normal => (HWND_NOTOPMOST, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW),
                Level::AlwaysOnTop => (HWND_TOPMOST, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW),
                Level::AlwaysOnBottom => (HWND_BOTTOM, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW),
            };

            SetWindowPos(hwnd, Some(insert_after), 0, 0, 0, 0, flags).map_err(|e| anyhow!("SetWindowPos failed: {e}"))?;
        }

        Ok(())
    }

    pub(super) fn set_click_through(window: &Window, enabled: bool) -> Result<()> {
        let hwnd = hwnd(window)?;

        unsafe {
            let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
            let mut desired_style = ex_style | WS_EX_LAYERED.0;
            if enabled {
                desired_style |= WS_EX_TRANSPARENT.0;
            } else {
                desired_style &= !WS_EX_TRANSPARENT.0;
            }

            if desired_style != ex_style {
                SetLastError(WIN32_ERROR(0));
                let _ = SetWindowLongPtrW(hwnd, GWL_EXSTYLE, desired_style as isize);
                let last_error = GetLastError();
                if last_error != WIN32_ERROR(0) {
                    return Err(anyhow!("SetWindowLongPtrW failed: {last_error:?}"));
                }
            }
        }

        Ok(())
    }
}

#[cfg(not(target_os = "windows"))]
mod platform {
    use super::*;

    pub(super) fn set_level(_window: &Window, level: Level) -> Result<()> {
        if matches!(level, Level::Normal) {
            Ok(())
        } else {
            unsupported_platform_operation("window levels")
        }
    }

    pub(super) fn set_click_through(_window: &Window, enabled: bool) -> Result<()> {
        if !enabled {
            Ok(())
        } else {
            unsupported_platform_operation("click-through")
        }
    }
}
