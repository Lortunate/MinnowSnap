// Overlay session boundary: command routing, transient state, and UI-facing view models.
mod annotation;
mod command;
mod diagnostics;
mod effects;
mod picker;
mod queries;
mod selection;
mod session;

pub(crate) use crate::features::overlay::annotation::{
    AnnotationKind, AnnotationKindTag, AnnotationLayerState, AnnotationSelectionInfo, AnnotationStyleState, AnnotationTool, MosaicMode,
};
pub(crate) use command::{AnnotationCommand, CaptureCommand, LifecycleCommand, OverlayCommand, PickerCommand};
#[cfg(feature = "overlay-diagnostics")]
pub(crate) use diagnostics::OverlayDiagnosticsSnapshot;
pub(crate) use effects::{OverlayEffect, OverlayOutcome};
pub(crate) use picker::{PickerFormat, PickerNeighborhood, PickerSample};
pub use session::OverlayHandle;
use session::OverlaySession;
pub(crate) use session::{DragMode, OverlayFrame, OverlaySurface, PickerVm, ResizeCorner};

#[cfg(test)]
mod tests {
    use super::session::OverlaySession as SessionUnderTest;
    use super::session::OverlaySurface;
    use super::*;
    use crate::features::overlay::window_catalog::WindowInfo;
    use gpui::{Point, RenderImage, px};
    use std::sync::Arc;

