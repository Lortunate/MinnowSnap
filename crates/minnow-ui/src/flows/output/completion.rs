use super::{CaptureActionEffect, OutputState, notify_committed, notify_preserved_failure};
use gpui::App;
use minnow_application::{ApplicationServices, ErrorKind, JobScope, OperationError};
use std::sync::Arc;

/// A completed attempt still owns feedback when its requesting view has gone away.
pub(crate) struct OutputCompletion {
    delivery: OutputDelivery,
    preserve: bool,
}

pub(crate) struct OutputDelivery {
    services: Arc<ApplicationServices>,
    scope: JobScope,
    effect: CaptureActionEffect,
}

impl OutputCompletion {
    pub(super) fn new(services: Arc<ApplicationServices>, scope: JobScope, preserve: bool, effect: CaptureActionEffect) -> Self {
        Self {
            delivery: OutputDelivery { services, scope, effect },
            preserve,
        }
    }

    /// The feature accepts only its own live attempt and retains its window policy.
    pub(crate) fn deliver(self, cx: &mut App, accept: impl FnOnce(OutputDelivery, &mut App) -> bool) {
        notify_committed(&self.delivery.effect, cx);
        let detached_failure = self.delivery.effect.preserved_failure(self.preserve).cloned();
        if !accept(self.delivery, cx)
            && let Some(error) = detached_failure
        {
            notify_preserved_failure(&error, cx);
        }
    }
}

impl OutputDelivery {
    pub(crate) fn is_current(&self) -> bool {
        self.services.is_current(self.scope)
    }

    /// Call only after the feature accepts the source identity and pending attempt.
    /// A failed window handoff is this request's failure and keeps its frozen retry.
    pub(crate) fn resolve_handoff(&mut self, cx: &mut App) {
        let CaptureActionEffect::OpenPin { prepared, auto_ocr } = &self.effect else {
            return;
        };
        let result = if self.is_current() {
            crate::navigation::open_pin(cx, prepared.clone(), *auto_ocr)
        } else {
            Err(OperationError::new(
                ErrorKind::Conflict,
                crate::presentation::i18n::overlay::image_changed(),
            ))
        };
        if let Err(error) = result {
            self.effect = CaptureActionEffect::Warning(error);
        }
    }

    pub(crate) fn finish(&self, state: &mut OutputState, attempt: u64) -> bool {
        if !state.is_pending_attempt(attempt) {
            return false;
        }
        state.discard_stale_derived_retry(|input| self.services.is_current(input.scope()));
        state.finish(attempt, &self.effect)
    }

    pub(crate) fn succeeded(&self) -> bool {
        !matches!(self.effect, CaptureActionEffect::Warning(_))
    }
}
