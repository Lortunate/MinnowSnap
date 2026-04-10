use super::{
    pages::{self, PreferencesRenderActions},
    state::{
        self, MutationResult,
        frame::PreferencesFrame,
        state::{PreferencesNotice, PreferencesPage, PreferencesState},
    },
};
use crate::shell::{
    hotkey::{self, HotkeyAction, ShortcutBindings},
    system,
};
use gpui::{
    AnyElement, App, AsyncWindowContext, ClickEvent, Context, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent, ParentElement,
    PathPromptOptions, SharedString, StatefulInteractiveElement, Styled, WeakEntity, Window, div,
};
use gpui_component::ActiveTheme as _;
use minnow_core::{i18n, ocr::service};
use std::sync::Arc;

#[derive(Clone)]
pub(super) struct PreferencesView {
    pub(super) focus_handle: FocusHandle,
    pub(super) state: PreferencesState,
}

impl PreferencesView {
    pub(super) fn new(focus_handle: FocusHandle) -> Self {
        Self {
            focus_handle,
            state: PreferencesState::new(),
        }
    }

    pub(super) fn request_close(window: &mut Window, cx: &mut App) {
        window.defer(cx, |window, _| {
            window.remove_window();
        });
    }

    pub(super) fn on_language_selected(value: SharedString, _: &mut Window, cx: &mut App) {
        Self::apply_app_mutation(state::general::set_language(value), cx);
    }

    pub(super) fn on_theme_selected(value: SharedString, window: &mut Window, cx: &mut App) {
        Self::apply_app_mutation(state::general::set_theme(value, window, cx), cx);
    }

    pub(super) fn on_font_selected(value: SharedString, _: &mut Window, cx: &mut App) {
        Self::apply_app_mutation(state::general::set_font(value, cx), cx);
    }

    pub(super) fn on_open_repository(_: &ClickEvent, _: &mut Window, cx: &mut App) {
        system::open_external_url(cx, "https://github.com/Lortunate/MinnowSnap");
    }

    pub(super) fn on_report_issue(_: &ClickEvent, _: &mut Window, cx: &mut App) {
        system::open_external_url(cx, "https://github.com/Lortunate/MinnowSnap/issues");
    }

    pub(super) fn select_page(&mut self, page: PreferencesPage, cx: &mut Context<Self>) {
        if self.state.select_page(page) {
            cx.notify();
        }
    }

    pub(super) fn show_notice(&mut self, notice: PreferencesNotice, cx: &mut Context<Self>) {
        self.state.show_notice(notice);
        cx.notify();
    }

    pub(super) fn show_error(&mut self, message: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.show_notice(PreferencesNotice::error(message), cx);
    }

    pub(super) fn clear_notice(&mut self, cx: &mut Context<Self>) {
        if self.state.clear_notice() {
            cx.notify();
        }
    }

    pub(super) fn on_auto_start_changed(&mut self, checked: bool, _: &mut Window, cx: &mut Context<Self>) {
        let result = state::general::set_auto_start(checked, cx);
        self.apply_mutation(result, cx);
    }

    pub(super) fn on_image_compression_changed(&mut self, checked: bool, _: &mut Window, cx: &mut Context<Self>) {
        self.apply_mutation(state::general::set_image_compression(checked), cx);
    }

    pub(super) fn on_notifications_enabled_changed(&mut self, checked: bool, _: &mut Window, cx: &mut Context<Self>) {
        self.apply_mutation(state::notifications::set_enabled(checked), cx);
    }

    pub(super) fn on_save_notification_changed(&mut self, checked: bool, _: &mut Window, cx: &mut Context<Self>) {
        self.apply_mutation(state::notifications::set_save_notification(checked), cx);
    }

    pub(super) fn on_copy_notification_changed(&mut self, checked: bool, _: &mut Window, cx: &mut Context<Self>) {
        self.apply_mutation(state::notifications::set_copy_notification(checked), cx);
    }

