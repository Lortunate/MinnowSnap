mod actions;

pub(crate) use actions::LongCaptureToolbarAction;

use crate::features::long_capture::coordinator::{LongCaptureCoordinator, LongCaptureResult, LongCaptureSnapshot};
use crate::features::long_capture::layout::{TOOLBAR_TOP_RESERVED, WINDOW_WARNING_HEIGHT, WINDOW_WARNING_TOP};
use crate::flows::output::{self, CaptureIntent, OutputRequest};
use crate::presentation::i18n;
use crate::presentation::panel_layout::{self, PanelLayout};
use actions::LongCaptureToolbarIcon;
use gpui::{App, ClickEvent, Context, Div, FocusHandle, IntoElement, KeyDownEvent, ParentElement, Render, Styled, Window, div, px};
use gpui::{InteractiveElement, StatefulInteractiveElement};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::tooltip::Tooltip;
use gpui_component::{ActiveTheme as _, Disableable, Icon, Sizable, h_flex};
use minnow_application::{ImageInput, SaveDestination};
use std::sync::Arc;

const WARNING_HEIGHT: f64 = 34.0;

pub(crate) struct ToolbarWindowView {
    coordinator: Arc<LongCaptureCoordinator>,
    focus_handle: FocusHandle,
}

impl ToolbarWindowView {
    pub(crate) fn new(coordinator: Arc<LongCaptureCoordinator>, focus_handle: FocusHandle, window: &mut Window, cx: &mut Context<Self>) -> Self {
        coordinator.ensure_runtime_poller(window, cx);
        Self { coordinator, focus_handle }
    }

    fn toolbar_button(&self, action: LongCaptureToolbarAction, snapshot: &LongCaptureSnapshot, cx: &mut Context<Self>) -> Button {
        let button = Button::new(action.id())
            .compact()
            .icon(toolbar_icon(cx, action.icon()))
            .tooltip(if action == LongCaptureToolbarAction::Cancel && snapshot.output_submitted {
                i18n::common::close()
            } else {
                action.tooltip()
            })
            .ghost()
            .disabled(action.disabled_when_busy() && (snapshot.busy || !snapshot.result.permits_output(snapshot.partial_accepted)));

        match action {
            LongCaptureToolbarAction::Save => button.on_click(cx.listener(Self::on_save)),
            LongCaptureToolbarAction::Pin => button.on_click(cx.listener(Self::on_pin)),
            LongCaptureToolbarAction::Copy => button.on_click(cx.listener(Self::on_copy)),
            LongCaptureToolbarAction::Cancel => button.on_click(cx.listener(Self::on_cancel)),
        }
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if event.is_held {
            return;
        }
        if event.keystroke.key == "escape" {
            self.cancel(window, cx);
        }
    }

