use anyhow::{Result, anyhow};
use gpui::{Div, InteractiveElement, MouseButton, Window, WindowControlArea};

pub trait WindowDragExt {
    fn start_system_drag(&mut self) -> Result<()>;
}

impl WindowDragExt for Window {
    fn start_system_drag(&mut self) -> Result<()> {
        platform::start_system_drag(self)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PopupDragBehavior {
    // Use GPUI's native hit-test region for windows that already behave correctly with it.
    HitTest,
    // Use a posted non-client drag message on Windows for popup headers that need explicit system move.
    SystemMove,
}

pub trait PopupDragRegionExt {
    fn popup_drag_region(self, behavior: PopupDragBehavior) -> Self;
}

impl PopupDragRegionExt for Div {
    fn popup_drag_region(self, behavior: PopupDragBehavior) -> Self {
        match behavior {
            PopupDragBehavior::HitTest => self.window_control_area(WindowControlArea::Drag),
            PopupDragBehavior::SystemMove => {
                #[cfg(target_os = "windows")]
                {
                    self.on_mouse_down(MouseButton::Left, |_, window, _| {
                        if let Err(err) = window.start_system_drag() {
                            tracing::debug!("failed to start popup drag: {err}");
                        }
                    })
                }

                #[cfg(not(target_os = "windows"))]
                {
                    self.window_control_area(WindowControlArea::Drag)
                }
            }
        }
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use super::*;
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
    use windows::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture;
    use windows::Win32::UI::WindowsAndMessaging::{HTCAPTION, PostMessageW, WM_NCLBUTTONDOWN};

    fn hwnd(window: &Window) -> Result<HWND> {
        let raw = HasWindowHandle::window_handle(window)
            .map_err(|e| anyhow!("failed to get native window handle: {e}"))?
            .as_raw();

        match raw {
            RawWindowHandle::Win32(h) => Ok(HWND(h.hwnd.get() as *mut _)),
            other => return Err(anyhow!("expected Win32 handle, got {other:?}")),
        }
    }

    pub(super) fn start_system_drag(window: &Window) -> Result<()> {
        let hwnd = hwnd(window)?;

        unsafe {
            let _ = ReleaseCapture();
            PostMessageW(Some(hwnd), WM_NCLBUTTONDOWN, WPARAM(HTCAPTION as usize), LPARAM(0)).map_err(|e| anyhow!("PostMessageW failed: {e}"))?;
        }

        Ok(())
    }
}

#[cfg(not(target_os = "windows"))]
mod platform {
    use super::*;

    pub(super) fn start_system_drag(window: &Window) -> Result<()> {
        window.start_window_move();
        Ok(())
    }
}
