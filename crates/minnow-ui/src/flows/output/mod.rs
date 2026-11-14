//! Image and text output shared by capture features.
mod completion;
mod state;

use crate::presentation::feedback::{self, NotificationType};
use crate::presentation::i18n;
pub(crate) use completion::{OutputCompletion, OutputDelivery};
use gpui::App;
use minnow_application::{ApplicationServices, ErrorKind, OperationError, OperationResult};
use minnow_application::{ImageCommand, ImageCommandResult, ImageInput, PreparedImage, SaveDestination};
pub(crate) use state::OutputState;
use std::sync::Arc;

#[derive(Debug)]
pub(crate) struct NotificationSpec {
    title: String,
    message: String,
    kind: NotificationType,
}

#[derive(Debug)]
pub(crate) enum CaptureActionEffect {
    Completed(NotificationSpec),
    OpenPin { prepared: PreparedImage, auto_ocr: bool },
    Warning(OperationError),
}

/// UI intents include window transitions that are not image-domain commands.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum CaptureIntent {
    Copy,
    Save(SaveDestination),
    OpenPin,
    Ocr,
    Scroll,
    CopyQr,
    CopyColor,
}

impl CaptureIntent {
    pub(crate) fn output(self, input: ImageInput, auto_ocr: bool) -> OperationResult<OutputRequest> {
        let command = match self {
            Self::Copy => ImageCommand::Copy,
            Self::Save(destination) => ImageCommand::Save { destination },
            Self::CopyQr => ImageCommand::CopyQr,
            Self::CopyColor => ImageCommand::CopyColor,
            Self::OpenPin => return Ok(OutputRequest::OpenPin { input, auto_ocr }),
            Self::Ocr => return Ok(OutputRequest::OpenPin { input, auto_ocr: true }),
            Self::Scroll => return Err(OperationError::new(ErrorKind::InvalidInput, i18n::overlay::action_unavailable())),
        };
        Ok(OutputRequest::Image { command, input })
    }
}

#[derive(Clone, Debug)]
pub(crate) enum OutputRequest {
    Image { command: ImageCommand, input: ImageInput },
    OpenPin { input: ImageInput, auto_ocr: bool },
    Text { text: String, message: String },
}

impl OutputRequest {
    pub(crate) fn scope(&self) -> minnow_application::JobScope {
        self.input().map_or(minnow_application::JobScope::Application, ImageInput::scope)
    }

    pub(crate) fn input(&self) -> Option<&ImageInput> {
        match self {
            Self::Image { input, .. } | Self::OpenPin { input, .. } => Some(input),
            Self::Text { .. } => None,
        }
    }

    pub(crate) fn preserves_output(&self) -> bool {
        !matches!(self, Self::OpenPin { .. })
    }
}

impl CaptureActionEffect {
    /// External output failures still belong to the requesting user after its view changes.
    pub(crate) fn preserved_failure(&self, preserve: bool) -> Option<&OperationError> {
        match (self, preserve) {
            (Self::Warning(error), true) => Some(error),
            _ => None,
        }
    }
}

fn completed(message: String, kind: NotificationType) -> CaptureActionEffect {
    CaptureActionEffect::Completed(NotificationSpec {
        title: i18n::app::capture_name(),
        message,
        kind,
    })
}

pub(crate) fn interpret(result: OperationResult<ImageCommandResult>) -> CaptureActionEffect {
    match result {
        Ok(ImageCommandResult::Copied) => completed(i18n::notify::copied_image(), NotificationType::Copy),
        Ok(ImageCommandResult::Saved(receipt)) => completed(i18n::notify::saved_image(receipt.path.to_string_lossy()), NotificationType::Save),
        Ok(ImageCommandResult::QrCopied(_)) => completed(i18n::notify::copied_qr(), NotificationType::QrCode),
        Ok(ImageCommandResult::ColorCopied(_)) => completed(i18n::notify::copied_text(), NotificationType::Copy),
        Err(error) => CaptureActionEffect::Warning(error),
    }
}

async fn execute_request(services: Arc<ApplicationServices>, request: OutputRequest) -> CaptureActionEffect {
    match request {
        OutputRequest::Image { command, input } => interpret(services.execute_image_command(command, input).await),
        OutputRequest::OpenPin { input, auto_ocr } => {
            let scope = input.scope();
            match services.prepare_image(input).await {
                Ok(prepared) if services.is_current(scope) => CaptureActionEffect::OpenPin { prepared, auto_ocr },
                Ok(_) => CaptureActionEffect::Warning(OperationError::new(ErrorKind::Conflict, i18n::overlay::image_changed())),
                Err(error) => CaptureActionEffect::Warning(error),
            }
        }
        OutputRequest::Text { text, message } => match services.copy_text(text).await {
            Ok(()) => completed(message, NotificationType::Copy),
            Err(error) => CaptureActionEffect::Warning(error),
        },
    }
}

fn notify_preserved_failure(error: &OperationError, cx: &mut App) {
    feedback::show_notification(&i18n::app::capture_name(), &error.to_string(), NotificationType::Info, cx);
}

pub(crate) fn error_message(error: &OperationError) -> String {
    if error.kind == ErrorKind::Busy {
        i18n::output::busy()
    } else {
        i18n::output::failed(error)
    }
}

/// Committed output is acknowledged even after its source window has closed.
fn notify_committed(effect: &CaptureActionEffect, cx: &mut App) {
    if let CaptureActionEffect::Completed(notification) = effect {
        feedback::show_notification(&notification.title, &notification.message, notification.kind, cx);
    }
}

/// Execute one admitted request; completion owns delivery and detached feedback rules.
pub(crate) async fn execute(services: Arc<ApplicationServices>, request: OutputRequest) -> OutputCompletion {
    let scope = request.scope();
    let preserve = request.preserves_output();
    let effect = execute_request(services.clone(), request).await;
    OutputCompletion::new(services, scope, preserve, effect)
}
