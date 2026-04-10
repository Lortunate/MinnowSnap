use crate::shell::windowing::{PopupWindowSpec, configure_window, popup_window_options};
use gpui::{App, AppContext, Bounds, Context, IntoElement, Render, Styled, Window, WindowBounds, WindowKind, div, point, px, rgba, size};
use gpui_component::Root;
use minnow_core::app_meta::APP_ID;

/// Windows tray apps that close all visible GPUI windows can still receive late
/// window updates during teardown. Keeping a single hidden native host window
/// avoids the app entering a zero-window state while preserving tray-first UX.
///
/// This is intentionally isolated so the rest of the app can stay focused on
/// user-facing windows only.
#[derive(Default)]
struct BackgroundHostWindow;

impl Render for BackgroundHostWindow {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().bg(rgba(0x00000000))
    }
}

#[cfg(target_os = "windows")]
pub fn install(cx: &mut App) -> gpui::Result<()> {
    cx.open_window(
        popup_window_options(
            PopupWindowSpec {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(-10_000.0), px(-10_000.0)),
                    size(px(1.0), px(1.0)),
                ))),
                kind: WindowKind::PopUp,
                focus: false,
                show: false,
                is_movable: false,
                is_resizable: false,
                is_minimizable: false,
                display_id: None,
                window_min_size: Some(size(px(1.0), px(1.0))),
            },
            APP_ID,
        ),
        |window, cx| {
            configure_window(window, cx, false);
            let view = cx.new(|_| BackgroundHostWindow);
            cx.new(move |cx| Root::new(view, window, cx))
        },
    )?;

    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn install(_cx: &mut App) -> gpui::Result<()> {
    Ok(())
}
