mod input;
mod ocr_geometry;
mod render;

use super::{
    CloseAllPins, ClosePin, CopyPinContent, SavePinImage,
    state::{PinManager, PinSession},
};

use crate::flows::output;
use crate::presentation::i18n;
use gpui::{App, Context, Entity, FocusHandle, Subscription, Window};
use minnow_application::{ImageCommand, ImageInput, ImageProjection, OperationResult};
use std::collections::BTreeSet;

enum PointerMode {
    Idle,
    BoxSelect {
        result_revision: u64,
        start: gpui::Point<gpui::Pixels>,
        base_selection: BTreeSet<usize>,
    },
    TextSelect {
        result_revision: u64,
        block_index: usize,
    },
}

impl PointerMode {
    fn for_result(&mut self, current_revision: u64) -> &Self {
        let source_revision = match self {
            Self::Idle => None,
            Self::BoxSelect { result_revision, .. } | Self::TextSelect { result_revision, .. } => Some(*result_revision),
        };
        if source_revision.is_some_and(|revision| revision != current_revision) {
            *self = Self::Idle;
        }
        self
    }
}

pub(super) struct PinView {
    session: Entity<PinSession>,
    manager: PinManager,
    focus_handle: FocusHandle,
    pointer_mode: PointerMode,
    _session_observer: Subscription,
    services: std::sync::Arc<minnow_application::ApplicationServices>,
    application_session: minnow_application::SessionId,
}

impl PinView {
    pub(super) fn new(
        session: Entity<PinSession>,
        manager: PinManager,
        focus_handle: FocusHandle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.defer_in(window, |view, _, cx| view.handle_auto_ocr(cx));
        let observer = cx.observe(&session, |_, _, cx| {
            cx.notify();
        });
        let services = crate::application(cx);
        let application_session = services.create_session(session.read(cx).image(), 0).id();
        Self {
            session,
            manager,
            focus_handle,
            pointer_mode: PointerMode::Idle,
            _session_observer: observer,
            services,
            application_session,
        }
    }

    fn request_close(manager: &PinManager, window: &mut Window, cx: &mut App) {
        manager.unregister(Window::window_handle(window).window_id(), cx);
        window.defer(cx, |window, _| {
            window.remove_window();
        });
    }

    fn image_input(&self) -> OperationResult<ImageInput> {
        self.services
            .publish_session_output(self.application_session, 0, ImageProjection::Full, None)
    }

    fn run_image_command(&self, command: ImageCommand, cx: &mut App) {
        match self.image_input() {
            Ok(input) => self.run_output(output::OutputRequest::Image { command, input }, cx),
            Err(error) => self.session.update(cx, |session, cx| {
                session.show_preflight_error(error);
                cx.notify();
            }),
        }
    }

    fn run_output(&self, request: output::OutputRequest, cx: &mut App) {
        let attempt = self.session.update(cx, |session, cx| {
            let attempt = session.begin_output(&request);
            if attempt.is_some() {
                cx.notify();
            }
            attempt
        });
        let Some(attempt) = attempt else { return };
        let weak_session = self.session.downgrade();
        let services = crate::application(cx);
        cx.spawn(async move |cx| {
            let completion = output::execute(services, request).await;
            cx.update(|cx| {
                completion.deliver(cx, |delivery, cx| {
                    delivery.is_current()
                        && weak_session
                            .update(cx, |session, cx| {
                                let delivered = session.finish_output(attempt, &delivery);
                                if delivered {
                                    cx.notify();
                                }
                                delivered
                            })
                            .unwrap_or(false)
                })
            });
        })
        .detach();
    }

    fn retry_output(&self, cx: &mut App) {
        if let Some(request) = self.session.read(cx).retry_output() {
            self.run_output(request, cx);
        }
    }

    fn copy_selection_or_image(&self, cx: &mut App) {
        if let Some(text) = self.session.read(cx).selected_or_active_text() {
            self.run_output(
                output::OutputRequest::Text {
                    text,
                    message: i18n::notify::copied_text(),
                },
                cx,
            );
            return;
        }
        self.run_image_command(ImageCommand::Copy, cx);
    }

    fn start_ocr(&self, cx: &mut App) {
        if let Err(error) = self.services.recognition_availability().require_available() {
            self.session.update(cx, |session, cx| {
                session.record_ocr_unavailable(error.to_string());
                cx.notify();
            });
            return;
        }
        let input = match self.image_input() {
            Ok(input) => input,
            Err(error) => {
                self.session.update(cx, |session, cx| {
                    session.record_ocr_unavailable(error.to_string());
                    cx.notify();
                });
                return;
            }
        };
        let scope = input.scope();
        let source = self.session.update(cx, |session, cx| {
            let source = session.begin_ocr_source();
            if source.is_some() {
                cx.notify();
            }
            source
        });
        let Some((generation, _)) = source else {
            return;
        };
        let weak_session = self.session.downgrade();
        let job = self.services.recognize(input);
        let services = self.services.clone();
        cx.spawn(async move |cx| {
            let result = job.await;
            let _ = weak_session.update(cx, |session, cx| {
                if !services.is_current(scope) {
                    return;
                }
                session.finish_ocr(generation, result.map_err(|error| error.to_string()));
                cx.notify();
            });
        })
        .detach();
    }

    fn handle_auto_ocr(&mut self, cx: &mut Context<Self>) {
        let should_run = self.session.update(cx, |session, _| session.take_auto_ocr_request());
        if should_run {
            self.start_ocr(cx);
        }
    }
}

impl Drop for PinView {
    fn drop(&mut self) {
        self.services.close_session(self.application_session);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::pin::PinRequest;
    use gpui::{point, px};
    use minnow_application::ports::{NormalizedOcrGeometry, OcrBlock};
    use minnow_imaging::ImageAsset;

    #[test]
    fn an_old_drag_cannot_reintroduce_indices_after_a_successful_ocr_replacement() {
        let image = ImageAsset::new(image::RgbaImage::new(80, 40));
        let lease = crate::features::pin::state::PinBudget::default().reserve(&image).unwrap();
        let mut session = PinSession::from_request(PinRequest::new(image, None, false), lease);
        let block = || OcrBlock::new("text", NormalizedOcrGeometry::new(0.5, 0.5, 0.4, 0.1, 0.0).unwrap());
        let (first, _) = session.begin_ocr_source().unwrap();
        session.finish_ocr(first, Ok(vec![block(), block(), block()]));
        session.set_single_selected_index(2);
        let first_result = session.frame().ocr;
        let mut pointer = PointerMode::BoxSelect {
            result_revision: first_result.result_revision,
            start: point(px(0.0), px(0.0)),
            base_selection: first_result.selected_indices,
        };
        session.start_selection_rect(point(px(0.0), px(0.0)));

        let (retry, _) = session.begin_ocr_source().unwrap();
        assert!(
            matches!(pointer.for_result(session.ocr_result_revision()), PointerMode::BoxSelect { .. }),
            "retrying must keep the previous result usable"
        );
        session.finish_ocr(retry, Ok(vec![block()]));

        // The next pointer move uses the same guarded dispatch as the view.
        if let PointerMode::BoxSelect { base_selection, .. } = pointer.for_result(session.ocr_result_revision()) {
            session.set_selected_indices(base_selection.clone());
        }
        assert!(matches!(pointer, PointerMode::Idle));
        assert!(session.frame().ocr.selected_indices.is_empty());
    }
}
