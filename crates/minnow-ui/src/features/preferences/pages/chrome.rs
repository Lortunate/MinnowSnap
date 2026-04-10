use super::components::window_brand;
use gpui::{AnyElement, App, InteractiveElement, IntoElement, ParentElement, SharedString, Styled, WindowControlArea, div, px};
use gpui_component::{ActiveTheme as _, scroll::ScrollableElement, v_flex};

pub(crate) const SIDEBAR_WIDTH: f32 = 208.0;
pub(crate) const TITLEBAR_HEIGHT: f32 = 48.0;

pub(crate) fn sidebar_panel(items: Vec<AnyElement>, cx: &App) -> AnyElement {
    let theme = cx.theme();

    div()
        .w(px(SIDEBAR_WIDTH))
        .min_w(px(SIDEBAR_WIDTH))
        .max_w(px(SIDEBAR_WIDTH))
        .flex()
        .flex_col()
        .min_h(px(0.))
        .overflow_hidden()
        .border_r_1()
        .border_color(theme.border)
        .bg(theme.background)
        .child(
            div()
                .h(px(TITLEBAR_HEIGHT))
                .px_4()
                .border_b_1()
                .border_color(theme.border.alpha(0.7))
                .window_control_area(WindowControlArea::Drag)
                .child(window_brand()),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .min_h(px(0.))
                .overflow_y_scrollbar()
                .px_2()
                .py_3()
                .child(v_flex().w_full().gap_1().children(items)),
        )
        .into_any_element()
}

pub(crate) fn content_panel(
    page_title: impl Into<SharedString>,
    close_button: impl IntoElement,
    notice: Option<AnyElement>,
    page_body: AnyElement,
    cx: &App,
) -> AnyElement {
    let theme = cx.theme();

    div()
        .flex_1()
        .min_w(px(0.))
        .min_h(px(0.))
        .flex()
        .flex_col()
        .bg(theme.popover)
        .child(
            div()
                .h(px(TITLEBAR_HEIGHT))
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .px_6()
                .border_b_1()
                .border_color(theme.border.alpha(0.7))
                .window_control_area(WindowControlArea::Drag)
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .overflow_hidden()
                        .line_clamp(1)
                        .text_ellipsis()
                        .child(page_title.into()),
                )
                .child(close_button),
        )
        .child(
            div().flex_1().min_w(px(0.)).min_h(px(0.)).overflow_y_scrollbar().child(
                div()
                    .w_full()
                    .min_w(px(0.))
                    .px_6()
                    .py_5()
                    .child(v_flex().w_full().min_w(px(0.)).gap_5().children(notice).child(page_body)),
            ),
        )
        .into_any_element()
}
