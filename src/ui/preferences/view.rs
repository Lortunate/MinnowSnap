use crate::core::i18n;
use crate::core::settings::SETTINGS;
use gpui::{App, Context, FocusHandle, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled, Window, div};
use gpui_component::ActiveTheme as _;

#[derive(Clone)]
pub(super) struct PreferencesView {
    focus_handle: FocusHandle,
}

impl PreferencesView {
    pub(super) fn new(focus_handle: FocusHandle) -> Self {
        Self { focus_handle }
    }

    fn settings_snapshot() -> crate::core::settings::AppSettings {
        SETTINGS.lock().map(|guard| guard.get()).unwrap_or_default()
    }

    fn request_close(window: &mut Window, cx: &mut App) {
        window.defer(cx, |window, _| {
            window.remove_window();
        });
    }
}

impl gpui::Render for PreferencesView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let hover_bg = theme.accent;
        let settings = Self::settings_snapshot();
        let save_path = settings.output.save_path.clone().unwrap_or_else(i18n::default_path_label);
        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .gap_3()
            .px_4()
            .py_3()
            .border_b_1()
            .border_color(theme.border)
            .bg(theme.popover)
            .text_color(theme.popover_foreground)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(i18n::preferences::title())
                    .child(div().text_sm().text_color(theme.muted_foreground).child(i18n::preferences::subtitle())),
            )
            .child(
                div()
                    .id("preferences-close")
                    .cursor_pointer()
                    .px_3()
                    .py_1()
                    .rounded_md()
                    .text_color(theme.popover_foreground)
                    .hover(move |style| style.bg(hover_bg))
                    .on_click(|_, window: &mut Window, cx: &mut App| {
                        Self::request_close(window, cx);
                    })
                    .child(i18n::common::close()),
            );

        let body = div().flex_1().p_4().child(
            div()
                .flex()
                .flex_col()
                .gap_3()
                .rounded_lg()
                .border_1()
                .border_color(theme.border)
                .bg(theme.background)
                .text_color(theme.foreground)
                .p_4()
                .child(format!(
                    "{}: {}",
                    i18n::preferences::auto_start(),
                    i18n::bool_label(settings.general.auto_start)
                ))
                .child(format!("{}: {}", i18n::preferences::ocr(), i18n::bool_label(settings.ocr.enabled)))
                .child(format!(
                    "{}: {}",
                    i18n::preferences::notifications(),
                    i18n::bool_label(settings.notification.enabled)
                ))
                .child(format!("{}: {}", i18n::preferences::save_path(), save_path))
                .child(div().text_sm().text_color(theme.muted_foreground).child(i18n::preferences::intro())),
        );

        div()
            .id("preferences-view")
            .track_focus(&self.focus_handle)
            .size_full()
            .bg(theme.background)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .size_full()
                    .rounded(theme.radius_lg)
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.popover)
                    .overflow_hidden()
                    .child(header)
                    .child(body),
            )
    }
}
