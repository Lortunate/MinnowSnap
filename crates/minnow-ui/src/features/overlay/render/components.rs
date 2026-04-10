use gpui::{App, IntoElement, Styled, div, px};
use gpui_component::ActiveTheme as _;

use crate::features::overlay::state::ResizeCorner;

const HANDLE_MARKER_SIZE: f64 = 12.0;

pub(crate) fn handle_marker(app_ctx: &App, x: f64, y: f64, corner: ResizeCorner) -> impl IntoElement {
    let theme = app_ctx.theme();
    let size = HANDLE_MARKER_SIZE;

    let marker = div()
        .absolute()
        .left(px((x - size / 2.0) as f32))
        .top(px((y - size / 2.0) as f32))
        .w(px(size as f32))
        .h(px(size as f32))
        .rounded_full()
        .bg(theme.primary_foreground.opacity(0.95))
        .border_1()
        .border_color(theme.primary);

    match corner {
        ResizeCorner::TopLeft | ResizeCorner::BottomRight => marker.cursor_nwse_resize(),
        ResizeCorner::TopRight | ResizeCorner::BottomLeft => marker.cursor_nesw_resize(),
        ResizeCorner::Left | ResizeCorner::Right => marker.cursor_ew_resize(),
        ResizeCorner::Top | ResizeCorner::Bottom => marker.cursor_ns_resize(),
    }
}
