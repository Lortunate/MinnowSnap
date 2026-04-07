mod input;
mod ocr_geometry;
mod render;

use super::{
    actions::{CloseAllPins, ClosePin, CopyPinContent, SavePinImage},
    session::{PinManager, PinSession},
};
use crate::core::capture::action::{ActionContext, ActionResult, CaptureAction};
use crate::core::i18n;
use crate::core::io::clipboard::copy_text_to_clipboard;
use crate::core::notify::NotificationType;
use crate::core::ocr_service;
use gpui::{App, Context, Entity, FocusHandle, Subscription, Window};
use std::collections::BTreeSet;

enum PointerMode {
    Idle,
    BoxSelect {
        start: gpui::Point<gpui::Pixels>,
        base_selection: BTreeSet<usize>,
    },
    TextSelect {
        block_index: usize,
    },
}

pub(super) struct PinView {
    session: Entity<PinSession>,
    manager: PinManager,
    focus_handle: FocusHandle,
    pointer_mode: PointerMode,
    _session_observer: Subscription,
}

impl PinView {
    pub(super) fn new(session: Entity<PinSession>, manager: PinManager, focus_handle: FocusHandle, cx: &mut Context<Self>) -> Self {
        let observer = cx.observe(&session, |_, _, cx| {
            cx.notify();
        });
        Self {
            session,
            manager,
            focus_handle,
            pointer_mode: PointerMode::Idle,
            _session_observer: observer,
        }
    }

    fn request_close(manager: &PinManager, window: &mut Window, cx: &mut App) {
        manager.unregister(Window::window_handle(window).window_id(), cx);
        window.defer(cx, |window, _| {
            window.remove_window();
        });
    }

    fn run_capture_action(session: &Entity<PinSession>, action: CaptureAction, cx: &mut App) {
        let image_path = session.read(cx).frame().image_path;
        let path = image_path.to_string_lossy().to_string();
        match action.execute(ActionContext::full_image(path)) {
            ActionResult::Copied => {
                crate::core::notify::show(
                    i18n::app::capture_name().as_str(),
                    i18n::notify::copied_image().as_str(),
                    NotificationType::Copy,
                );
            }
            ActionResult::Saved(path) => {
                crate::core::notify::show(
                    i18n::app::capture_name().as_str(),
                    i18n::notify::saved_image(path).as_str(),
                    NotificationType::Save,
                );
            }
            ActionResult::Error(err) => {
                tracing::error!("Pin action error: {err}");
            }
            _ => {}
        }
    }

    fn copy_selection_or_image(session: &Entity<PinSession>, cx: &mut App) {
        let text = session.read(cx).selected_or_active_text();
        if let Some(text) = text
            && copy_text_to_clipboard(text)
        {
            crate::core::notify::show(
                i18n::app::capture_name().as_str(),
                i18n::notify::copied_text().as_str(),
                NotificationType::Copy,
            );
            return;
        }
        Self::run_capture_action(session, CaptureAction::Copy, cx);
    }

    fn start_ocr(session: &Entity<PinSession>, cx: &mut App) {
        let image_path = session.update(cx, |session, _| {
            if !session.begin_ocr() {
                return None;
            }
            Some(session.frame().image_path)
        });
        let Some(image_path) = image_path else {
            return;
        };

        let weak_session = session.downgrade();
        cx.spawn(async move |cx| {
            let result = ocr_service::recognize_image_blocks(&image_path).await;
            let _ = weak_session.update(cx, |session, _| {
                session.finish_ocr(result);
            });
        })
        .detach();
    }

    fn handle_auto_ocr(&mut self, cx: &mut Context<Self>) {
        let should_run = self.session.update(cx, |session, _| session.take_auto_ocr_request());
        if should_run {
            Self::start_ocr(&self.session, cx);
        }
    }
}
