#[cfg(target_os = "windows")]
use anyhow::Result;
#[cfg(target_os = "windows")]
use anyhow::anyhow;
#[cfg(target_os = "windows")]
use gpui::MouseButton;
#[cfg(target_os = "windows")]
use gpui::Window;
#[cfg(not(target_os = "windows"))]
use gpui::WindowControlArea;
use gpui::{Div, InteractiveElement};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PopupDragBehavior {
    // Use GPUI's native drag support when we can; fallback to hit-test on Windows.
    SystemMove,
    HitTest,
}

pub trait PopupDragRegionExt {
    fn popup_drag_region(self, behavior: PopupDragBehavior) -> Self;
}

impl PopupDragRegionExt for Div {
    fn popup_drag_region(self, behavior: PopupDragBehavior) -> Self {
        match behavior {
            PopupDragBehavior::HitTest => self,
            PopupDragBehavior::SystemMove => {
                #[cfg(target_os = "windows")]
                {
                    self.on_mouse_down(MouseButton::Left, |_, window, _| {
                        if let Err(err) = platform::start_system_drag(window) {
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
    use crate::platform::native_window::raw_window_handle;
    use raw_window_handle::RawWindowHandle;
    use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
    use windows::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture;
    use windows::Win32::UI::WindowsAndMessaging::{HTCAPTION, PostMessageW, WM_NCLBUTTONDOWN};

    fn hwnd(window: &Window) -> Result<HWND> {
        let raw = raw_window_handle(window)?;
        match raw {
            RawWindowHandle::Win32(h) => Ok(HWND(h.hwnd.get() as *mut _)),
            other => Err(anyhow!("expected Win32 handle, got {other:?}")),
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
