use gpui::{App, IntoElement, ParentElement, Styled, div, px};
use gpui_component::ActiveTheme as _;

use crate::features::overlay::state::OverlayDiagnosticsSnapshot;

pub(crate) fn overlay_diagnostics_hud(app_ctx: &App, snapshot: &OverlayDiagnosticsSnapshot, viewport_w: f64, viewport_h: f64) -> impl IntoElement {
    let theme = app_ctx.theme();
    let width = 300.0_f64.min((viewport_w - 24.0).max(220.0));
    let left = (viewport_w - width - 12.0).max(12.0);
    let top = (viewport_h - 110.0).max(12.0);

    let mut panel = div()
        .absolute()
        .left(px(left as f32))
        .top(px(top as f32))
        .w(px(width as f32))
        .rounded(theme.radius)
        .border_1()
        .border_color(theme.border)
        .bg(theme.popover)
        .px_3()
        .py_2();
    if theme.shadow {
        panel = panel.shadow_lg();
    }
    panel
        .child(div().text_xs().text_color(theme.popover_foreground).child("Overlay Diagnostics"))
        .child(div().mt_1().text_xs().text_color(theme.muted_foreground).child(format!(
            "Pointer {:.0}Hz · Render {:.0}Hz · Refresh {:.0}Hz",
            snapshot.pointer_hz, snapshot.render_hz, snapshot.refresh_hz
        )))
        .child(div().mt_1().text_xs().text_color(theme.muted_foreground).child(format!(
            "Applied {:.0}% · Coalesced {:.0}%",
            snapshot.apply_ratio * 100.0,
            snapshot.coalesced_ratio * 100.0
        )))
        .child(div().mt_1().text_xs().text_color(theme.muted_foreground).child(format!(
            "Raster C{} / P{} / B{} · Fast D{} / M{}",
            snapshot.annotation_committed_rebuilds,
            snapshot.annotation_composed_rebuilds,
            snapshot.annotation_interaction_base_rebuilds,
            snapshot.annotation_drawing_fast_path_hits,
            snapshot.annotation_moving_fast_path_hits
        )))
}