    pub(super) fn on_qr_notification_changed(&mut self, checked: bool, _: &mut Window, cx: &mut Context<Self>) {
        self.apply_mutation(state::notifications::set_qr_code_notification(checked), cx);
    }

    pub(super) fn on_shutter_sound_changed(&mut self, checked: bool, _: &mut Window, cx: &mut Context<Self>) {
        self.apply_mutation(state::notifications::set_shutter_sound(checked), cx);
    }

    pub(super) fn on_ocr_enabled_changed(&mut self, checked: bool, _: &mut Window, cx: &mut Context<Self>) {
        self.apply_mutation(state::ocr::set_enabled(checked), cx);
    }

    pub(super) fn on_capture_shortcut_record(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.begin_shortcut_recording(HotkeyAction::Capture, window, cx);
    }

    pub(super) fn on_quick_shortcut_record(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.begin_shortcut_recording(HotkeyAction::QuickCapture, window, cx);
    }

    pub(super) fn on_restore_default_shortcuts(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.state.stop_shortcut_recording();
        self.apply_shortcuts(ShortcutBindings::default(), cx);
    }

    pub(super) fn on_shortcut_key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(target_action) = self.state.shortcut_recording else {
            return;
        };

        if event.is_held {
            return;
        }

        let Some(formatted) = hotkey::format_keystroke(&event.keystroke) else {
            return;
        };

