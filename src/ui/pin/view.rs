use crate::core::i18n;
use gpui::InteractiveElement;
use gpui::StyledImage;
use gpui::{
    App, Context, FocusHandle, IntoElement, MouseButton, MouseDownEvent, ObjectFit, ParentElement, ScrollWheelEvent, Styled, Window,
    WindowControlArea, div, img, px, size,
};
use gpui_component::menu::{ContextMenuExt, PopupMenuItem};
use gpui_component::{ActiveTheme as _, Root};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use tracing::{info, warn};

use super::actions::{CloseAllPins, ClosePin, PIN_CONTEXT};
use super::request::PinRequest;

#[derive(Clone, Debug)]
pub(super) struct PinView {
    request: PinRequest,
    focus_handle: FocusHandle,
    base_size: (f32, f32),
    zoom: f32,
    opacity: f32,
}

impl PinView {
    const MIN_SIZE: f32 = 24.0;
    const MIN_ZOOM: f32 = 0.2;
    const MAX_ZOOM: f32 = 8.0;
    const ZOOM_STEP: f32 = 0.1;
    const MIN_OPACITY: f32 = 0.2;
    const MAX_OPACITY: f32 = 1.0;
    const OPACITY_STEP: f32 = 0.05;

    pub(super) fn new(request: PinRequest, focus_handle: FocusHandle) -> Self {
        let base_size = request.base_size();
        Self {
            request,
            focus_handle,
            base_size,
            zoom: Self::initial_zoom(base_size),
            opacity: 1.0,
        }
    }

    pub(super) fn min_size() -> f32 {
        Self::MIN_SIZE
    }

    pub(super) fn initial_zoom(base_size: (f32, f32)) -> f32 {
        Self::min_zoom_for(base_size).max(1.0).min(Self::MAX_ZOOM)
    }

    fn min_zoom_for(base_size: (f32, f32)) -> f32 {
        let (base_width, base_height) = base_size;
        if base_width <= 0.0 || base_height <= 0.0 {
            return Self::MIN_ZOOM;
        }

        (Self::MIN_SIZE / base_width).max(Self::MIN_SIZE / base_height).max(Self::MIN_ZOOM)
    }

    fn zoom_bounds(&self) -> (f32, f32) {
        let min_zoom = Self::min_zoom_for(self.base_size).min(Self::MAX_ZOOM);
        (min_zoom, Self::MAX_ZOOM)
    }

    fn scaled_size(&self) -> (f32, f32) {
        let (base_width, base_height) = self.base_size;
        (base_width * self.zoom, base_height * self.zoom)
    }

    #[cfg(target_os = "windows")]
    fn start_native_window_drag(window: &Window) {
        use core::ffi::c_void;
        use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
        use windows::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture;
        use windows::Win32::UI::WindowsAndMessaging::{HTCAPTION, SendMessageW, WM_NCLBUTTONDOWN};

        match <Window as HasWindowHandle>::window_handle(window) {
            Ok(handle) => match handle.as_raw() {
                RawWindowHandle::Win32(raw) => {
                    let hwnd = HWND(raw.hwnd.get() as *mut c_void);
                    unsafe {
                        let _ = ReleaseCapture();
                        let _ = SendMessageW(hwnd, WM_NCLBUTTONDOWN, Some(WPARAM(HTCAPTION as usize)), Some(LPARAM(0)));
                    }
                    info!(target: "minnowsnap::pin", "pin native window drag requested (win32)");
                }
                other => {
                    warn!(target: "minnowsnap::pin", handle = ?other, "pin native window drag skipped: non-win32 handle");
                }
            },
            Err(err) => {
                warn!(target: "minnowsnap::pin", error = %err, "pin native window drag skipped: failed to get handle");
            }
        }
    }

    fn request_close(window: &mut Window, cx: &mut App) {
        window.defer(cx, |window, _| {
            window.remove_window();
        });
    }

    fn close_all(cx: &mut App) {
        cx.defer(|cx| {
            let handles: Vec<_> = cx
                .windows()
                .into_iter()
                .filter_map(|handle| {
                    handle
                        .read::<Root, _, _>(cx, |root, cx| root.read(cx).view().clone().downcast::<PinView>().ok())
                        .ok()
                        .flatten()
                        .map(|_| handle)
                })
                .collect::<Vec<_>>();
            info!(target: "minnowsnap::pin", count = handles.len(), "closing all pin windows");
            for handle in handles {
                let _ = handle.update(cx, |_, window, _| {
                    window.remove_window();
                });
            }
        });
    }

