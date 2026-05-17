// Overlay session boundary: command routing, transient state, and UI-facing view models.
mod annotation;
mod command;
mod diagnostics;
mod effects;
mod picker;
mod selection;
mod session;

pub(crate) use crate::ui::features::overlay::annotation::{
    AnnotationKind, AnnotationKindTag, AnnotationLayerState, AnnotationSelectionInfo, AnnotationStyleState, AnnotationTool, MosaicMode,
};
pub(crate) use command::{AnnotationCommand, CaptureCommand, LifecycleCommand, OverlayCommand, PickerCommand};
#[cfg(feature = "overlay-diagnostics")]
pub(crate) use diagnostics::OverlayDiagnosticsSnapshot;
pub(crate) use effects::{OverlayEffect, OverlayOutcome};
pub(crate) use picker::{PickerFormat, PickerNeighborhood, PickerSample};
pub use session::OverlayHandle;
pub(crate) use session::{DragMode, OverlayFrame, OverlaySession, PickerVm, ResizeCorner};

#[cfg(test)]
mod tests {
    use super::session::OverlaySession as SessionUnderTest;
    use super::session::OverlaySurface;
    use super::*;
    use crate::ui::features::overlay::window_catalog::WindowInfo;
    use gpui::{Point, RenderImage, px};
    use std::sync::Arc;

    fn window(title: &str, app_name: &str, x: i32, y: i32, width: u32, height: u32) -> WindowInfo {
        WindowInfo {
            title: title.into(),
            app_name: app_name.into(),
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn hover_tracks_window_entry_and_exit() {
        let mut session = OverlaySession::default();
        session.set_viewport_size(400.0, 300.0);
        session.windows = vec![window("MinnowSnap", "MinnowSnap", 10, 10, 100, 100)];

        assert!(session.update_hover(Point::new(px(20.0), px(20.0))));
        assert_eq!(
            session.viewport.target,
            Some(crate::services::geometry::RectF::new(10.0, 10.0, 100.0, 100.0))
        );
        assert!(session.update_hover(Point::new(px(200.0), px(200.0))));
        assert_eq!(session.viewport.target, None);
    }

    #[test]
    fn scroll_capture_dispatches_long_capture_effect() {
        let mut session = OverlaySession::default();
        session.set_viewport_size(600.0, 400.0);
        session.viewport.selection = Some(crate::services::geometry::RectF::new(40.0, 40.0, 320.0, 220.0));

        let outcome = session.apply(OverlayCommand::Capture(CaptureCommand::Execute(
            crate::services::capture::action::CaptureAction::Scroll,
        )));
        assert_eq!(outcome.effects.len(), 1);
        assert!(matches!(outcome.effects[0], OverlayEffect::StartLongCapture { .. }));
    }

    #[test]
    fn session_prepare_surface_resets_transient_pointer_and_selection_state() {
        let mut session = SessionUnderTest::default();
        session.set_viewport_size(200.0, 120.0);
        session.apply(OverlayCommand::Lifecycle(LifecycleCommand::StartSelection(Point::new(
            px(10.0),
            px(10.0),
        ))));
        session.apply(OverlayCommand::Lifecycle(LifecycleCommand::PointerMoved(Point::new(px(40.0), px(50.0)))));

        session.prepare_surface(OverlaySurface::default());

        assert_eq!(session.mode(), DragMode::Idle);
        assert_eq!(session.selection(), None);
        assert_eq!(session.viewport.pending_pointer, None);
        assert_eq!(session.viewport.target, None);
    }

    #[test]
    fn scroll_capture_effect_ignores_overlay_background_image() {
        let mut session = OverlaySession::default();
        session.set_viewport_size(600.0, 400.0);
        session.viewport.selection = Some(crate::services::geometry::RectF::new(40.0, 40.0, 320.0, 220.0));

        let bg = Arc::new(RenderImage::new([image::Frame::new(image::RgbaImage::from_pixel(
            2,
            2,
            image::Rgba([1, 2, 3, 255]),
        ))]));
        session.background_image = Some(bg.clone());

        let outcome = session.apply(OverlayCommand::Capture(CaptureCommand::Execute(
            crate::services::capture::action::CaptureAction::Scroll,
        )));
        assert_eq!(outcome.effects.len(), 1);

        let OverlayEffect::StartLongCapture {
            selection_rect,
            viewport_rect,
            viewport_scale,
        } = &outcome.effects[0]
        else {
            panic!("expected StartLongCapture effect");
        };
        assert!(selection_rect.has_area());
        assert!(viewport_rect.width > 0.0);
        assert!(viewport_rect.height > 0.0);
        assert!(*viewport_scale >= 1.0);
        assert!(session.background_image.is_some());
    }
}
