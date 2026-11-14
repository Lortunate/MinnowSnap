#[cfg(target_os = "windows")]
use gpui::MouseButton;
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
                    self.on_mouse_down(MouseButton::Left, |_, window, cx| {
                        if let Err(err) = crate::host(cx).start_window_drag(window) {
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
