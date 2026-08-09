use super::OverlayHandle;
use crate::app::workflows;
use crate::platform::shell::{self, NotificationType};
use crate::services::capture::action::{ActionContext, CaptureAction};
use crate::services::geometry::{Rect, RectF};
use crate::ui::features::long_capture::{self, LongCaptureRequest};
use crate::ui::features::pin::{self, PinRequest};
use crate::ui::support::capture_actions::{self, CaptureActionHost, CaptureActionHostKind};
use gpui::{App, Window};

pub(crate) enum OverlayEffect {
    Refresh,
    Close,
    StartLongCapture {
        selection_rect: Rect,
        viewport_rect: RectF,
        viewport_scale: f64,
    },
    Capture {
        action: CaptureAction,
        context: ActionContext,
    },
    CopyText {
        text: String,
        title: String,
        message: String,
        notification_type: NotificationType,
        close_on_success: bool,
    },
}

#[derive(Default)]
pub(crate) struct OverlayOutcome {
    pub(super) effects: Vec<OverlayEffect>,
}

impl OverlayOutcome {
    pub(super) fn push(&mut self, effect: OverlayEffect) {
        self.effects.push(effect);
    }

    pub(super) fn refresh() -> Self {
        let mut outcome = Self::default();
        outcome.push(OverlayEffect::Refresh);
        outcome
    }

    pub(super) fn with_effect(effect: OverlayEffect) -> Self {
        let mut outcome = Self::default();
        outcome.push(effect);
        outcome
    }
}

struct CopyTextPayload {
    text: String,
    title: String,
    message: String,
    notification_type: NotificationType,
    close_on_success: bool,
}

impl OverlayHandle {
    pub(crate) fn dispatch(&self, command: crate::ui::features::overlay::state::OverlayCommand, window: &mut Window, cx: &mut App) {
        self.sync_viewport(window, cx);
        let outcome = self.0.update(cx, |session, _| session.apply(command));
        self.run_outcome(outcome, window, cx);
    }

    pub(crate) fn prepare_frame(&self, window: &Window, cx: &mut App) -> crate::ui::features::overlay::state::OverlayFrame {
        self.sync_viewport(window, cx);
        self.0.update(cx, |session, _| {
            let _ = session.apply_pending_pointer();
            session.diag_on_render();
            session.frame()
        })
    }

    fn run_outcome(&self, outcome: OverlayOutcome, window: &mut Window, cx: &mut App) {
        for effect in outcome.effects {
            self.run_effect(effect, window, cx);
        }
    }

    fn run_effect(&self, effect: OverlayEffect, window: &mut Window, cx: &mut App) {
        match effect {
            OverlayEffect::Refresh => self.refresh(window, cx),
            OverlayEffect::Close => self.close(window, cx),
            OverlayEffect::StartLongCapture {
                selection_rect,
                viewport_rect,
                viewport_scale,
            } => self.start_long_capture(selection_rect, viewport_rect, viewport_scale, window, cx),
            OverlayEffect::CopyText {
                text,
                title,
                message,
                notification_type,
                close_on_success,
            } => self.copy_text(
                CopyTextPayload {
                    text,
                    title,
                    message,
                    notification_type,
                    close_on_success,
                },
                window,
                cx,
            ),
            OverlayEffect::Capture { action, context } => self.capture(action, context, window, cx),
        }
    }

    fn refresh(&self, window: &mut Window, cx: &mut App) {
        self.0.update(cx, |session, _| session.diag_on_refresh());
        window.refresh();
    }

    fn close(&self, window: &mut Window, cx: &mut App) {
        self.0.update(cx, |session, _| session.clear());
        window.defer(cx, |window, _| {
            window.remove_window();
        });
    }

    fn start_long_capture(
        &self,
        selection_rect: crate::services::geometry::Rect,
        viewport_rect: crate::services::geometry::RectF,
        viewport_scale: f64,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.0.update(cx, |session, _| session.clear());
        let bounds = window.window_bounds().get_bounds();
        let request = LongCaptureRequest {
            selection_rect,
            viewport_rect,
            viewport_scale,
            viewport_origin_screen: (bounds.origin.x.to_f64(), bounds.origin.y.to_f64()),
        };
        cx.defer(move |cx| {
            long_capture::open_window(cx, request);
        });
        self.close(window, cx);
    }

    fn copy_text(&self, payload: CopyTextPayload, window: &mut Window, cx: &mut App) {
        if shell::copy_text_to_clipboard(payload.text) {
            shell::show_notification(&payload.title, &payload.message, payload.notification_type);
            if payload.close_on_success {
                self.close(window, cx);
            }
            return;
        }

        self.refresh(window, cx);
    }

    fn capture(&self, action: CaptureAction, context: crate::services::capture::action::ActionContext, window: &mut Window, cx: &mut App) {
        let result = workflows::execute_capture_action(action, context);
        let effect = capture_actions::interpret(action, result, CaptureActionHostKind::Overlay);
        capture_actions::apply_host_effect(self, effect, window, cx);
    }
}

impl CaptureActionHost for OverlayHandle {
    fn close_capture(&self, window: &mut Window, cx: &mut App) {
        self.close(window, cx);
    }

    fn refresh_capture(&self, window: &mut Window, cx: &mut App) {
        self.refresh(window, cx);
    }

    fn open_pin(&self, request: PinRequest, cx: &mut App) {
        cx.defer(move |cx| {
            pin::open_window(cx, request);
        });
    }

    fn show_warning(&self, message: String, _window: &mut Window, _cx: &mut App) {
        tracing::warn!("Capture action warning: {message}");
    }
}
