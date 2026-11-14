use super::{CaptureActionEffect, OutputRequest};
use minnow_application::{ImageInput, OperationError};

/// A source owns at most one pending request and one frozen retry input.
#[derive(Clone, Debug, Default)]
pub(crate) struct OutputState {
    sequence: u64,
    pending: Option<u64>,
    retry: Option<OutputRequest>,
    request_error: Option<OperationError>,
    preflight_error: Option<OperationError>,
}

impl OutputState {
    pub(crate) fn begin(&mut self, request: &OutputRequest) -> Option<u64> {
        if self.pending.is_some() {
            return None;
        }
        self.sequence = self.sequence.saturating_add(1);
        self.pending = Some(self.sequence);
        self.retry = Some(request.clone());
        self.request_error = None;
        self.preflight_error = None;
        self.pending
    }

    pub(crate) fn finish(&mut self, attempt: u64, effect: &CaptureActionEffect) -> bool {
        if self.pending != Some(attempt) {
            return false;
        }
        self.pending = None;
        // Only this request's result settles here; later preflight feedback survives.
        self.request_error = match effect {
            CaptureActionEffect::Warning(error) => Some(error.clone()),
            CaptureActionEffect::Completed(_) | CaptureActionEffect::OpenPin { .. } => {
                self.retry = None;
                None
            }
        };
        true
    }

    pub(crate) fn is_pending(&self) -> bool {
        self.pending.is_some()
    }

    pub(crate) fn is_pending_attempt(&self, attempt: u64) -> bool {
        self.pending == Some(attempt)
    }

    pub(crate) fn discard_stale_derived_retry(&mut self, is_current: impl FnOnce(&ImageInput) -> bool) -> bool {
        if let Some(OutputRequest::OpenPin { input, .. }) = &self.retry
            && !is_current(input)
        {
            self.retry = None;
            return true;
        }
        false
    }

    #[cfg(test)]
    pub(crate) fn error(&self) -> Option<&OperationError> {
        self.preflight_error.as_ref().or(self.request_error.as_ref())
    }

    pub(crate) fn request_error(&self) -> Option<&OperationError> {
        self.request_error.as_ref()
    }

    pub(crate) fn preflight_error(&self) -> Option<&OperationError> {
        self.preflight_error.as_ref()
    }

    pub(crate) fn retry(&self) -> Option<OutputRequest> {
        self.can_retry().then(|| self.retry.clone()).flatten()
    }

    pub(crate) fn can_retry(&self) -> bool {
        !self.is_pending() && self.request_error.is_some() && self.retry.is_some()
    }

