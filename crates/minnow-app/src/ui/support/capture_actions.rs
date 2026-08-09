use crate::platform::shell::{self, NotificationType};
use crate::services::capture::action::{ActionResult, CaptureAction, PinCaptureRequest};
use crate::services::i18n;
use gpui::{App, Window};

#[derive(Debug)]
pub(crate) struct NotificationSpec {
    pub(crate) title: String,
    pub(crate) message: String,
    pub(crate) kind: NotificationType,
}

impl NotificationSpec {
    fn new(title: String, message: String, kind: NotificationType) -> Self {
        Self { title, message, kind }
    }
}

#[derive(Debug)]
pub(crate) enum CaptureActionEffect {
    NotifyAndClose(NotificationSpec),
    CopyText {
        text: String,
        notification: NotificationSpec,
        close_on_success: bool,
    },
    OpenPin(PinCaptureRequest),
    Refresh {
        notification: Option<NotificationSpec>,
        error: Option<String>,
    },
    NotifyOnly(NotificationSpec),
    Warning(String),
    LogError(String),
    NoOp,
}

#[derive(Clone, Copy)]
pub(crate) enum CaptureActionHostKind {
    Overlay,
    LongCapture,
    Pin,
}

pub(crate) trait CaptureActionHost {
    fn close_capture(&self, window: &mut Window, cx: &mut App);
    fn refresh_capture(&self, window: &mut Window, cx: &mut App);
    fn open_pin(&self, request: PinCaptureRequest, cx: &mut App);
    fn show_warning(&self, message: String, window: &mut Window, cx: &mut App);
}

pub(crate) fn interpret(action: CaptureAction, result: ActionResult, host: CaptureActionHostKind) -> CaptureActionEffect {
    match host {
        CaptureActionHostKind::Overlay => interpret_overlay(action, result),
        CaptureActionHostKind::LongCapture => interpret_long_capture(action, result),
        CaptureActionHostKind::Pin => interpret_pin(result),
    }
}

pub(crate) fn apply_host_effect<H: CaptureActionHost>(host: &H, effect: CaptureActionEffect, window: &mut Window, cx: &mut App) {
    match effect {
        CaptureActionEffect::NotifyAndClose(notification) => {
            show_notification(notification);
            host.close_capture(window, cx);
        }
        CaptureActionEffect::CopyText {
            text,
            notification,
            close_on_success,
        } => {
            if shell::copy_text_to_clipboard(text) {
                show_notification(notification);
                if close_on_success {
                    host.close_capture(window, cx);
                }
            } else {
                host.refresh_capture(window, cx);
            }
        }
        CaptureActionEffect::OpenPin(request) => {
            host.open_pin(request, cx);
            host.close_capture(window, cx);
        }
        CaptureActionEffect::Refresh { notification, error } => {
            if let Some(error) = error {
                tracing::error!("Capture action error: {error}");
            }
            if let Some(notification) = notification {
                show_notification(notification);
            }
            host.refresh_capture(window, cx);
        }
        CaptureActionEffect::Warning(message) => host.show_warning(message, window, cx),
        CaptureActionEffect::LogError(error) => tracing::error!("Capture action error: {error}"),
        CaptureActionEffect::NotifyOnly(notification) => show_notification(notification),
        CaptureActionEffect::NoOp => {}
    }
}

pub(crate) fn apply_pin_effect(effect: CaptureActionEffect) {
    match effect {
        CaptureActionEffect::NotifyOnly(notification) => show_notification(notification),
        CaptureActionEffect::LogError(error) => tracing::error!("Pin capture action error: {error}"),
        CaptureActionEffect::Refresh { notification, error } => {
            if let Some(error) = error {
                tracing::error!("Pin capture action error: {error}");
            }
            if let Some(notification) = notification {
                show_notification(notification);
            }
        }
        CaptureActionEffect::NoOp => {}
        _ => {}
    }
}

