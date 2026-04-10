use gpui::{
    App, ClickEvent, Div, InteractiveElement, ObjectFit, ParentElement, SharedString, Styled, StyledImage, Window, WindowControlArea, div, img,
};
use gpui_component::ActiveTheme as _;
use gpui_component::menu::PopupMenuItem;
use std::path::PathBuf;

pub(super) fn menu_item(label: SharedString, on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> PopupMenuItem {
    PopupMenuItem::new(label).on_click(on_click)
}

pub(super) fn panel(image_path: PathBuf, opacity: f32, cx: &App) -> Div {
    let theme = cx.theme();
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
        img(image_path)
            .block_mouse_except_scroll()
            .size_full()
            .object_fit(ObjectFit::Contain)
            .opacity(opacity),
    )
}
