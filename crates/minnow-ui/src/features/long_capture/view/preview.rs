use crate::features::long_capture::coordinator::LongCaptureCoordinator;
use gpui::{Context, InteractiveElement, IntoElement, ObjectFit, ParentElement, Render, Styled, StyledImage, Window, div, img, px};
use gpui_component::ActiveTheme as _;
use minnow_core::i18n;
use std::sync::Arc;

pub(crate) struct PreviewWindowView {
    coordinator: Arc<LongCaptureCoordinator>,
}

impl PreviewWindowView {
    pub(crate) fn new(coordinator: Arc<LongCaptureCoordinator>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        coordinator.ensure_runtime_poller(window, cx);
        Self { coordinator }
    }
}

impl Render for PreviewWindowView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let snapshot = self.coordinator.snapshot();
        let theme = cx.theme();

        let mut panel = div()
            .id("long-capture-preview")
            .size_full()
            .rounded(theme.radius_lg)
            .border_1()
            .border_color(theme.border)
            .bg(theme.popover)
            .overflow_hidden();

        if theme.shadow {
            panel = panel.shadow_lg();
        }

        panel = if let Some(image) = snapshot.preview_image {
            panel.child(img(image).size_full().object_fit(ObjectFit::Contain))
        } else {
            panel.child(
                div()
                    .flex()
                    .size_full()
                    .items_center()
                    .justify_center()
                    .text_color(theme.muted_foreground)
                    .child(i18n::overlay::long_capture_scroll_hint()),
            )
        };

        div().size_full().bg(gpui::transparent_black()).child(
            panel.child(
                div()
                    .absolute()
                    .right_2()
                    .bottom_2()
                    .px_2()
                    .py_0p5()
                    .rounded(theme.radius_lg)
                    .bg(theme.primary)
                    .text_color(theme.primary_foreground)
                    .text_size(px(12.0))
                    .child(format!("{} px", snapshot.preview_height_px.max(0))),
            ),
        )
    }
}