    fn on_action_close(&mut self, _: &ClosePin, window: &mut Window, cx: &mut Context<Self>) {
        Self::request_close(window, cx);
    }

    fn on_action_close_all(&mut self, _: &CloseAllPins, _window: &mut Window, cx: &mut Context<Self>) {
        Self::close_all(cx);
    }

    fn on_mouse_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if event.button == MouseButton::Left {
            info!(
                target: "minnowsnap::pin",
                button = "left",
                position = ?event.position,
                "pin mouse down: request window move"
            );
            #[cfg(target_os = "windows")]
            {
                window.start_window_move();
                info!(target: "minnowsnap::pin", "pin mouse down: requested gpui start_window_move()");
                window.defer(cx, |window, _| {
                    Self::start_native_window_drag(window);
                });
                return;
            }

            #[cfg(not(target_os = "windows"))]
            window.start_window_move();
        }
    }

    fn apply_opacity_step(&mut self, step: f32) -> bool {
        let next_opacity = (self.opacity + step * Self::OPACITY_STEP).clamp(Self::MIN_OPACITY, Self::MAX_OPACITY);
        if (next_opacity - self.opacity).abs() <= f32::EPSILON {
            return false;
        }
        self.opacity = next_opacity;
        true
    }

    fn apply_zoom_step(&mut self, step: f32, window: &mut Window) -> bool {
        let (min_zoom, max_zoom) = self.zoom_bounds();
        let next_zoom = (self.zoom + step * Self::ZOOM_STEP).clamp(min_zoom, max_zoom);
        if (next_zoom - self.zoom).abs() <= f32::EPSILON {
            return false;
        }
        self.zoom = next_zoom;
        window.resize(self.window_size_for_zoom());
        true
    }

    fn on_scroll_wheel(&mut self, event: &ScrollWheelEvent, window: &mut Window, cx: &mut Context<Self>) {
        let delta_y = event.delta.pixel_delta(px(24.0)).y.to_f64() as f32;
        if delta_y.abs() < f32::EPSILON {
            return;
        }

        let step = if delta_y > 0.0 { 1.0 } else { -1.0 };
        let changed = if event.modifiers.alt {
            self.apply_opacity_step(step)
        } else {
            self.apply_zoom_step(step, window)
        };

        if changed {
            cx.notify();
        }
    }

    fn window_size_for_zoom(&self) -> gpui::Size<gpui::Pixels> {
        let (draw_width, draw_height) = self.scaled_size();
        size(px(draw_width), px(draw_height))
    }
}

impl gpui::Render for PinView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let close_text = i18n::common::close();
        let close_all_text = i18n::pin::close_all();

        div()
            .id("pin-view")
            .track_focus(&self.focus_handle)
            .window_control_area(WindowControlArea::Drag)
            .key_context(PIN_CONTEXT)
            .on_action(cx.listener(Self::on_action_close))
            .on_action(cx.listener(Self::on_action_close_all))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_scroll_wheel(cx.listener(Self::on_scroll_wheel))
            .size_full()
            .bg(theme.transparent)
            .context_menu(move |menu, _, _| {
                menu.item(PopupMenuItem::new(close_text.clone()).on_click(|_, window, cx| {
                    Self::request_close(window, cx);
                }))
                .separator()
                .item(PopupMenuItem::new(close_all_text.clone()).on_click(|_, _window, cx| {
                    Self::close_all(cx);
                }))
            })
            .child({
                let mut panel = div()
                    .flex()
                    .size_full()
                    .items_center()
                    .justify_center()
                    .overflow_hidden()
                    .rounded(theme.radius_lg)
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.popover)
                    .window_control_area(WindowControlArea::Drag);
                if theme.shadow {
                    panel = panel.shadow_lg();
                }
                panel.child(
                    img(self.request.image_path.clone())
                        .window_control_area(WindowControlArea::Drag)
                        .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
                        .size_full()
                        .object_fit(ObjectFit::Contain)
                        .opacity(self.opacity),
                )
            })
    }
}