    pub(crate) fn show_error(&mut self, error: OperationError) {
        // A finished request must not be offered beside an unrelated error.
        // A pending request still owns its input until its own completion.
        if !self.is_pending() {
            self.retry = None;
            self.request_error = None;
        }
        self.preflight_error = Some(error);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flows::output::interpret;
    use minnow_application::{ErrorKind, OperationError};
    use minnow_application::{ImageCommand, ImageCommandResult, SaveDestination};

    fn image_request(command: ImageCommand) -> OutputRequest {
        OutputRequest::Image {
            command,
            input: test_input(),
        }
    }

    fn test_input() -> ImageInput {
        ImageInput::image(minnow_imaging::ImageAsset::new(image::RgbaImage::new(1, 1)), None).unwrap()
    }

    #[test]
    fn failure_remains_a_warning_for_retry() {
        let effect = interpret(Err(OperationError::new(ErrorKind::Io, "disk full")));
        assert!(matches!(effect, CaptureActionEffect::Warning(error) if error.kind == ErrorKind::Io && error.message == "disk full"));
    }
    #[test]
    fn external_output_failures_keep_feedback_but_obsolete_derived_views_do_not() {
        let failure = interpret(Err(OperationError::new(ErrorKind::Io, "disk full")));
        assert_eq!(failure.preserved_failure(true).map(|error| error.message.as_str()), Some("disk full"));
        assert_eq!(failure.preserved_failure(true).map(|error| error.message.as_str()), Some("disk full"));
        assert_eq!(failure.preserved_failure(false), None);
        assert_eq!(interpret(Ok(ImageCommandResult::Copied)).preserved_failure(true), None);
    }

    #[test]
    fn busy_remains_a_typed_retryable_local_failure() {
        let request = image_request(ImageCommand::Copy);
        let mut state = OutputState::default();
        let attempt = state.begin(&request).unwrap();
        let failure = interpret(Err(OperationError::new(ErrorKind::Busy, "output admission is occupied")));
        state.finish(attempt, &failure);
        assert_eq!(state.error().map(|error| error.kind), Some(ErrorKind::Busy));
        assert!(state.can_retry());
        assert!(!state.is_pending());
    }

    #[test]
    fn obsolete_derived_retries_are_removed_both_before_and_after_completion() {
        for auto_ocr in [false, true] {
            let request = OutputRequest::OpenPin {
                input: test_input(),
                auto_ocr,
            };
            let mut state = OutputState::default();
            let attempt = state.begin(&request).unwrap();
            state.finish(attempt, &interpret(Err(OperationError::new(ErrorKind::Busy, "occupied"))));
            assert!(state.can_retry(), "a current Busy request can be retried");
            assert!(!state.discard_stale_derived_retry(|_| true));
            assert!(state.discard_stale_derived_retry(|_| false), "an edit expires the frozen derived input");
            assert!(state.retry().is_none());

            let mut pending = OutputState::default();
            let attempt = pending.begin(&request).unwrap();
            pending.discard_stale_derived_retry(|_| false);
            pending.finish(attempt, &interpret(Err(OperationError::new(ErrorKind::Conflict, "image changed"))));
            assert!(!pending.is_pending());
            assert!(pending.retry().is_none(), "completion cannot restore a retry removed by an edit");
        }
        for request in [
            image_request(ImageCommand::Copy),
            image_request(ImageCommand::Save {
                destination: SaveDestination::Preferences,
            }),
            OutputRequest::Text {
                text: "frozen A".into(),
                message: "copied".into(),
            },
        ] {
            let mut state = OutputState::default();
            let attempt = state.begin(&request).unwrap();
            state.finish(attempt, &interpret(Err(OperationError::new(ErrorKind::Busy, "occupied"))));
            assert!(!state.discard_stale_derived_retry(|_| false));
            assert!(state.can_retry(), "external output keeps its original snapshot after edits");
        }
    }

    #[test]
    fn preflight_errors_do_not_retry_old_copy_but_handoff_errors_retry_the_current_pin() {
        let copy_a = OutputRequest::Text {
            text: "A".into(),
            message: "copied".into(),
        };
        let mut state = OutputState::default();
        let attempt = state.begin(&copy_a).unwrap();
        state.finish(attempt, &interpret(Err(OperationError::new(ErrorKind::Io, "copy A failed"))));
        assert!(state.can_retry());
        state.show_error(OperationError::new(ErrorKind::Native, "scroll preparation failed"));
        assert_eq!(state.error().map(|error| error.message.as_str()), Some("scroll preparation failed"));
        assert!(state.request_error().is_none());
        assert!(state.retry().is_none(), "the scroll error must never offer a retry of copy A");

        let pin = OutputRequest::OpenPin {
            input: test_input(),
            auto_ocr: false,
        };
        let attempt = state.begin(&pin).unwrap();
        // The navigation result settles the current output, without passing
        // through the unrelated-preflight error path.
        state.finish(
            attempt,
            &interpret(Err(OperationError::new(ErrorKind::Native, "pin window failed to open"))),
        );
        assert!(matches!(state.retry(), Some(OutputRequest::OpenPin { .. })));
    }

    #[test]
    fn a_pending_copy_settles_its_own_result_without_consuming_later_preflight_feedback() {
        for copy_fails in [false, true] {
            let request = OutputRequest::Text {
                text: "frozen A".into(),
                message: "copied".into(),
            };
            let mut state = OutputState::default();
            let attempt = state.begin(&request).unwrap();
            let font_error = OperationError::new(ErrorKind::Unavailable, "text font unavailable");
            state.show_error(font_error.clone());
            assert!(state.is_pending());
            assert_eq!(state.preflight_error(), Some(&font_error));
            assert!(state.request_error().is_none(), "the font failure does not belong to Copy");
            assert!(!state.can_retry(), "a pending request cannot be retried");

            let result = if copy_fails {
                interpret(Err(OperationError::new(ErrorKind::Io, "clipboard unavailable")))
            } else {
                interpret(Ok(ImageCommandResult::Copied))
            };
            assert!(state.finish(attempt, &result));
            assert_eq!(state.preflight_error(), Some(&font_error), "completion cannot swallow a later tool error");
            if copy_fails {
                assert_eq!(state.request_error().map(|error| error.kind), Some(ErrorKind::Io));
                assert!(matches!(state.retry(), Some(OutputRequest::Text { text, .. }) if text == "frozen A"));
            } else {
                assert!(state.request_error().is_none());
                assert!(state.retry().is_none());
            }
            assert!(state.begin(&request).is_some(), "a later explicit request remains available");
            assert!(state.preflight_error().is_none());
            assert!(state.request_error().is_none());
        }
    }
}