    fn cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.coordinator.cancel_capture();
        self.coordinator.close_windows_except(Some(window.window_handle().window_id()), cx);
        window.defer(cx, |window, _| {
            window.remove_window();
        });
    }

    fn on_save(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.execute_capture_action(CaptureIntent::Save(SaveDestination::Preferences), window, cx);
    }

    fn on_pin(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.execute_capture_action(CaptureIntent::OpenPin, window, cx);
    }

    fn on_copy(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.execute_capture_action(CaptureIntent::Copy, window, cx);
    }

    fn on_cancel(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.cancel(window, cx);
    }

    fn on_use_partial(&mut self, _: &ClickEvent, window: &mut Window, _cx: &mut Context<Self>) {
        self.coordinator.accept_partial();
        window.refresh();
    }

    fn execute_capture_action(&mut self, intent: CaptureIntent, window: &mut Window, cx: &mut Context<Self>) {
        if !self.coordinator.start_capture_action() {
            return;
        }

        let services = crate::application(cx);
        let auto_ocr = services.settings_snapshot().ocr.enabled;
        let coordinator = self.coordinator.clone();
        let window_handle = window.window_handle();
        cx.spawn(async move |this, cx| {
            let request = coordinator
                .finish_image()
                .await
                .and_then(|image| ImageInput::image(image, None))
                .and_then(|input| intent.output(input, auto_ocr));
            let _ = window_handle.update(cx, |_, window, cx| {
                this.update(cx, |view, cx| match request {
                    Ok(request) => view.run_output(request, window, cx),
                    Err(error) => {
                        coordinator.show_preflight_error(error);
                        window.refresh();
                    }
                })
            });
        })
        .detach();
    }

    fn retry_output(&self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(request) = self.coordinator.retry_output() {
            self.run_output(request, window, cx);
        }
    }

    fn run_output(&self, request: OutputRequest, window: &mut Window, cx: &mut Context<Self>) {
        let Some(attempt) = self.coordinator.begin_output(&request) else {
            return;
        };
        window.refresh();
        let coordinator = self.coordinator.clone();
        let services = crate::application(cx);
        let window_handle = window.window_handle();
        cx.spawn(async move |this, cx| {
            let completion = output::execute(services, request).await;
            cx.update(|cx| {
                completion.deliver(cx, |mut delivery, cx| {
                    window_handle
                        .update(cx, |_, window, cx| {
                            this.update(cx, |view, cx| {
                                if !coordinator.accepts_output(attempt) {
                                    return false;
                                }
                                delivery.resolve_handoff(cx);
                                if !coordinator.finish_output(attempt, &delivery) {
                                    return false;
                                }
                                if delivery.succeeded() {
                                    view.close_capture(window, cx);
                                } else {
                                    window.refresh();
                                }
                                true
                            })
                            .unwrap_or(false)
                        })
                        .unwrap_or(false)
                })
            });
        })
        .detach();
    }

    fn close_capture(&self, window: &mut Window, cx: &mut App) {
        self.coordinator.close_windows_except(Some(window.window_handle().window_id()), cx);
        window.defer(cx, |window, _| {
            window.remove_window();
        });
    }
}

