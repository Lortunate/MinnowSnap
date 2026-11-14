use super::{
    pages,
    state::{self, PreferencesNotice, PreferencesPage, PreferencesState, frame::PreferencesFrame},
};
use crate::presentation::hotkeys::{HotkeyAction, format_keystroke, shortcuts_conflict};
use crate::presentation::i18n;
use crate::presentation::{appearance, locale};
use gpui::{
    AnyElement, App, AsyncWindowContext, ClickEvent, Context, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent, ParentElement,
    PathPromptOptions, SharedString, StatefulInteractiveElement, Styled, WeakEntity, Window, div, px,
};
use gpui_component::ActiveTheme as _;
use minnow_application::settings::{SaveDirectory, SaveDirectoryError, SettingsAction, ShortcutSettings};

#[derive(Clone)]
pub(super) struct PreferencesView {
    pub(super) focus_handle: FocusHandle,
    pub(super) state: PreferencesState,
    services: crate::UiServices,
    fonts: Vec<SharedString>,
}

impl PreferencesView {
    pub(super) fn new(focus_handle: FocusHandle, cx: &mut Context<Self>) -> Self {
        let services = cx.global::<crate::UiServices>().clone();
        if let Some(mut status) = services.application.subscribe_model_status() {
            cx.spawn(async move |this, cx| {
                while status.changed().await.is_ok() {
                    if this.update(cx, |_, cx| cx.notify()).is_err() {
                        break;
                    }
                }
            })
            .detach();
        }
        let preferences = services.application.preferences().clone();
        let mut previous = preferences.persistence_status();
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(std::time::Duration::from_millis(500)).await;
                let current = preferences.persistence_status();
                if current != previous {
                    previous = current;
                    if this.update(cx, |_, cx| cx.notify()).is_err() {
                        break;
                    }
                } else if this.upgrade().is_none() {
                    break;
                }
            }
        })
        .detach();
        services.application.preferences().refresh_native(services.native_preferences.as_ref());
        let fonts = services.host.system_fonts().into_iter().map(SharedString::from).collect();
        Self {
            focus_handle,
            state: PreferencesState::new(),
            services,
            fonts,
        }
    }

    pub(super) fn request_close(window: &mut Window, cx: &mut App) {
        window.defer(cx, |window, _| {
            window.remove_window();
        });
    }

    pub(super) fn on_language_selected(&mut self, value: SharedString, _: &mut Window, cx: &mut Context<Self>) {
        let language = value.to_string();
        if self.apply_preference(SettingsAction::Language(language.clone()), cx) {
            locale::apply(&language);
            cx.refresh_windows();
        }
    }

    pub(super) fn on_theme_selected(&mut self, value: SharedString, window: &mut Window, cx: &mut Context<Self>) {
        let theme = value.to_string();
        if self.apply_preference(SettingsAction::Theme(theme.clone()), cx) {
            appearance::apply_theme_choice(&theme, Some(window), cx);
        }
    }

    pub(super) fn on_font_selected(&mut self, value: SharedString, _: &mut Window, cx: &mut Context<Self>) {
        let font = value.to_string();
        if self.apply_preference(SettingsAction::FontFamily(font.clone()), cx) {
            appearance::apply_font_family((!font.trim().is_empty()).then_some(font.as_str()), cx);
        }
    }

    pub(super) fn on_open_repository(_: &ClickEvent, _: &mut Window, cx: &mut App) {
        cx.open_url("https://github.com/Lortunate/MinnowSnap");
    }

    pub(super) fn on_report_issue(_: &ClickEvent, _: &mut Window, cx: &mut App) {
        cx.open_url("https://github.com/Lortunate/MinnowSnap/issues");
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
        self.apply_preference(SettingsAction::AutoStart(checked), cx);
    }

    pub(super) fn on_refresh_native(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.services
            .application
            .preferences()
            .refresh_native(self.services.native_preferences.as_ref());
        cx.notify();
    }

    pub(super) fn on_image_compression_changed(&mut self, checked: bool, _: &mut Window, cx: &mut Context<Self>) {
        self.apply_preference(SettingsAction::OxipngEnabled(checked), cx);
    }

    pub(super) fn on_notifications_enabled_changed(&mut self, checked: bool, _: &mut Window, cx: &mut Context<Self>) {
        self.apply_preference(SettingsAction::NotificationEnabled(checked), cx);
    }

    pub(super) fn on_save_notification_changed(&mut self, checked: bool, _: &mut Window, cx: &mut Context<Self>) {
        self.apply_preference(SettingsAction::SaveNotification(checked), cx);
    }

    pub(super) fn on_copy_notification_changed(&mut self, checked: bool, _: &mut Window, cx: &mut Context<Self>) {
        self.apply_preference(SettingsAction::CopyNotification(checked), cx);
    }

    pub(super) fn on_qr_notification_changed(&mut self, checked: bool, _: &mut Window, cx: &mut Context<Self>) {
        self.apply_preference(SettingsAction::QrCodeNotification(checked), cx);
    }

    pub(super) fn on_shutter_sound_changed(&mut self, checked: bool, _: &mut Window, cx: &mut Context<Self>) {
        self.apply_preference(SettingsAction::ShutterSound(checked), cx);
    }

    pub(super) fn on_ocr_enabled_changed(&mut self, checked: bool, _: &mut Window, cx: &mut Context<Self>) {
        self.apply_preference(SettingsAction::OcrEnabled(checked), cx);
    }

    pub(super) fn on_capture_shortcut_record(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.begin_shortcut_recording(HotkeyAction::Capture, window, cx);
    }

    pub(super) fn on_quick_shortcut_record(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.begin_shortcut_recording(HotkeyAction::QuickCapture, window, cx);
    }

    pub(super) fn on_restore_default_shortcuts(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.state.stop_shortcut_recording();
        self.apply_shortcuts(ShortcutSettings::default(), cx);
    }

    pub(super) fn on_retry_shortcuts(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.state.stop_shortcut_recording();
        match self
            .services
            .application
            .preferences()
            .retry_shortcuts(self.services.native_preferences.as_ref())
        {
            Ok(_) => self.show_notice(PreferencesNotice::info(i18n::preferences::shortcuts_retry_succeeded()), cx),
            Err(error) => self.show_error(i18n::preferences::shortcuts_update_failed(error.message), cx),
        }
    }

    pub(super) fn on_shortcut_key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(target_action) = self.state.shortcut_recording else {
            return;
        };

        if event.is_held {
            return;
        }

        let Some(formatted) = format_keystroke(&event.keystroke) else {
            return;
        };

        self.state.stop_shortcut_recording();
        let desired = self.services.application.preferences().snapshot().shortcuts;
        if state::shortcuts::conflicts_with_desired(&desired, target_action, &formatted) {
            self.show_error(i18n::preferences::shortcuts_conflict(), cx);
            return;
        }
        let action = match target_action {
            HotkeyAction::Capture => SettingsAction::CaptureShortcut(formatted),
            HotkeyAction::QuickCapture => SettingsAction::QuickCaptureShortcut(formatted),
        };
        self.apply_preference(action, cx);
    }

    pub(super) fn on_download_ocr_models(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.download_ocr_models(window, cx);
    }

    pub(super) fn on_browse_save_path(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.browse_save_path(window, cx);
    }

    fn apply_preference(&mut self, action: SettingsAction, cx: &mut Context<Self>) -> bool {
        match self
            .services
            .application
            .preferences()
            .apply(action, self.services.native_preferences.as_ref())
        {
            Ok(_) => {
                self.state.clear_notice();
                cx.refresh_windows();
                cx.notify();
                true
            }
            Err(error) => {
                self.show_error(error.to_string(), cx);
                false
            }
        }
    }

    fn begin_shortcut_recording(&mut self, action: HotkeyAction, window: &mut Window, cx: &mut Context<Self>) {
        self.state.start_shortcut_recording(action);
        self.focus_handle.focus(window, cx);
        cx.notify();
    }

    fn apply_shortcuts(&mut self, bindings: ShortcutSettings, cx: &mut Context<Self>) {
        if shortcuts_conflict(&bindings.capture, &bindings.quick_capture) {
            self.show_error(i18n::preferences::shortcuts_conflict(), cx);
            return;
        }
        self.apply_preference(
            SettingsAction::Shortcuts {
                capture: bindings.capture,
                quick_capture: bindings.quick_capture,
            },
            cx,
        );
    }

    fn download_ocr_models(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if let Err(error) = self.services.application.recognition_availability().require_available() {
            self.show_error(i18n::preferences::ocr_runtime_unavailable(error.message), cx);
            return;
        }
        let job = self.services.application.ensure_models(true);
        cx.spawn(async move |this, cx| {
            let result = job.await;
            let _ = this.update(cx, |view, cx| match result {
                Ok(()) => view.show_notice(PreferencesNotice::info(i18n::preferences::ocr_download_completed()), cx),
                Err(error) => view.show_error(error.to_string(), cx),
            });
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

                let directory = SaveDirectory::new(next_path);
                let _ = this.update(&mut cx, move |view: &mut PreferencesView, cx| match directory {
                    Ok(directory) => {
                        view.apply_preference(SettingsAction::SavePath(directory), cx);
                    }
                    Err(SaveDirectoryError::NonUtf8) => view.show_error(i18n::preferences::save_directory_non_utf8(), cx),
                    Err(SaveDirectoryError::ContainsNul) => view.show_error(i18n::preferences::save_directory_contains_nul(), cx),
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

    fn render_panel(&self, frame: &PreferencesFrame, cx: &mut Context<Self>) -> gpui::Stateful<gpui::Div> {
        let page_notice = (!frame.notices.is_empty()).then(|| {
            gpui::div()
                .flex()
                .flex_col()
                .gap_2()
                .children(frame.notices.iter().map(|notice| pages::components::notice_banner(notice, cx)))
                .into_any_element()
        });
        let page_body = pages::render_active_page(&frame.page, cx);
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
                    .flex_col()
                    .flex()
                    .size_full()
                    .rounded(theme.radius_lg)
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.popover)
                    .overflow_hidden()
                    .child(pages::chrome::title_bar(
                        frame.page_title.clone(),
                        pages::components::close_button(|_, window, cx| Self::request_close(window, cx), cx),
                        cx,
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_1()
                            .min_h(px(0.))
                            .child(pages::chrome::sidebar_panel(sidebar_items, cx))
                            .child(pages::chrome::content_panel(page_notice, page_body, cx)),
                    ),
            );

        if theme.shadow {
            panel = panel.shadow_lg();
        }

        panel
    }

    fn render_window(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let frame = state::frame::build(&self.state, &self.services.native_preferences.snapshot(), &self.fonts, cx);
        self.render_panel(&frame, cx)
    }
}

impl gpui::Render for PreferencesView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.render_window(cx)
    }
}
