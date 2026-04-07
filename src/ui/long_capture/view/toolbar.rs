use crate::core::capture::action::{ActionContext, ActionResult, CaptureAction};
use crate::core::capture::service::CaptureService;
use crate::core::i18n;
use crate::core::notify::NotificationType;
use crate::ui::long_capture::coordinator::LongCaptureCoordinator;
use crate::ui::long_capture::layout::TOOLBAR_TOP_RESERVED;
use crate::ui::overlay::render::layout::{OverlayPanelLayout, toolbar_size};
use crate::ui::overlay::render::toolbar::{ToolbarIcon, toolbar_icon, toolbar_panel};
use crate::ui::pin::{self, PinRequest};
use gpui::InteractiveElement;
use gpui::{ClickEvent, Context, FocusHandle, IntoElement, KeyDownEvent, ParentElement, Render, Styled, Window, div, px};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::{ActiveTheme as _, Disableable, h_flex};
use std::sync::Arc;
use std::time::Duration;

const WARNING_HEIGHT: f64 = 34.0;
const CAPTURE_ACTION_TIMEOUT: Duration = Duration::from_millis(260);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LongCaptureToolbarAction {
    Save,
    Pin,
    Copy,
    Cancel,
}

impl LongCaptureToolbarAction {
    pub(crate) const ORDERED: [Self; 4] = [Self::Save, Self::Pin, Self::Copy, Self::Cancel];

    fn id(self) -> &'static str {
        match self {
            Self::Save => "long-capture-save",
            Self::Pin => "long-capture-pin",
            Self::Copy => "long-capture-copy",
            Self::Cancel => "long-capture-cancel",
        }
    }

    fn icon(self) -> ToolbarIcon {
        match self {
            Self::Save => ToolbarIcon::Save,
            Self::Pin => ToolbarIcon::Pin,
            Self::Copy => ToolbarIcon::Copy,
            Self::Cancel => ToolbarIcon::Cancel,
        }
    }

    fn tooltip(self) -> String {
        match self {
            Self::Save => i18n::common::save(),
            Self::Pin => i18n::common::pin(),
            Self::Copy => i18n::common::copy(),
            Self::Cancel => i18n::common::cancel(),
        }
    }

    fn disabled_when_busy(self) -> bool {
        self != Self::Cancel
    }
}

pub(crate) struct ToolbarWindowView {
    coordinator: Arc<LongCaptureCoordinator>,
    focus_handle: FocusHandle,
}

impl ToolbarWindowView {
    pub(crate) fn new(coordinator: Arc<LongCaptureCoordinator>, focus_handle: FocusHandle, window: &mut Window, cx: &mut Context<Self>) -> Self {
        coordinator.ensure_runtime_poller(window, cx);
        Self { coordinator, focus_handle }
    }

    fn toolbar_button(&self, action: LongCaptureToolbarAction, busy: bool, cx: &mut Context<Self>) -> Button {
        let button = Button::new(action.id())
            .compact()
            .icon(toolbar_icon(cx, action.icon()))
            .tooltip(action.tooltip())
            .ghost()
            .disabled(busy && action.disabled_when_busy());

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
        self.execute_capture_action(CaptureAction::Save, window, cx);
    }

    fn on_pin(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.execute_capture_action(CaptureAction::Pin, window, cx);
    }

    fn on_copy(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.execute_capture_action(CaptureAction::Copy, window, cx);
    }

    fn on_cancel(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.cancel(window, cx);
    }

    fn close_capture_windows(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.coordinator.close_windows_except(Some(window.window_handle().window_id()), cx);
        window.defer(cx, |window, _| {
            window.remove_window();
        });
    }

    fn handle_capture_action_result(&mut self, result: ActionResult, window: &mut Window, cx: &mut Context<Self>) {
        match result {
            ActionResult::Copied => {
                crate::core::notify::show(
                    i18n::app::capture_name().as_str(),
                    i18n::notify::copied_image().as_str(),
                    NotificationType::Copy,
                );
                self.close_capture_windows(window, cx);
            }
            ActionResult::Saved(path) => {
                crate::core::notify::show(
                    i18n::app::capture_name().as_str(),
                    i18n::notify::saved_image(path).as_str(),
                    NotificationType::Save,
                );
                self.close_capture_windows(window, cx);
            }
            ActionResult::PinRequested(path, source_bounds, auto_ocr) => {
                let request = PinRequest::new(path, Some(source_bounds), auto_ocr);
                cx.defer(move |cx| {
                    pin::open_window(cx, request);
                });
                self.close_capture_windows(window, cx);
            }
            ActionResult::Error(err) => {
                self.coordinator.finish_capture_action_with_warning(err);
                cx.notify();
            }
            _ => {
                self.coordinator.finish_capture_action_with_warning(i18n::overlay::action_unavailable());
                cx.notify();
            }
        }
    }

    fn execute_capture_action(&mut self, action: CaptureAction, window: &mut Window, cx: &mut Context<Self>) {
        if self.coordinator.snapshot().busy {
            return;
        }

        self.coordinator.start_capture_action();

        let image = self.coordinator.take_capture_image(CAPTURE_ACTION_TIMEOUT);
        let Some(image) = image else {
            self.coordinator.finish_capture_action_with_warning(i18n::overlay::long_capture_empty());
            cx.notify();
            return;
        };

        let Some(temp_path) = CaptureService::save_temp(&image) else {
            self.coordinator.finish_capture_action_with_warning(i18n::capture::copy_failed());
            cx.notify();
            return;
        };

        self.handle_capture_action_result(action.execute(ActionContext::full_image(temp_path)), window, cx);
    }
}

impl Render for ToolbarWindowView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let snapshot = self.coordinator.snapshot();
        let (toolbar_width, toolbar_height) = toolbar_size(LongCaptureToolbarAction::ORDERED.len());
        let layout = OverlayPanelLayout {
            x: 0.0,
            y: TOOLBAR_TOP_RESERVED,
            width: toolbar_width,
            height: toolbar_height,
        };

        let mut action_row = h_flex().items_center().gap_0p5();
        for action in LongCaptureToolbarAction::ORDERED {
            action_row = action_row.child(self.toolbar_button(action, snapshot.busy, cx));
        }

        let theme = cx.theme();
        let mut root = div()
            .id("long-capture-toolbar")
            .track_focus(&self.focus_handle)
            .size_full()
            .bg(gpui::transparent_black())
            .on_key_down(cx.listener(Self::on_key_down))
            .child(toolbar_panel(cx, layout).child(action_row));

        if snapshot.busy {
            root = root.child(
                div()
                    .absolute()
                    .left(px(0.0))
                    .top(px(0.0))
                    .w(px(toolbar_width as f32))
                    .h(px(WARNING_HEIGHT as f32))
                    .rounded(theme.radius_lg)
                    .bg(theme.popover)
                    .border_1()
                    .border_color(theme.border)
                    .px_2()
                    .py_1()
                    .text_color(theme.muted_foreground)
                    .child(i18n::overlay::long_capture_processing()),
            );
        }

        root
    }
}
