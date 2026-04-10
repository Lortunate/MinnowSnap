use gpui::{App, IntoElement, ParentElement, Styled, div, px};
use gpui_component::ActiveTheme as _;

use crate::features::overlay::render::components::handle_marker;
use crate::features::overlay::state::ResizeCorner;
use minnow_core::geometry::RectF;

pub(crate) fn overlay_mask(app_ctx: &App, selection: Option<RectF>, viewport_w: f64, viewport_h: f64) -> impl IntoElement {
    let theme = app_ctx.theme();
    let mask_color = theme.overlay;

    let Some(selection) = selection else {
        return div().absolute().left(px(0.0)).top(px(0.0)).size_full().bg(mask_color);
    };

    let right = selection.x + selection.width;
    let bottom = selection.y + selection.height;

    div()
        .absolute()
        .left(px(0.0))
        .top(px(0.0))
        .size_full()
        .child(
            div()
                .absolute()
                .left(px(0.0))
                .top(px(0.0))
                .w(px(viewport_w as f32))
                .h(px(selection.y.max(0.0) as f32))
                .bg(mask_color),
        )
        .child(
            div()
                .absolute()
                .left(px(0.0))
                .top(px(selection.y.max(0.0) as f32))
                .w(px(selection.x.max(0.0) as f32))
                .h(px(selection.height.max(0.0) as f32))
                .bg(mask_color),
        )
        .child(
            div()
                .absolute()
                .left(px(right.max(0.0) as f32))
                .top(px(selection.y.max(0.0) as f32))
                .w(px((viewport_w - right).max(0.0) as f32))
                .h(px(selection.height.max(0.0) as f32))
                .bg(mask_color),
        )
        .child(
            div()
                .absolute()
                .left(px(0.0))
                .top(px(bottom.max(0.0) as f32))
                .w(px(viewport_w as f32))
                .h(px((viewport_h - bottom).max(0.0) as f32))
                .bg(mask_color),
        )
}

pub(crate) fn selection_frame(app_ctx: &App, selection: RectF, movable: bool) -> impl IntoElement {
    let theme = app_ctx.theme();
    let frame = div()
        .absolute()
        .left(px(selection.x as f32))
        .top(px(selection.y as f32))
        .w(px(selection.width as f32))
        .h(px(selection.height as f32))
        .rounded_lg()
        .border_2()
        .border_color(theme.selection)
        .bg(theme.selection.alpha(0.075));

    if movable { frame.cursor_move() } else { frame }
}

pub(crate) fn selection_handles(app_ctx: &App, selection: RectF) -> impl IntoElement {
    let width = selection.width.max(0.0) as f32;
    let height = selection.height.max(0.0) as f32;
    let center_x = width / 2.0;
    let center_y = height / 2.0;

    let edge_hit = 12.0;
    let corner_hit = 16.0;
    let half_edge = edge_hit / 2.0;
    let half_corner = corner_hit / 2.0;

    let v_hit_h = (height - corner_hit).max(0.0);
    let h_hit_w = (width - corner_hit).max(0.0);

    let hit_box = |x: f32, y: f32, w: f32, h: f32| div().absolute().left(px(x)).top(px(y)).w(px(w)).h(px(h));

    div()
        .absolute()
        .left(px(selection.x as f32))
        .top(px(selection.y as f32))
        .w(px(width))
        .h(px(height))
        .child(hit_box(-half_corner, -half_corner, corner_hit, corner_hit).cursor_nwse_resize())
        .child(hit_box(width - half_corner, -half_corner, corner_hit, corner_hit).cursor_nesw_resize())
        .child(hit_box(-half_corner, height - half_corner, corner_hit, corner_hit).cursor_nesw_resize())
        .child(hit_box(width - half_corner, height - half_corner, corner_hit, corner_hit).cursor_nwse_resize())
        .child(hit_box(-half_edge, half_corner, edge_hit, v_hit_h).cursor_ew_resize())
        .child(hit_box(width - half_edge, half_corner, edge_hit, v_hit_h).cursor_ew_resize())
        .child(hit_box(half_corner, -half_edge, h_hit_w, edge_hit).cursor_ns_resize())
        .child(hit_box(half_corner, height - half_edge, h_hit_w, edge_hit).cursor_ns_resize())
        .children(
            [
                (0.0, 0.0, ResizeCorner::TopLeft),
                (width, 0.0, ResizeCorner::TopRight),
                (0.0, height, ResizeCorner::BottomLeft),
                (width, height, ResizeCorner::BottomRight),
                (0.0, center_y, ResizeCorner::Left),
                (width, center_y, ResizeCorner::Right),
                (center_x, 0.0, ResizeCorner::Top),
                (center_x, height, ResizeCorner::Bottom),
            ]
            .into_iter()
            .map(|(x, y, corner)| handle_marker(app_ctx, x as f64, y as f64, corner)),
        )
        .cursor_move()
}
