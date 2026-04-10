use super::{OverlayEffect, OverlayHandle, OverlayOutcome};
use crate::features::long_capture::{self, LongCaptureRequest};
use crate::features::pin::{self, PinRequest};
use gpui::{App, Window};
use minnow_core::capture::action::{ActionResult, CaptureAction};
use minnow_core::i18n;
use minnow_core::io::clipboard::copy_text_to_clipboard;
use minnow_core::notify::NotificationType;

struct CopyTextPayload {
    text: String,
    title: String,
    message: String,
    notification_type: NotificationType,
    close_on_success: bool,
}

impl OverlayHandle {
    pub(crate) fn dispatch(&self, command: crate::features::overlay::state::OverlayCommand, window: &mut Window, cx: &mut App) {
        self.sync_viewport(window, cx);
        let outcome = self.0.update(cx, |session, _| session.apply(command));
        self.run_outcome(outcome, window, cx);
    }

    pub(crate) fn prepare_frame(&self, window: &Window, cx: &mut App) -> crate::features::overlay::state::OverlayFrame {
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
        selection_rect: minnow_core::geometry::Rect,
        viewport_rect: minnow_core::geometry::RectF,
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
        if copy_text_to_clipboard(payload.text) {
            minnow_core::notify::show(&payload.title, &payload.message, payload.notification_type);
            if payload.close_on_success {
                self.close(window, cx);
            }
            return;
        }

        self.refresh(window, cx);
    }

    fn capture(&self, action: CaptureAction, context: minnow_core::capture::action::ActionContext, window: &mut Window, cx: &mut App) {
        match action.execute(context) {
            ActionResult::Copied => {
                minnow_core::notify::show(
                    i18n::app::capture_name().as_str(),
                    i18n::notify::copied_image().as_str(),
                    NotificationType::Copy,
                );
                self.close(window, cx);
            }
            ActionResult::ColorPicked(color) => {
                self.copy_text(
                    CopyTextPayload {
                        text: color.clone(),
                        title: i18n::app::capture_name(),
                        message: format!("Color copied: {color}"),
                        notification_type: NotificationType::Copy,
                        close_on_success: true,
                    },
                    window,
                    cx,
                );
            }
            ActionResult::Saved(path) => {
                minnow_core::notify::show(
                    i18n::app::capture_name().as_str(),
                    i18n::notify::saved_image(path).as_str(),
                    NotificationType::Save,
                );
                self.close(window, cx);
            }
            ActionResult::PinRequested(path, source_bounds, auto_ocr) => {
                let request = PinRequest::new(path, Some(source_bounds), auto_ocr);
                cx.defer(move |cx| {
                    pin::open_window(cx, request);
                });
                self.close(window, cx);
            }
            ActionResult::OcrResult(content) => {
                self.copy_text(
                    CopyTextPayload {
                        text: content,
                        title: i18n::app::capture_name(),
                        message: i18n::notify::copied_qr(),
                        notification_type: NotificationType::Copy,
                        close_on_success: true,
                    },
                    window,
                    cx,
                );
            }
            ActionResult::NoOp => {
                if matches!(action, CaptureAction::QrCode) {
                    minnow_core::notify::show(i18n::app::name().as_str(), i18n::overlay::qr_not_found().as_str(), NotificationType::Info);
                }
                self.refresh(window, cx);
            }
            ActionResult::Error(err) => {
                tracing::error!("Action error: {err}");
                if matches!(action, CaptureAction::QrCode) {
                    minnow_core::notify::show(i18n::app::name().as_str(), i18n::overlay::qr_not_found().as_str(), NotificationType::Info);
                }
                self.refresh(window, cx);
            }
        }
    }
}
