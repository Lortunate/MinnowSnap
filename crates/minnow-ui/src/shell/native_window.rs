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

/// cx.open_window(opts, with_level(Level::AlwaysOnTop, |window, cx| { ... }))
pub fn with_level<T>(level: Level, build: impl FnOnce(&mut Window, &mut App) -> T) -> impl FnOnce(&mut Window, &mut App) -> T {
    move |window, cx| {
        window.set_level(level).expect("failed to apply native window level");
        build(window, cx)
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use super::*;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        GWL_EXSTYLE, GetWindowLongPtrW, HWND_BOTTOM, HWND_NOTOPMOST, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW,
        SetWindowLongPtrW, SetWindowPos, WS_EX_LAYERED, WS_EX_TRANSPARENT,
    };

    pub(super) fn set_level(window: &Window, level: Level) -> Result<()> {
        let raw = HasWindowHandle::window_handle(window)
            .map_err(|e| anyhow!("failed to get native window handle: {e}"))?
            .as_raw();

        let hwnd = match raw {
            RawWindowHandle::Win32(h) => HWND(h.hwnd.get() as *mut _),
            other => return Err(anyhow!("expected Win32 handle, got {other:?}")),
        };

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
        let raw = HasWindowHandle::window_handle(window)
            .map_err(|e| anyhow!("failed to get native window handle: {e}"))?
            .as_raw();

        let hwnd = match raw {
            RawWindowHandle::Win32(h) => HWND(h.hwnd.get() as *mut _),
            other => return Err(anyhow!("expected Win32 handle, got {other:?}")),
        };

        unsafe {
            let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            if ex_style == 0 {
                return Err(anyhow!("GetWindowLongPtrW failed"));
            }

            let click_through_bits = (WS_EX_TRANSPARENT.0 | WS_EX_LAYERED.0) as isize;
            let next_style = if enabled {
                ex_style | click_through_bits
            } else {
                ex_style & !click_through_bits
            };

            let previous = SetWindowLongPtrW(hwnd, GWL_EXSTYLE, next_style);
            if previous == 0 {
                return Err(anyhow!("SetWindowLongPtrW failed"));
            }
        }

        Ok(())
    }
}

#[cfg(not(target_os = "windows"))]
mod platform {
    use super::*;

    pub(super) fn set_level(_: &Window, _: Level) -> Result<()> {
        Err(anyhow!("window levels are only implemented for macOS and Windows"))
    }

    pub(super) fn set_click_through(_: &Window, _: bool) -> Result<()> {
        Err(anyhow!("click-through is only implemented for Windows"))
    }
}
