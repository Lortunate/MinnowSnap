use gpui::{App, IntoElement, ParentElement, Styled, div, px};
use gpui_component::ActiveTheme as _;

use crate::core::geometry::RectF;
use crate::core::window::WindowInfo;
use crate::ui::overlay::render::layout::OverlayPanelLayout;

const WINDOW_INFO_VERTICAL_PADDING: f64 = 4.0;
const WINDOW_INFO_LINE_HEIGHT: f64 = 16.0;
const WINDOW_INFO_LINE_GAP: f64 = 4.0;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct WindowInfoTooltipContent {
    pub title: String,
    pub subtitle: Option<String>,
    pub height: f64,
}

pub(crate) fn window_info_tooltip_content(window: &WindowInfo) -> Option<WindowInfoTooltipContent> {
    let title = if window.title.trim().is_empty() {
        window.app_name.trim().to_string()
    } else {
        window.title.trim().to_string()
    };

    if title.is_empty() {
        return None;
    }

    let subtitle = match window.app_name.trim() {
        "" => None,
        app_name if app_name == title => None,
        app_name => Some(app_name.to_string()),
    };

    let line_count = if subtitle.is_some() { 2.0 } else { 1.0 };
    let gaps = if subtitle.is_some() { WINDOW_INFO_LINE_GAP } else { 0.0 };

    Some(WindowInfoTooltipContent {
        title,
        subtitle,
        height: WINDOW_INFO_VERTICAL_PADDING * 2.0 + WINDOW_INFO_LINE_HEIGHT * line_count + gaps,
    })
}

fn tooltip_panel(app_ctx: &App, layout: OverlayPanelLayout) -> gpui::Div {
    let theme = app_ctx.theme();
    let mut panel = div()
        .absolute()
        .left(px(layout.x as f32))
        .top(px(layout.y as f32))
        .h(px(layout.height as f32))
        .flex()
        .flex_col()
        .justify_center()
        .rounded(theme.radius_lg)
        .border_1()
        .border_color(theme.border)
        .bg(theme.popover)
        .px_2()
        .py_1();

    if layout.width > 0.0 {
        panel = panel.max_w(px(layout.width as f32));
    }
    if theme.shadow {
        panel = panel.shadow_lg();
    }

    panel
}

fn tooltip_line(app_ctx: &App, text: impl Into<String>, muted: bool) -> gpui::Div {
    let theme = app_ctx.theme();
    let color = if muted { theme.muted_foreground } else { theme.popover_foreground };

    div()
        .flex()
        .items_center()
        .h(px(WINDOW_INFO_LINE_HEIGHT as f32))
        .overflow_hidden()
        .child(div().text_xs().text_color(color).line_clamp(1).text_ellipsis().child(text.into()))
}

pub(crate) fn resolution_tooltip(app_ctx: &App, selection: RectF, layout: OverlayPanelLayout) -> impl IntoElement {
    let theme = app_ctx.theme();
    let size = format!("{:.0} × {:.0}", selection.width.max(0.0), selection.height.max(0.0));

    tooltip_panel(app_ctx, layout).child(div().text_xs().text_color(theme.popover_foreground).child(size))
}

pub(crate) fn window_info_tooltip(app_ctx: &App, content: &WindowInfoTooltipContent, layout: OverlayPanelLayout) -> impl IntoElement {
    let mut panel = tooltip_panel(app_ctx, layout).child(tooltip_line(app_ctx, content.title.clone(), false));
    if let Some(subtitle) = content.subtitle.clone() {
        panel = panel.child(div().mt_1().child(tooltip_line(app_ctx, subtitle, true)));
    }

    panel
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolution_tooltip_text_only_contains_dimensions() {
        let selection = RectF::new(0.0, 0.0, 128.4, 64.2);
        let text = format!("{:.0} × {:.0}", selection.width.max(0.0), selection.height.max(0.0));

        assert_eq!(text, "128 × 64");
        assert!(!text.contains("Selection"));
    }

    #[test]
    fn window_info_prefers_window_title_when_present() {
        let info = WindowInfo {
            title: "MinnowSnap".into(),
            app_name: "Rust App".into(),
            x: 0,
            y: 0,
            width: 100,
            height: 100,
        };

        let content = window_info_tooltip_content(&info).unwrap();
        assert_eq!(content.title, "MinnowSnap");
        assert_eq!(content.subtitle.as_deref(), Some("Rust App"));
    }

    #[test]
    fn window_info_uses_single_line_when_title_matches_app() {
        let info = WindowInfo {
            title: "".into(),
            app_name: "MinnowSnap".into(),
            x: 0,
            y: 0,
            width: 100,
            height: 100,
        };

        let content = window_info_tooltip_content(&info).unwrap();
        assert_eq!(content.title, "MinnowSnap");
        assert_eq!(content.subtitle, None);
        assert_eq!(content.height, WINDOW_INFO_VERTICAL_PADDING * 2.0 + WINDOW_INFO_LINE_HEIGHT);
    }
}
