//! The single owner of transitions between capture, long capture, preferences and pin views.
use crate::features::{long_capture, overlay, pin};
use gpui::{AnyWindowHandle, App, Global};
use minnow_application::{CaptureTarget, OperationResult, PreparedImage};

#[derive(Default)]
struct NavigationState {
    generation: u64,
    capture_pending: bool,
    overlay: Option<AnyWindowHandle>,
}
impl Global for NavigationState {}

pub(crate) fn install(cx: &mut App) {
    cx.set_global(NavigationState::default());
}

pub(crate) fn open_capture(cx: &mut App) {
    if cx.global::<NavigationState>().capture_pending {
        return;
    }
    let state = cx.global_mut::<NavigationState>();
    state.generation = state.generation.wrapping_add(1);
    let generation = state.generation;
    state.capture_pending = true;
    let previous = state.overlay.take();
    let handle = cx.global::<overlay::OverlayHandle>().clone();
    handle.end_document(cx);
    if let Some(previous) = previous {
        let _ = previous.update(cx, |_, window, _| window.remove_window());
    }
    // Let the previous native window be removed before taking the desktop snapshot.
    cx.defer(move |cx| {
        if cx.global::<NavigationState>().generation != generation {
            return;
        }
        capture_surface(generation, cx);
    });
}

fn capture_surface(generation: u64, cx: &mut App) {
    let services = crate::application(cx);
    let job = services.capture_surface(CaptureTarget::Primary);
    cx.spawn(async move |cx| {
        let result = job.await;
        cx.update(|cx| {
            if cx.global::<NavigationState>().generation != generation {
                return;
            }
            cx.global_mut::<NavigationState>().capture_pending = false;
            match result {
                Ok(surface) => {
                    for warning in &surface.warnings {
                        tracing::warn!("Capture window catalog: {warning}");
                    }
                    let handle = cx.global::<overlay::OverlayHandle>().clone();
                    let prepared = handle.prepare(
                        overlay::OverlaySurface {
                            frame: surface.frame,
                            windows: surface.windows.into_iter().map(Into::into).collect(),
                        },
                        cx,
                    );
                    if let Err(error) = prepared {
                        handle.end_document(cx);
                        crate::presentation::feedback::show_notification(
                            &crate::i18n::app::name(),
                            &error.to_string(),
                            minnow_application::ports::NotificationKind::Info,
                            cx,
                        );
                        return;
                    }
                    match overlay::open_window(cx) {
                        Ok(window) => {
                            cx.global_mut::<NavigationState>().overlay = Some(window);
                            let shutter = crate::application(cx).play_shutter();
                            cx.spawn(async move |_| {
                                if let Err(error) = shutter.await {
                                    tracing::warn!("Shutter feedback failed: {error}");
                                }
                            })
                            .detach();
                        }
                        Err(error) => {
                            handle.end_document(cx);
                            tracing::error!("Failed to open capture window: {error}");
                        }
                    }
                }
                Err(error) => crate::presentation::feedback::show_notification(
                    &crate::i18n::app::name(),
                    &error.to_string(),
                    minnow_application::ports::NotificationKind::Info,
                    cx,
                ),
            }
        });
    })
    .detach();
}

pub(crate) fn open_pin(cx: &mut App, prepared: PreparedImage, auto_ocr: bool) -> OperationResult<()> {
    pin::open_window(cx, pin::PinRequest::from_application(prepared, auto_ocr))
}

pub(crate) fn prepare_long_capture(
    cx: &mut App,
    request: long_capture::LongCaptureRequest,
) -> OperationResult<minnow_application::LongCaptureHandle> {
    let target = CaptureTarget::Display(request.geometry.display_id.clone());
    let capture = crate::application(cx).prepare_long_capture(target, minnow_imaging::ViewRect(request.selection_rectf()))?;
    if let Err(error) = long_capture::open_window(cx, request, capture.clone()) {
        capture.cancel();
        return Err(error);
    }
    Ok(capture)
}

pub(crate) fn close_all(cx: &mut App) {
    let state = cx.global_mut::<NavigationState>();
    state.generation = state.generation.wrapping_add(1);
    state.capture_pending = false;
    state.overlay = None;
    let overlay = cx.global::<overlay::OverlayHandle>().clone();
    overlay.end_document(cx);
    for window in cx.windows() {
        let _ = window.update(cx, |_, window, _| window.remove_window());
    }
}
