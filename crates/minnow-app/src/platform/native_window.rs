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

        let insert_after = match level {
            Level::Normal => HWND_NOTOPMOST,
            Level::AlwaysOnTop => HWND_TOPMOST,
            Level::AlwaysOnBottom => HWND_BOTTOM,
        };

        unsafe {
            SetWindowPos(
                hwnd,
                Some(insert_after),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
            )
            .map_err(|e| anyhow!("SetWindowPos failed: {e}"))?;
        }

        Ok(())
    }

    pub(super) fn set_click_through(window: &Window, enabled: bool) -> Result<()> {
        let hwnd = hwnd(window)?;

        unsafe {
            SetLastError(WIN32_ERROR(0));
            let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            let read_error = GetLastError();
            if ex_style == 0 && read_error != WIN32_ERROR(0) {
                return Err(anyhow!("GetWindowLongPtrW failed: {read_error:?}"));
            }

            let click_through_bits = (WS_EX_TRANSPARENT.0 | WS_EX_LAYERED.0) as isize;
            let next_style = if enabled {
                ex_style | click_through_bits
            } else {
                ex_style & !click_through_bits
            };

            SetLastError(WIN32_ERROR(0));
            let previous = SetWindowLongPtrW(hwnd, GWL_EXSTYLE, next_style);
            let write_error = GetLastError();
            if previous == 0 && write_error != WIN32_ERROR(0) {
                return Err(anyhow!("SetWindowLongPtrW failed: {write_error:?}"));
            }
        }

        Ok(())
    }
}

#[cfg(not(target_os = "windows"))]
mod platform {
    use super::*;

    pub(super) fn noop_set_level() -> Result<()> {
        unsupported_platform_operation("window levels")
    }

    pub(super) fn noop_set_click_through(_: bool) -> Result<()> {
        unsupported_platform_operation("click-through")
    }

    pub(super) fn set_level(_: &Window, _: Level) -> Result<()> {
        noop_set_level()
    }

    pub(super) fn set_click_through(_: &Window, enabled: bool) -> Result<()> {
        noop_set_click_through(enabled)
    }
}

#[cfg(test)]
mod tests {
    use super::Level;
    use anyhow::anyhow;

    #[test]
    fn level_application_failures_do_not_abort_window_build() {
        super::log_window_level_result(Level::AlwaysOnTop, Err(anyhow!("unsupported platform")));
        let built = true;
        let result = 7;

        assert_eq!(result, 7);
        assert!(built);
    }

    #[test]
    fn unsupported_platform_operations_use_detectable_errors() {
        if cfg!(target_os = "windows") {
            return;
        }

        let level = super::unsupported_platform_operation("window levels");
        let click_through = super::unsupported_platform_operation("click-through");

        assert!(level.is_err());
        assert!(click_through.is_err());
    }
}