fn interpret_overlay(action: CaptureAction, result: ActionResult) -> CaptureActionEffect {
    match result {
        ActionResult::Copied => CaptureActionEffect::NotifyAndClose(NotificationSpec::new(
            i18n::app::capture_name(),
            i18n::notify::copied_image(),
            NotificationType::Copy,
        )),
        ActionResult::ColorPicked(color) => CaptureActionEffect::CopyText {
            text: color.clone(),
            notification: NotificationSpec::new(i18n::app::capture_name(), format!("Color copied: {color}"), NotificationType::Copy),
            close_on_success: true,
        },
        ActionResult::Saved(path) => CaptureActionEffect::NotifyAndClose(NotificationSpec::new(
            i18n::app::capture_name(),
            i18n::notify::saved_image(path),
            NotificationType::Save,
        )),
        ActionResult::PinRequested(request) => CaptureActionEffect::OpenPin(request),
        ActionResult::OcrResult(content) => CaptureActionEffect::CopyText {
            text: content,
            notification: NotificationSpec::new(i18n::app::capture_name(), i18n::notify::copied_qr(), NotificationType::Copy),
            close_on_success: true,
        },
        ActionResult::NoOp => CaptureActionEffect::Refresh {
            notification: qr_notification(action),
            error: None,
        },
        ActionResult::Error(error) => CaptureActionEffect::Refresh {
            notification: qr_notification(action),
            error: Some(error),
        },
    }
}

fn interpret_long_capture(_action: CaptureAction, result: ActionResult) -> CaptureActionEffect {
    match result {
        ActionResult::Copied => CaptureActionEffect::NotifyAndClose(NotificationSpec::new(
            i18n::app::capture_name(),
            i18n::notify::copied_image(),
            NotificationType::Copy,
        )),
        ActionResult::Saved(path) => CaptureActionEffect::NotifyAndClose(NotificationSpec::new(
            i18n::app::capture_name(),
            i18n::notify::saved_image(path),
            NotificationType::Save,
        )),
        ActionResult::PinRequested(request) => CaptureActionEffect::OpenPin(request),
        ActionResult::Error(error) => CaptureActionEffect::Warning(error),
        _ => CaptureActionEffect::Warning(i18n::overlay::action_unavailable()),
    }
}

fn interpret_pin(result: ActionResult) -> CaptureActionEffect {
    match result {
        ActionResult::Copied => CaptureActionEffect::NotifyOnly(NotificationSpec::new(
            i18n::app::capture_name(),
            i18n::notify::copied_image(),
            NotificationType::Copy,
        )),
        ActionResult::Saved(path) => CaptureActionEffect::NotifyOnly(NotificationSpec::new(
            i18n::app::capture_name(),
            i18n::notify::saved_image(path),
            NotificationType::Save,
        )),
        ActionResult::Error(error) => CaptureActionEffect::LogError(error),
        _ => CaptureActionEffect::NoOp,
    }
}

fn qr_notification(action: CaptureAction) -> Option<NotificationSpec> {
    matches!(action, CaptureAction::QrCode).then(|| NotificationSpec::new(i18n::app::name(), i18n::overlay::qr_not_found(), NotificationType::Info))
}

fn show_notification(notification: NotificationSpec) {
    shell::show_notification(&notification.title, &notification.message, notification.kind);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_qr_error_keeps_diagnostics_and_user_feedback() {
        let effect = interpret(
            CaptureAction::QrCode,
            ActionResult::Error("decode failed".to_string()),
            CaptureActionHostKind::Overlay,
        );

        let CaptureActionEffect::Refresh { notification, error } = effect else {
            panic!("QR error should refresh the overlay");
        };
        assert_eq!(error.as_deref(), Some("decode failed"));
        assert_eq!(notification.map(|item| item.kind), Some(NotificationType::Info));
    }

    #[test]
    fn long_capture_rejects_unsupported_results_with_warning() {
        let effect = interpret(CaptureAction::Copy, ActionResult::NoOp, CaptureActionHostKind::LongCapture);
        assert!(matches!(effect, CaptureActionEffect::Warning(_)));
    }

    #[test]
    fn pin_copy_result_only_notifies() {
        let effect = interpret(CaptureAction::Copy, ActionResult::Copied, CaptureActionHostKind::Pin);
        assert!(matches!(effect, CaptureActionEffect::NotifyOnly(_)));
    }
}
