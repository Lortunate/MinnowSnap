use super::OverlayHandle;
use crate::features::long_capture::LongCaptureRequest;
use minnow_application::{ImageProjection, OperationError, OperationResult};
use minnow_imaging::geometry::{Rect, RectF};

use crate::flows::output::{self, CaptureIntent, OutputRequest};
use gpui::{App, Window};

pub(crate) enum OverlayEffect {
    Refresh,
    Warning(OperationError),
    FlushPointer,
    Close,
    StartLongCapture {
        selection_rect: Rect,
        viewport_rect: RectF,
        geometry: minnow_imaging::CaptureGeometry,
    },
    Capture {
        intent: CaptureIntent,
        selection: minnow_imaging::ViewRect,
        annotations: minnow_editor::AnnotationSnapshot,
    },
    CopyText {
        text: String,
        message: String,
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

impl OverlayHandle {
    pub(crate) fn dispatch(&self, command: crate::features::overlay::state::OverlayCommand, window: &mut Window, cx: &mut App) {
        let viewport = window.viewport_size();
        let outcome = self.mutate_session(cx, |session| {
            session.set_viewport_size(viewport.width.to_f64(), viewport.height.to_f64());
            session.apply(command)
        });
        match outcome {
            Ok(outcome) => self.run_outcome(outcome, window, cx),
            Err(_) => window.refresh(),
        }
    }

    /// Every document mutation publishes its application revision before effects can observe it.
    pub(super) fn mutate_session<R>(&self, cx: &mut App, change: impl FnOnce(&mut super::OverlaySession) -> R) -> OperationResult<R> {
        let (result, publication) = self.0.update(cx, |state, cx| {
            let result = change(state);
            let revision = state.sync_operation_revision();
            let publication = state.application_session.map(|id| (id, revision));
            cx.notify();
            (result, publication)
        });
        let published = publication.map_or(Ok(()), |(id, revision)| self.1.publish_session_revision(id, revision));
        self.0.update(cx, |state, cx| {
            state.discard_stale_derived_retry(&self.1);
            if let Err(error) = &published {
                state.show_preflight_error(error.clone());
                cx.notify();
            }
        });
        published.map(|()| result)
    }

    fn flush_pending_pointer(&self, window: &mut Window, cx: &mut App) {
        let changed = self.mutate_session(cx, |session| session.apply_pending_pointer());
        if !matches!(changed, Ok(false)) {
            self.refresh(window, cx);
        }
    }

    fn run_outcome(&self, outcome: OverlayOutcome, window: &mut Window, cx: &mut App) {
        for effect in outcome.effects {
            self.run_effect(effect, window, cx);
        }
    }

    fn run_effect(&self, effect: OverlayEffect, window: &mut Window, cx: &mut App) {
        match effect {
            OverlayEffect::Refresh => self.refresh(window, cx),
            OverlayEffect::Warning(error) => self.show_warning(error, window, cx),
            OverlayEffect::FlushPointer => {
                let handle = self.clone();
                window.on_next_frame(move |window, cx| handle.flush_pending_pointer(window, cx));
                window.refresh();
            }
            OverlayEffect::Close => self.close(window, cx),
            OverlayEffect::StartLongCapture {
                selection_rect,
                viewport_rect,
                geometry,
            } => self.start_long_capture(selection_rect, viewport_rect, geometry, window, cx),
            OverlayEffect::CopyText { text, message } => {
                self.run_output(OutputRequest::Text { text, message }, None, window, cx);
            }
            OverlayEffect::Capture {
                intent,
                selection,
                annotations,
            } => self.capture(intent, selection, annotations, window, cx),
        }
    }

    fn refresh(&self, window: &mut Window, cx: &mut App) {
        self.0.update(cx, |session, _| session.diag_on_refresh());
        window.refresh();
    }

    fn close(&self, window: &mut Window, cx: &mut App) {
        self.end_document(cx);
        window.defer(cx, |window, _| {
            window.remove_window();
        });
    }

    fn start_long_capture(
        &self,
        selection_rect: minnow_imaging::geometry::Rect,
        viewport_rect: minnow_imaging::geometry::RectF,
        geometry: minnow_imaging::CaptureGeometry,
        window: &mut Window,
        cx: &mut App,
    ) {
        let bounds = window.window_bounds().get_bounds();
        let request = LongCaptureRequest {
            geometry,
            selection_rect,
            viewport_rect,
            viewport_origin_screen: (bounds.origin.x.to_f64(), bounds.origin.y.to_f64()),
        };
        match crate::navigation::prepare_long_capture(cx, request) {
            Ok(capture) => {
                // Destination windows exist. Commit the source only after preparation succeeds.
                self.close(window, cx);
                // Remove the native overlay before the capture worker crosses its start gate.
                cx.defer(move |_| {
                    capture.start();
                });
            }
            Err(error) => self.show_warning(error, window, cx),
        }
    }

    fn capture(
        &self,
        intent: CaptureIntent,
        selection: minnow_imaging::ViewRect,
        annotations: minnow_editor::AnnotationSnapshot,
        window: &mut Window,
        cx: &mut App,
    ) {
        let state = self.0.read(cx);
        let input = state
            .application_session
            .ok_or_else(|| {
                OperationError::new(
                    minnow_application::ErrorKind::Conflict,
                    crate::presentation::i18n::overlay::image_changed(),
                )
            })
            .and_then(|id| {
                self.1
                    .publish_session_output(id, state.operation_revision, ImageProjection::Selection(selection), Some(annotations))
            });
        let auto_ocr = self.1.settings_snapshot().ocr.enabled;
        match input.and_then(|input| intent.output(input, auto_ocr)) {
            Ok(request) => self.run_output(request, None, window, cx),
            Err(error) => self.show_warning(error, window, cx),
        };
    }

    pub(crate) fn retry_output(&self, window: &mut Window, cx: &mut App) {
        let retry = self.0.update(cx, |state, cx| {
            let retry = state.retry_output(&self.1);
            cx.notify();
            retry
        });
        if let Some((request, close_revision)) = retry {
            self.run_output(request, Some(close_revision), window, cx);
        }
    }

    fn run_output(&self, request: OutputRequest, close_revision: Option<u64>, window: &mut Window, cx: &mut App) {
        let identity = self.0.update(cx, |state, cx| {
            let identity = state.begin_output(&request, close_revision);
            if identity.is_some() {
                cx.notify();
            }
            identity
        });
        let Some(identity) = identity else { return };
        let services = self.1.clone();
        let handle = self.clone();
        let window_handle = window.window_handle();
        cx.spawn(async move |cx| {
            let completion = output::execute(services, request).await;
            cx.update(|cx| {
                completion.deliver(cx, |mut delivery, cx| {
                    window_handle
                        .update(cx, |_, window, cx| {
                            let state = handle.0.read(cx);
                            if !state.accepts_output(identity) {
                                return false;
                            }
                            let may_close = state.output_delivery_may_close(identity) && delivery.is_current();
                            delivery.resolve_handoff(cx);
                            let delivered = handle.0.update(cx, |state, cx| {
                                let delivered = state.finish_output(identity, &delivery);
                                if delivered {
                                    cx.notify();
                                }
                                delivered
                            });
                            if !delivered {
                                return false;
                            }
                            if delivery.succeeded() {
                                if may_close {
                                    handle.close(window, cx);
                                }
                            } else {
                                handle.refresh(window, cx);
                            }
                            true
                        })
                        .unwrap_or(false)
                })
            });
        })
        .detach();
    }

    fn show_warning(&self, error: OperationError, window: &mut Window, cx: &mut App) {
        self.0.update(cx, |state, cx| {
            state.show_preflight_error(error);
            cx.notify();
        });
        window.refresh();
    }
}