        self.state.stop_shortcut_recording();
        let current = state::shortcuts::snapshot(cx).bindings;
        let updated = state::shortcuts::next_shortcut_bindings(&current, target_action, &formatted);
        self.apply_shortcuts(updated, cx);
    }

    pub(super) fn on_download_ocr_models(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.download_ocr_models(window, cx);
    }

    pub(super) fn on_browse_save_path(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.browse_save_path(window, cx);
    }

    fn apply_app_mutation(result: MutationResult, cx: &mut App) {
        if result.refresh_windows {
            cx.refresh_windows();
        }
    }

    fn apply_mutation(&mut self, result: MutationResult, cx: &mut Context<Self>) {
        if result.clear_notice {
            self.state.clear_notice();
        }
        if result.refresh_windows {
            cx.refresh_windows();
        }
        cx.notify();
    }

    fn begin_shortcut_recording(&mut self, action: HotkeyAction, window: &mut Window, cx: &mut Context<Self>) {
        self.state.start_shortcut_recording(action);
        self.focus_handle.focus(window);
        cx.notify();
    }

    fn apply_shortcuts(&mut self, bindings: ShortcutBindings, cx: &mut Context<Self>) {
        match state::shortcuts::persist_shortcut_bindings(bindings, cx) {
            Ok(result) => self.apply_mutation(result, cx),
            Err(message) => self.show_error(message, cx),
        }
    }

    fn download_ocr_models(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.state.ocr_download.in_progress {
            return;
        }

        self.state.start_ocr_download();
        cx.notify();

        cx.spawn_in(window, move |this: WeakEntity<PreferencesView>, cx: &mut AsyncWindowContext| {
            let mut cx = cx.clone();
            async move {
                let (progress_tx, mut progress_rx) = tokio::sync::mpsc::unbounded_channel::<u8>();
                let progress_callback = Arc::new(move |progress: f32| {
                    let percent = (progress * 100.0).round().clamp(0.0, 100.0) as u8;
                    let _ = progress_tx.send(percent);
                });

                let download = service::download_mobile_models(true, Some(progress_callback));
                tokio::pin!(download);

                let result = loop {
                    tokio::select! {
                        result = &mut download => break result,
                        Some(progress_percent) = progress_rx.recv() => {
                            let _ = this.update(&mut cx, |view: &mut PreferencesView, cx| {
                                if view.state.update_ocr_download_progress(progress_percent) {
                                    cx.notify();
                                }
                            });
                        }
                    }
                };

                let _ = this.update(&mut cx, |view: &mut PreferencesView, cx| {
                    view.state.finish_ocr_download(result);
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn folder_picker_failed(err: impl std::fmt::Display) -> String {
        format!("{}: {err}", i18n::preferences::folder_picker_failed())
    }

    fn browse_save_path(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.clear_notice(cx);
        let prompt = SharedString::from(i18n::preferences::select_save_directory());

        cx.spawn_in(window, move |this: WeakEntity<PreferencesView>, cx: &mut AsyncWindowContext| {
            let mut cx = cx.clone();
            async move {
                let receiver = match cx.update(|_: &mut Window, app: &mut App| {
                    app.prompt_for_paths(PathPromptOptions {
                        files: false,
                        directories: true,
                        multiple: false,
                        prompt: Some(prompt.clone()),
                    })
                }) {
                    Ok(receiver) => receiver,
                    Err(err) => {
                        let _ = this.update(&mut cx, |view: &mut PreferencesView, cx| {
                            view.show_error(Self::folder_picker_failed(err), cx);
                        });
                        return;
                    }
                };

                let next_path = match receiver.await {
                    Ok(Ok(Some(paths))) => paths.into_iter().next(),
                    Ok(Ok(None)) => None,
                    Ok(Err(err)) => {
                        let _ = this.update(&mut cx, |view: &mut PreferencesView, cx| {
                            view.show_error(Self::folder_picker_failed(err), cx);
                        });
                        return;
                    }
                    Err(err) => {
                        let _ = this.update(&mut cx, |view: &mut PreferencesView, cx| {
                            view.show_error(Self::folder_picker_failed(err), cx);
                        });
                        return;
                    }
                };

                let Some(next_path) = next_path else {
                    return;
                };

                let save_path = next_path.to_string_lossy().into_owned();
                let _ = this.update(&mut cx, |view: &mut PreferencesView, cx| {
                    view.apply_mutation(state::general::set_save_path(save_path), cx);
                });
            }
        })
        .detach();
    }

    fn render_sidebar_items(&self, frame: &PreferencesFrame, cx: &mut Context<Self>) -> Vec<AnyElement> {
        frame
            .sidebar_items
            .iter()
            .map(|item| {
                let page = item.page;
                pages::components::sidebar_item(item, cx)
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.select_page(page, cx);
                    }))
                    .into_any_element()
            })
            .collect()
    }

    fn render_panel(&self, frame: &PreferencesFrame, actions: &PreferencesRenderActions, cx: &mut Context<Self>) -> gpui::Stateful<gpui::Div> {
        let page_notice = frame.notice.as_ref().map(|notice| pages::components::notice_banner(notice, cx));
        let page_body = pages::render_active_page(frame, actions, cx);
        let sidebar_items = self.render_sidebar_items(frame, cx);
        let theme = cx.theme();

        let mut panel = div()
            .id("preferences-view")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::on_shortcut_key_down))
            .size_full()
            .bg(theme.transparent)
            .child(
                div()
                    .flex()
                    .size_full()
                    .rounded(theme.radius_lg)
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.popover)
                    .overflow_hidden()
                    .child(pages::chrome::sidebar_panel(sidebar_items, cx))
                    .child(pages::chrome::content_panel(
                        frame.page_title.clone(),
                        pages::components::close_button(|_, window, cx| Self::request_close(window, cx), cx),
                        page_notice,
                        page_body,
                        cx,
                    )),
            );

        if theme.shadow {
            panel = panel.shadow_lg();
        }

        panel
    }

    fn render_window(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let frame = self.build_frame(cx);
        let actions = PreferencesRenderActions::default();
        self.render_panel(&frame, &actions, cx)
    }

    fn build_frame(&self, cx: &App) -> PreferencesFrame {
        state::frame::build(&self.state, cx)
    }
}

impl gpui::Render for PreferencesView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.render_window(cx)
    }
}