    fn test_surface() -> OverlaySurface {
        OverlaySurface {
            frame: minnow_imaging::CapturedFrame::new(
                minnow_imaging::ImageAsset::new(image::RgbaImage::new(600, 400)),
                minnow_imaging::CaptureGeometry {
                    display_id: minnow_imaging::DisplayId("test".into()),
                    screen_origin: minnow_imaging::ScreenPoint { x: 0.0, y: 0.0 },
                    scale_factor: 1.0,
                    pixel_size: (600, 400),
                },
            )
            .expect("test frame geometry"),
            windows: vec![],
        }
    }

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
            Some(minnow_imaging::geometry::RectF::new(10.0, 10.0, 100.0, 100.0))
        );
        assert!(session.update_hover(Point::new(px(200.0), px(200.0))));
        assert_eq!(session.viewport.target, None);
    }

    #[test]
    fn scroll_capture_dispatches_long_capture_effect() {
        let mut session = OverlaySession::default();
        session.prepare_surface(test_surface());
        session.set_viewport_size(600.0, 400.0);
        session.viewport.selection = Some(minnow_imaging::geometry::RectF::new(40.0, 40.0, 320.0, 220.0));

        let outcome = session.apply(OverlayCommand::Capture(CaptureCommand::Execute(
            crate::flows::output::CaptureIntent::Scroll,
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

        session.prepare_surface(test_surface());

        assert_eq!(session.mode(), DragMode::Idle);
        assert_eq!(session.selection(), None);
        assert_eq!(session.viewport.pending_pointer, None);
        assert_eq!(session.viewport.target, None);
    }

    #[test]
    fn scroll_capture_effect_ignores_overlay_background_image() {
        let mut session = OverlaySession::default();
        session.prepare_surface(test_surface());
        session.set_viewport_size(600.0, 400.0);
        session.viewport.selection = Some(minnow_imaging::geometry::RectF::new(40.0, 40.0, 320.0, 220.0));

        let bg = Arc::new(RenderImage::new([image::Frame::new(image::RgbaImage::from_pixel(
            2,
            2,
            image::Rgba([1, 2, 3, 255]),
        ))]));
        session.background_image = Some(bg.clone());

        let outcome = session.apply(OverlayCommand::Capture(CaptureCommand::Execute(
            crate::flows::output::CaptureIntent::Scroll,
        )));
        assert_eq!(outcome.effects.len(), 1);

        let OverlayEffect::StartLongCapture {
            selection_rect,
            viewport_rect,
            geometry,
        } = &outcome.effects[0]
        else {
            panic!("expected StartLongCapture effect");
        };
        assert!(selection_rect.has_area());
        assert!(viewport_rect.width > 0.0);
        assert!(viewport_rect.height > 0.0);
        assert!(geometry.scale_factor > 0.0);
        assert!(session.background_image.is_some());
    }
    #[test]
    fn derived_jobs_observe_selection_changes_without_annotation_edits() {
        let mut session = OverlaySession::default();
        session.prepare_surface(test_surface());
        session.viewport.selection = Some(minnow_imaging::RectF::new(10.0, 10.0, 100.0, 80.0));
        let first = session.sync_operation_revision();
        assert_eq!(session.sync_operation_revision(), first);
        session.viewport.selection = Some(minnow_imaging::RectF::new(20.0, 10.0, 100.0, 80.0));
        assert!(session.sync_operation_revision() > first);
    }

    #[test]
    fn a_font_failure_rejects_text_commands_without_disabling_shape_capture() {
        let mut session = OverlaySession::default();
        session.prepare_surface(test_surface());
        session.set_viewport_size(600.0, 400.0);
        session.viewport.selection = Some(minnow_imaging::RectF::new(10.0, 10.0, 300.0, 200.0));
        let font_error = minnow_application::OperationError::new(minnow_application::ErrorKind::Unavailable, "font bytes could not be read");
        session.set_annotation_font(Err(font_error.clone()));
        assert_eq!(session.frame().font_error, Some(font_error));
        for command in [
            AnnotationCommand::SetTool(AnnotationTool::Text),
            AnnotationCommand::SetTool(AnnotationTool::Counter),
            AnnotationCommand::StartTextEdit,
            AnnotationCommand::AppendText { text: "text".into() },
        ] {
            let outcome = session.apply(OverlayCommand::Annotation(command));
            assert!(matches!(&outcome.effects[..], [OverlayEffect::Warning(error)] if error.message.contains("font bytes could not be read")));
        }
        assert_eq!(session.annotation.active_tool(), None);
        // Even a preexisting text-tool state cannot bypass the command boundary.
        session.annotation.set_tool(AnnotationTool::Counter);
        let outcome = session.apply(OverlayCommand::Annotation(AnnotationCommand::StartDraw(Point::new(px(40.0), px(40.0)))));
        assert!(matches!(&outcome.effects[..], [OverlayEffect::Warning(_)]));
        session.apply(OverlayCommand::Annotation(AnnotationCommand::SetTool(AnnotationTool::Rectangle)));
        assert!(session.start_annotation_draw(Point::new(px(40.0), px(40.0))));
        assert!(session.update_annotation_interaction(Point::new(px(100.0), px(100.0))));
        assert!(session.finish_annotation_interaction());
        let outcome = session.apply(OverlayCommand::Capture(CaptureCommand::Execute(
            crate::flows::output::CaptureIntent::Copy,
        )));
        assert!(matches!(&outcome.effects[..], [OverlayEffect::Capture { .. }]));
    }

    #[test]
    fn a_later_font_warning_remains_visible_and_revokes_the_pending_copy_close_right() {
        let mut session = OverlaySession::default();
        session.prepare_surface(test_surface());
        session.set_viewport_size(600.0, 400.0);
        session.set_annotation_font(Err(minnow_application::OperationError::new(
            minnow_application::ErrorKind::Unavailable,
            "font bytes could not be read",
        )));
        let image_revision = session.sync_operation_revision();
        let granted_interaction = session.interaction_revision;
        let request = crate::flows::output::OutputRequest::Text {
            text: "copied value".into(),
            message: "copied".into(),
        };
        let attempt = session.output.begin(&request).unwrap();
        let outcome = session.apply(OverlayCommand::Annotation(AnnotationCommand::SetTool(AnnotationTool::Text)));
        let [OverlayEffect::Warning(error)] = &outcome.effects[..] else {
            panic!("font command must report its failure")
        };
        session.show_preflight_error(error.clone());
        let frame = session.frame();
        assert!(frame.output_pending);
        assert!(frame.preflight_error.is_some(), "the tool error is visible while Copy is pending");
        assert!(frame.output_error.is_none(), "Copy has not failed");
        assert!(!frame.output_can_retry);
        assert_eq!(
            session.sync_operation_revision(),
            image_revision,
            "feedback does not change image content"
        );
        assert!(!session.output_may_close(granted_interaction));

        session.output.finish(
            attempt,
            &crate::flows::output::interpret(Ok(minnow_application::ImageCommandResult::Copied)),
        );
        let frame = session.frame();
        assert!(!frame.output_pending);
        assert!(frame.preflight_error.is_some(), "successful Copy does not dismiss the later font warning");
        assert!(frame.output_error.is_none());
        assert!(
            !session.output_may_close(granted_interaction),
            "old Copy cannot close the later interaction"
        );
    }

    #[test]
    fn a_color_copy_keeps_its_output_but_cannot_close_a_later_picker_interaction() {
        let mut session = OverlaySession::default();
        session.prepare_surface(test_surface());
        session.set_viewport_size(600.0, 400.0);
        session.update_pointer(Point::new(px(40.0), px(40.0)));
        let image_revision = session.sync_operation_revision();
        let granted_interaction = session.interaction_revision;
        let requested_text = session.picker_text().unwrap();
        let request = crate::flows::output::OutputRequest::Text {
            text: requested_text.clone(),
            message: "copied".into(),
        };
        let attempt = session.output.begin(&request).unwrap();
        assert!(session.output_may_close(granted_interaction));
        session.apply(OverlayCommand::Picker(PickerCommand::CycleFormat));
        session.apply(OverlayCommand::Picker(PickerCommand::MoveByPixel { delta_x: 1, delta_y: 0 }));
        assert_eq!(session.sync_operation_revision(), image_revision);
        assert_ne!(session.picker_text().unwrap(), requested_text);
        assert!(
            session.output.finish(
                attempt,
                &crate::flows::output::interpret(Ok(minnow_application::ImageCommandResult::Copied)),
            ),
            "the requested external copy still completes"
        );
        assert!(!session.output_may_close(granted_interaction));
    }
    #[test]
    fn ending_a_document_releases_projection_pixels_but_keeps_a_submitted_snapshot_alive() {
        let mut session = OverlaySession::default();
        let surface = test_surface();
        let source = Arc::downgrade(surface.frame.image());
        let submitted = minnow_application::ImageInput::selection(
            surface.frame.clone(),
            minnow_imaging::ViewRect(minnow_imaging::RectF::new(0.0, 0.0, 10.0, 10.0)),
            None,
        )
        .unwrap();
        session.prepare_surface(surface);
        let texture = Arc::downgrade(session.background_image.as_ref().unwrap());
        session.clear();
        assert!(session.captured_frame.is_some(), "clearing a selection keeps its document");
        session.end_document();
        assert!(session.captured_frame.is_none());
        assert!(session.background_pixels.is_none());
        assert!(session.background_image.is_none());
        assert!(texture.upgrade().is_none(), "display textures end with their document");
        assert!(source.upgrade().is_some(), "submitted output owns its immutable input");
        drop(submitted);
        assert!(source.upgrade().is_none());
    }

    #[test]
    fn derived_jobs_keep_their_revision_when_only_editor_controls_change() {
        let mut session = OverlaySession::default();
        session.prepare_surface(test_surface());
        session.set_viewport_size(600.0, 400.0);
        session.viewport.selection = Some(minnow_imaging::RectF::new(10.0, 10.0, 300.0, 200.0));
        let initial = session.sync_operation_revision();
        session.set_annotation_tool(AnnotationTool::Rectangle);
        session.set_annotation_color(0x22aaffff);
        assert_eq!(
            session.sync_operation_revision(),
            initial,
            "tool and default color do not change the image"
        );

        assert!(session.start_annotation_draw(Point::new(px(40.0), px(40.0))));
        assert!(session.update_annotation_interaction(Point::new(px(100.0), px(100.0))));
        assert!(session.finish_annotation_interaction());
        let changed = session.sync_operation_revision();
        assert!(changed > initial);
        let selected = session.selected_annotation_item().unwrap().id;
        session.select_annotation(None);
        assert_eq!(session.sync_operation_revision(), changed);
        session.select_annotation(Some(selected));
        assert_eq!(
            session.sync_operation_revision(),
            changed,
            "object selection keeps derived results current"
        );
    }

    #[test]
    fn derived_jobs_observe_uncommitted_text_changes() {
        let mut session = OverlaySession::default();
        session.prepare_surface(test_surface());
        session.set_viewport_size(600.0, 400.0);
        session.viewport.selection = Some(minnow_imaging::RectF::new(10.0, 10.0, 300.0, 200.0));
        session.set_annotation_tool(AnnotationTool::Text);
        assert!(session.start_annotation_draw(Point::new(px(40.0), px(40.0))));
        let before = session.sync_operation_revision();
        assert!(session.append_text_edit("new text"));
        assert!(session.sync_operation_revision() > before, "text drafts change the output before commit");
    }
    #[test]
    fn image_outputs_commit_the_latest_text_draft_before_freezing() {
        for command in [
            CaptureCommand::Execute(crate::flows::output::CaptureIntent::Save(
                minnow_application::SaveDestination::Preferences,
            )),
            CaptureCommand::Execute(crate::flows::output::CaptureIntent::OpenPin),
            CaptureCommand::Execute(crate::flows::output::CaptureIntent::Ocr),
            CaptureCommand::SaveWithPath("/captures".into()),
        ] {
            let mut session = OverlaySession::default();
            session.prepare_surface(test_surface());
            session.set_viewport_size(600.0, 400.0);
            session.viewport.selection = Some(minnow_imaging::RectF::new(10.0, 10.0, 300.0, 200.0));
            session.set_annotation_tool(AnnotationTool::Text);
            assert!(session.start_annotation_draw(Point::new(px(40.0), px(40.0))));
            assert!(session.append_text_edit("-latest draft"));
            let outcome = session.apply(OverlayCommand::Capture(command));
            let OverlayEffect::Capture { annotations, .. } = &outcome.effects[0] else {
                panic!("expected frozen image output");
            };
            assert!(
                annotations
                    .items()
                    .iter()
                    .any(|item| matches!(&item.kind, AnnotationKind::Text { text, .. } if text.ends_with("-latest draft")))
            );
            assert!(session.text_editing_id().is_none());
        }
    }

    #[test]
    fn queued_edits_invalidate_old_output_before_paint_and_output_flushes_them() {
        let mut session = OverlaySession::default();
        session.prepare_surface(test_surface());
        session.set_viewport_size(600.0, 400.0);
        session.apply(OverlayCommand::Lifecycle(LifecycleCommand::StartSelection(Point::new(
            px(10.0),
            px(10.0),
        ))));
        let previous_revision = session.sync_operation_revision();
        session.apply(OverlayCommand::Lifecycle(LifecycleCommand::PointerMoved(Point::new(
            px(130.0),
            px(110.0),
        ))));
        assert!(session.sync_operation_revision() > previous_revision);
        assert!(session.viewport.pending_pointer.is_some());
        let outcome = session.apply(OverlayCommand::Capture(CaptureCommand::Execute(
            crate::flows::output::CaptureIntent::Save(minnow_application::SaveDestination::Preferences),
        )));
        let OverlayEffect::Capture { selection, .. } = &outcome.effects[0] else {
            panic!("expected image output");
        };
        assert_eq!(selection.0, minnow_imaging::RectF::new(10.0, 10.0, 120.0, 100.0));
        assert!(session.viewport.pending_pointer.is_none());
        assert_eq!(session.mode(), DragMode::Idle);
    }
}