impl Render for ToolbarWindowView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let snapshot = self.coordinator.snapshot();
        let (toolbar_width, toolbar_height) = panel_layout::toolbar_size(LongCaptureToolbarAction::ORDERED.len());
        let layout = PanelLayout {
            x: 0.0,
            y: TOOLBAR_TOP_RESERVED,
            width: toolbar_width,
            height: toolbar_height,
        };

        let mut action_row = h_flex().items_center().gap_0p5();
        for action in LongCaptureToolbarAction::ORDERED {
            action_row = action_row.child(self.toolbar_button(action, &snapshot, cx));
        }

        let theme = cx.theme();
        let mut root = div()
            .id("long-capture-toolbar")
            .track_focus(&self.focus_handle)
            .size_full()
            .bg(gpui::transparent_black())
            .on_key_down(cx.listener(Self::on_key_down))
            .child(toolbar_panel(cx, layout).child(action_row));

        if snapshot.result == LongCaptureResult::Partial && !snapshot.partial_accepted {
            root = root.child(
                div()
                    .absolute()
                    .left_0()
                    .top(px((WARNING_HEIGHT + 8.0) as f32))
                    .w(px(toolbar_width as f32))
                    .child(
                        Button::new("long-capture-use-partial")
                            .label(i18n::overlay::long_capture_use_partial())
                            .small()
                            .w_full()
                            .disabled(snapshot.busy)
                            .on_click(cx.listener(Self::on_use_partial)),
                    ),
            );
        }

        if let Some(error) = &snapshot.export_error {
            let message = i18n::overlay::long_capture_export_failed(output::error_message(error));
            let tooltip = message.clone();
            let mut failure = h_flex()
                .id("long-capture-output-error")
                .absolute()
                .left_0()
                .top(px((WARNING_HEIGHT + 8.0) as f32))
                .w(px(toolbar_width as f32))
                .h(px(32.0))
                .gap_1()
                .px_2()
                .rounded(theme.radius_lg)
                .bg(theme.danger)
                .text_color(theme.danger_foreground)
                .text_size(px(12.0))
                .tooltip(move |window, cx| Tooltip::new(tooltip.clone()).build(window, cx))
                .child(div().flex_1().min_w_0().line_clamp(2).child(message));
            if snapshot.output_can_retry {
                failure = failure.child(
                    Button::new("long-capture-retry-output")
                        .label(i18n::output::retry())
                        .small()
                        .on_click(cx.listener(|view, _, window, cx| view.retry_output(window, cx))),
                );
            }
            root = root.child(failure);
        }

        if snapshot.busy || snapshot.result != LongCaptureResult::Capturing || !snapshot.warning_text.is_empty() || snapshot.preflight_error.is_some()
        {
            let warning = !snapshot.warning_text.is_empty() || snapshot.capture_error.is_some() || snapshot.preflight_error.is_some();
            let message = if let Some(error) = &snapshot.preflight_error {
                output::error_message(error)
            } else if snapshot.busy {
                if snapshot.output_submitted {
                    i18n::overlay::long_capture_output_continues()
                } else {
                    i18n::overlay::long_capture_processing()
                }
            } else {
                match snapshot.result {
                    LongCaptureResult::Complete => i18n::overlay::long_capture_complete(),
                    LongCaptureResult::Partial if snapshot.partial_accepted => i18n::overlay::long_capture_partial_selected(),
                    LongCaptureResult::Partial => i18n::overlay::long_capture_partial(),
                    LongCaptureResult::Failed => i18n::overlay::long_capture_failed(),
                    LongCaptureResult::Cancelled => i18n::common::cancel(),
                    LongCaptureResult::Capturing => snapshot.warning_text,
                }
            };
            root = root.child(
                div()
                    .absolute()
                    .left(px(0.0))
                    .top(px(0.0))
                    .w(px(toolbar_width as f32))
                    .h(px(WARNING_HEIGHT as f32))
                    .rounded(theme.radius_lg)
                    .bg(if warning { theme.danger } else { theme.popover })
                    .border_1()
                    .border_color(theme.border)
                    .px_2()
                    .py_1()
                    .text_color(if warning { theme.danger_foreground } else { theme.muted_foreground })
                    .text_size(px(12.0))
                    .line_clamp(2)
                    .child(message),
            );
        }

        if let Some(message) = snapshot.window_warning {
            let tooltip = message.clone();
            root = root.child(
                div()
                    .id("long-capture-window-warning")
                    .absolute()
                    .left_0()
                    .top(px(WINDOW_WARNING_TOP as f32))
                    .w(px(toolbar_width as f32))
                    .h(px(WINDOW_WARNING_HEIGHT as f32))
                    .rounded(theme.radius_lg)
                    .bg(theme.danger)
                    .border_1()
                    .border_color(theme.border)
                    .px_2()
                    .py_1()
                    .text_color(theme.danger_foreground)
                    .text_size(px(12.0))
                    .line_clamp(3)
                    .tooltip(move |window, cx| Tooltip::new(tooltip.clone()).build(window, cx))
                    .child(message),
            );
        }

        root
    }
}

fn toolbar_icon(app_ctx: &App, icon_name: LongCaptureToolbarIcon) -> Icon {
    let theme = app_ctx.theme();
    Icon::new(icon_name).small().text_color(theme.popover_foreground)
}

fn toolbar_panel(app_ctx: &App, layout: PanelLayout) -> Div {
    let theme = app_ctx.theme();
    let mut panel = div()
        .absolute()
        .left(px(layout.x as f32))
        .top(px(layout.y as f32))
        .w(px(layout.width as f32))
        .h(px(layout.height as f32))
        .flex()
        .items_center()
        .justify_center()
        .rounded(theme.radius_lg)
        .border_1()
        .border_color(theme.border.alpha(0.82))
        .bg(theme.popover.alpha(0.98))
        .overflow_hidden()
        .px_2()
        .py_1()
        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_mouse_down(gpui::MouseButton::Middle, |_, _, cx| cx.stop_propagation())
        .on_mouse_down(gpui::MouseButton::Right, |_, _, cx| cx.stop_propagation());
    if theme.shadow {
        panel = panel.shadow_lg();
    }
    panel
}
