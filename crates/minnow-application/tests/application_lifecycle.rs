use image::{Rgba, RgbaImage};
use minnow_application::ports::{
    ExportOptions, ExportReceipt, ImageOutput, ModelStatus, NormalizedOcrGeometry, NotificationRequest, Notifications, OcrBlock, OperationFuture,
    QrDecoder, RecognitionAvailability, RecognitionModels, ShutterSound, TextRecognition,
};
use minnow_application::*;
use minnow_settings::{SaveDirectory, SettingsHandle, SettingsStore};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use tokio::sync::{Notify, watch};
use tokio_util::sync::CancellationToken;

#[derive(Default)]
struct Output {
    fail_save: AtomicBool,
    fail_copy: AtomicBool,
    paused_saves: Option<Arc<PausedSaves>>,
}

#[derive(Default)]
struct PausedSaves {
    entered: Notify,
    released: Mutex<bool>,
    condition: Condvar,
}

impl PausedSaves {
    fn wait(&self) {
        self.entered.notify_one();
        let mut released = self.released.lock().unwrap();
        while !*released {
            released = self.condition.wait(released).unwrap();
        }
    }

    fn resume(&self) {
        *self.released.lock().unwrap() = true;
        self.condition.notify_all();
    }
}

struct ResumeSavesOnDrop(Arc<PausedSaves>);

impl Drop for ResumeSavesOnDrop {
    fn drop(&mut self) {
        self.0.resume();
    }
}

impl ImageOutput for Output {
    fn copy_image(&self, _: &RgbaImage) -> OperationResult<()> {
        if self.fail_copy.swap(false, Ordering::SeqCst) {
            return Err(OperationError::new(ErrorKind::Native, "clipboard unavailable"));
        }
        Ok(())
    }
    fn copy_text(&self, _: &str) -> OperationResult<()> {
        Ok(())
    }
    fn default_directory(&self) -> OperationResult<std::path::PathBuf> {
        Ok("exports".into())
    }
    fn save_image(&self, _: &RgbaImage, options: &ExportOptions) -> OperationResult<ExportReceipt> {
        if self.fail_save.swap(false, Ordering::SeqCst) {
            return Err(OperationError::new(ErrorKind::Io, "disk full"));
        }
        if let Some(paused) = &self.paused_saves {
            paused.wait();
        }
        Ok(ExportReceipt {
            path: options.directory.join("snapshot.png"),
            bytes_written: 16,
        })
    }
}

struct Recognition {
    started: Notify,
    release: Notify,
}

impl TextRecognition for Recognition {
    fn availability(&self) -> RecognitionAvailability {
        RecognitionAvailability::Available
    }

    fn recognize(&self, _: ImageRef, _: CancellationToken) -> OperationFuture<'_, Vec<OcrBlock>> {
        Box::pin(async {
            self.started.notify_one();
            self.release.notified().await;
            Ok(vec![])
        })
    }
}

impl QrDecoder for Recognition {
    fn decode_qr(&self, _: &RgbaImage) -> OperationResult<Option<String>> {
        Ok(Some("decoded".into()))
    }
}

impl RecognitionModels for Recognition {
    fn model_status(&self) -> ModelStatus {
        ModelStatus::default()
    }
    fn subscribe_model_status(&self) -> watch::Receiver<ModelStatus> {
        watch::channel(ModelStatus::default()).1
    }
    fn ensure_models(&self, _: bool, _: CancellationToken) -> OperationFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }
}

struct Notices;
impl Notifications for Notices {
    fn show(&self, _: &NotificationRequest) -> OperationResult<()> {
        Ok(())
    }
}
impl ShutterSound for Notices {
    fn play(&self) -> OperationResult<()> {
        Ok(())
    }
}

struct Capture {
    frame: CapturedFrame,
}
impl CaptureBackend for Capture {
    fn capture(&self, _: &CaptureTarget) -> Result<CapturedFrame, CaptureError> {
        Ok(self.frame.clone())
    }
    fn displays(&self) -> Result<Vec<DisplayInfo>, CaptureError> {
        Ok(vec![])
    }
    fn windows(&self, _: &DisplayId) -> Result<Vec<WindowInfo>, CaptureError> {
        Ok(vec![])
    }
}

fn frame() -> CapturedFrame {
    let image = ImageAsset::new(RgbaImage::from_fn(64, 64, |x, y| Rgba([x as u8, y as u8, (x + y) as u8, 255])));
    CapturedFrame::new(
        image,
        CaptureGeometry {
            display_id: DisplayId("test".into()),
            screen_origin: ScreenPoint { x: -100.0, y: 40.0 },
            scale_factor: 2.0,
            pixel_size: (64, 64),
        },
    )
    .unwrap()
}

fn services(capture: Arc<dyn CaptureBackend>) -> (ApplicationServices, Arc<Recognition>) {
    services_with_output(
        capture,
        SettingsStore::in_memory(AppSettings::default()),
        Arc::new(Output {
            fail_save: AtomicBool::new(true),
            ..Default::default()
        }),
    )
}

fn services_with_output(
    capture: Arc<dyn CaptureBackend>,
    settings: SettingsHandle,
    output: Arc<dyn ImageOutput>,
) -> (ApplicationServices, Arc<Recognition>) {
    let recognition = Arc::new(Recognition {
        started: Notify::new(),
        release: Notify::new(),
    });
    let services = ApplicationServices::new(ApplicationDependencies {
        settings,
        capture,
        text_recognition: recognition.clone(),
        qr_decoder: recognition.clone(),
        recognition_models: Some(recognition.clone()),
        output,
        notifications: Arc::new(Notices),
        shutter_sound: Arc::new(Notices),
        runtime: tokio::runtime::Handle::current(),
    });
    (services, recognition)
}

#[tokio::test]
async fn failed_output_remains_observable_with_notifications_disabled_and_keeps_its_input_for_retry() {
    let settings = SettingsStore::in_memory(AppSettings::default());
    settings.apply(SettingsAction::NotificationEnabled(false)).unwrap();
    let (services, _) = services_with_output(
        Arc::new(Capture { frame: frame() }),
        settings,
        Arc::new(Output {
            fail_save: AtomicBool::new(true),
            fail_copy: AtomicBool::new(true),
            ..Default::default()
        }),
    );
    let session = services.create_session(frame().image().clone(), 1);
    let snapshot = session.snapshot().unwrap();
    let context = services
        .publish_session_output(snapshot.id(), snapshot.revision(), ImageProjection::Full, None)
        .unwrap();
    let error = services.execute_image_command(ImageCommand::Copy, context.clone()).await.unwrap_err();
    assert_eq!(error, OperationError::new(ErrorKind::Native, "clipboard unavailable"));
    let error = services
        .execute_image_command(
            ImageCommand::Save {
                destination: SaveDestination::Preferences,
            },
            context.clone(),
        )
        .await
        .unwrap_err();
    assert_eq!(error, OperationError::new(ErrorKind::Io, "disk full"));
    assert!(services.export_receipts().is_empty());
    assert!(Arc::ptr_eq(snapshot.image(), session.snapshot().unwrap().image()));
    let ImageCommandResult::Saved(receipt) = services
        .execute_image_command(
            ImageCommand::Save {
                destination: SaveDestination::Preferences,
            },
            context,
        )
        .await
        .unwrap()
    else {
        panic!("successful retry returns its receipt");
    };
    services.close_session(snapshot.id());
    assert_eq!(services.export_receipts(), [receipt]);
    assert!(services.shutdown().await.drained);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn excess_exports_are_rejected_while_accepted_saves_survive_cancelled_or_dropped_waiters() {
    let paused = Arc::new(PausedSaves::default());
    let resume = ResumeSavesOnDrop(paused.clone());
    let (services, _) = services_with_output(
        Arc::new(Capture { frame: frame() }),
        SettingsStore::in_memory(AppSettings::default()),
        Arc::new(Output {
            paused_saves: Some(paused.clone()),
            ..Default::default()
        }),
    );
    let session = services.create_session(frame().image().clone(), 1);
    let snapshot = session.snapshot().unwrap();
    let context = services
        .publish_session_output(snapshot.id(), snapshot.revision(), ImageProjection::Full, None)
        .unwrap();
    let first = services.execute_image_command(
        ImageCommand::Save {
            destination: SaveDestination::Preferences,
        },
        context.clone(),
    );
    paused.entered.notified().await;
    let second = services.execute_image_command(
        ImageCommand::Save {
            destination: SaveDestination::Preferences,
        },
        context.clone(),
    );
    paused.entered.notified().await;
    let queued = [
        services.execute_image_command(
            ImageCommand::Save {
                destination: SaveDestination::Preferences,
            },
            context.clone(),
        ),
        services.execute_image_command(
            ImageCommand::Save {
                destination: SaveDestination::Preferences,
            },
            context.clone(),
        ),
    ];
    let excess = services.execute_image_command(
        ImageCommand::Save {
            destination: SaveDestination::Preferences,
        },
        context,
    );
    assert!(matches!(excess.phase(), JobPhase::Failed(error) if error.kind == ErrorKind::Busy));
    assert_eq!(excess.await.unwrap_err().kind, ErrorKind::Busy);
    assert!(services.export_receipts().is_empty(), "blocked writes have not committed");
    first.cancel();
    assert_eq!(first.phase(), JobPhase::CancellationRequested);
    drop(first);
    services.publish_session_revision(snapshot.id(), snapshot.revision() + 1).unwrap();
    services.close_session(snapshot.id());
    drop(resume);
    for job in std::iter::once(second).chain(queued) {
        assert!(matches!(job.await.unwrap(), ImageCommandResult::Saved(_)));
    }
    assert!(services.shutdown().await.drained);
    assert_eq!(
        services.export_receipts().len(),
        4,
        "only the four accepted saves may commit, including the dropped waiter"
    );
}

#[tokio::test]
async fn exports_use_default_or_exact_saved_directory_and_explicit_override_takes_precedence() {
    let settings = SettingsStore::in_memory(AppSettings::default());
    let (services, _) = services_with_output(
        Arc::new(Capture { frame: frame() }),
        settings.clone(),
        Arc::new(Output {
            fail_save: AtomicBool::new(false),
            ..Default::default()
        }),
    );
    for (saved, override_path, expected) in [
        (" \t ", None, std::path::Path::new("exports").join("snapshot.png")),
        (
            "  custom captures  ",
            None,
            std::path::Path::new("  custom captures  ").join("snapshot.png"),
        ),
        (
            "  custom captures  ",
            Some("explicit captures"),
            std::path::Path::new("explicit captures").join("snapshot.png"),
        ),
    ] {
        settings.apply(SettingsAction::SavePath(SaveDirectory::new(saved).unwrap())).unwrap();
        let context = ImageInput::image(frame().image().clone(), None).unwrap();
        let destination = override_path.map_or(SaveDestination::Preferences, |path| SaveDestination::Directory(path.into()));
        let ImageCommandResult::Saved(receipt) = services.execute_image_command(ImageCommand::Save { destination }, context).await.unwrap() else {
            panic!("the export must return its committed receipt");
        };
        assert_eq!(receipt.path, expected);
    }
    assert!(services.shutdown().await.drained);
}

#[tokio::test]
async fn prepared_long_captures_reject_excess_work_and_recover_after_cancel_or_finish() {
    let (services, _) = services(Arc::new(Capture { frame: frame() }));
    let prepare = || services.prepare_long_capture(CaptureTarget::Primary, ViewRect(RectF::new(0.0, 0.0, 32.0, 32.0)));
    let first = prepare().unwrap();
    let second = prepare().unwrap();
    assert_eq!(
        prepare().err().expect("the third prepared capture must be rejected").kind,
        ErrorKind::Busy
    );
    assert!(first.cancel());
    assert_eq!(first.wait().await.unwrap_err().kind, ErrorKind::Cancelled);
    let replacement = prepare().expect("a settled cancellation returns its outstanding slot");
    assert!(!second.request_finish(), "finishing a prepared capture settles it without acquisition");
    assert_eq!(second.wait().await.unwrap_err().kind, ErrorKind::Cancelled);
    let after_finish = prepare().expect("finishing before start also returns its outstanding slot");
    assert!(replacement.cancel());
    assert!(after_finish.cancel());
    assert_eq!(replacement.wait().await.unwrap_err().kind, ErrorKind::Cancelled);
    assert_eq!(after_finish.wait().await.unwrap_err().kind, ErrorKind::Cancelled);
    assert!(services.shutdown().await.drained);
}

#[tokio::test]
async fn late_ocr_after_a_view_is_closed_is_rejected_even_if_the_backend_ignores_cancellation() {
    let (services, recognition) = services(Arc::new(Capture { frame: frame() }));
    let session = services.create_session(frame().image().clone(), 1);
    let input = services.publish_session_output(session.id(), 1, ImageProjection::Full, None).unwrap();
    let task = services.recognize(input);
    recognition.started.notified().await;
    services.close_session(session.id());
    recognition.release.notify_one();
    assert_eq!(task.await.unwrap_err().kind, ErrorKind::Conflict);
    assert_eq!(services.job_records().last().unwrap().phase, JobPhase::Superseded);
    assert!(services.shutdown().await.drained);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn waiting_ocr_is_bounded_without_blocking_capture_or_feedback() {
    for waiting_count in [4, 8] {
        let (services, recognition) = services(Arc::new(Capture { frame: frame() }));
        let mut waiting_ocr = Vec::new();
        for _ in 0..waiting_count {
            waiting_ocr.push(services.recognize(ImageInput::image(frame().image().clone(), None).unwrap()));
            recognition.started.notified().await;
        }

        let excess = if waiting_count == 8 {
            let rejected = services.recognize(ImageInput::image(frame().image().clone(), None).unwrap());
            Some((rejected.phase(), tokio::time::timeout(std::time::Duration::from_secs(1), rejected).await))
        } else {
            None
        };

        // Every accepted call is genuinely inside the recognition port and cannot
        // complete until after these independent commands have completed.
        let independent = tokio::time::timeout(std::time::Duration::from_secs(1), async {
            let (capture, quick, copied, notified, shutter) = tokio::join!(
                services.capture_surface(CaptureTarget::Primary),
                services.quick_capture(),
                services.copy_text("selected text".into()),
                services.notify(NotificationRequest {
                    title: "Capture".into(),
                    message: "Copied".into(),
                    kind: minnow_application::ports::NotificationKind::Info,
                }),
                services.play_shutter(),
            );
            assert_eq!(capture.unwrap().frame.image().dimensions(), (64, 64));
            quick.unwrap();
            copied.unwrap();
            notified.unwrap();
            shutter.unwrap();
        })
        .await;

        recognition.release.notify_waiters();
        for job in waiting_ocr {
            job.await.unwrap();
        }
        services
            .ensure_models(false)
            .await
            .expect("settled OCR returns capacity to its recognition group");
        assert!(services.shutdown().await.drained);
        if let Some((phase, result)) = excess {
            assert!(matches!(phase, JobPhase::Failed(error) if error.kind == ErrorKind::Busy));
            assert_eq!(result.expect("rejection must be immediate").unwrap_err().kind, ErrorKind::Busy);
        }
        independent.expect("waiting for recognition must not reserve capture, output or feedback resources");
    }
}
#[tokio::test]
async fn ocr_availability_and_model_management_are_independent_from_qr() {
    struct SystemText(bool);
    impl TextRecognition for SystemText {
        fn availability(&self) -> RecognitionAvailability {
            if self.0 {
                RecognitionAvailability::Available
            } else {
                RecognitionAvailability::Unavailable {
                    reason: "system OCR is unavailable".into(),
                }
            }
        }

        fn recognize(&self, _: ImageRef, _: CancellationToken) -> OperationFuture<'_, Vec<OcrBlock>> {
            assert!(self.0, "an unavailable port must not be invoked");
            Box::pin(async { Ok(vec![OcrBlock::new("system text", NormalizedOcrGeometry::new(0.5, 0.5, 0.5, 0.2, 0.0)?)]) })
        }
    }
    struct QrOnly;
    impl QrDecoder for QrOnly {
        fn decode_qr(&self, _: &RgbaImage) -> OperationResult<Option<String>> {
            Ok(Some("independent QR decoder".into()))
        }
    }

    for available in [true, false] {
        let services = ApplicationServices::new(ApplicationDependencies {
            settings: SettingsStore::in_memory(AppSettings::default()),
            capture: Arc::new(Capture { frame: frame() }),
            text_recognition: Arc::new(SystemText(available)),
            qr_decoder: Arc::new(QrOnly),
            recognition_models: None,
            output: Arc::new(Output {
                fail_save: AtomicBool::new(false),
                ..Default::default()
            }),
            notifications: Arc::new(Notices),
            shutter_sound: Arc::new(Notices),
            runtime: tokio::runtime::Handle::current(),
        });
        assert_eq!(services.recognition_availability().is_available(), available);
        assert!(services.model_status().is_none());
        assert!(services.subscribe_model_status().is_none());
        assert_eq!(services.ensure_models(false).await.unwrap_err().kind, ErrorKind::Unavailable);
        let text = services.recognize(ImageInput::image(frame().image().clone(), None).unwrap()).await;
        if available {
            assert_eq!(text.unwrap()[0].text, "system text");
        } else {
            assert_eq!(text.unwrap_err().kind, ErrorKind::Unavailable);
        }
        let qr = services
            .execute_image_command(ImageCommand::CopyQr, ImageInput::image(frame().image().clone(), None).unwrap())
            .await
            .unwrap();
        assert!(matches!(qr, ImageCommandResult::QrCopied(text) if text == "independent QR decoder"));
        assert!(services.shutdown().await.drained);
    }
}

#[tokio::test]
async fn preparing_a_selection_carries_cropped_pixels_and_desktop_placement_without_ui_intent() {
    let (services, _) = services(Arc::new(Capture { frame: frame() }));
    for (selection, dimensions, source_bounds) in [
        (RectF::new(2.0, 3.0, 4.0, 5.0), (8, 10), RectF::new(-98.0, 43.0, 4.0, 5.0)),
        (RectF::new(-0.25, 3.1, 4.2, 5.0), (8, 11), RectF::new(-100.0, 43.0, 4.0, 5.5)),
    ] {
        let input = ImageInput::selection(frame(), ViewRect(selection), None).unwrap();
        let prepared = services.prepare_image(input).await.unwrap();
        assert_eq!(prepared.image.dimensions(), dimensions);
        assert_eq!(prepared.source_bounds, Some(source_bounds));
    }
    services.shutdown().await;
}

#[tokio::test]
async fn session_publication_binds_registered_pixels_and_failed_geometry_does_not_advance_revision() {
    let (services, _) = services(Arc::new(Capture { frame: frame() }));
    let source = frame();
    let session = services.create_capture_session(source.clone(), 4);
    let selection = ImageProjection::Selection(ViewRect(RectF::new(2.0, 3.0, 4.0, 5.0)));
    let input = services.publish_session_output(session.id(), 4, selection, None).unwrap();
    assert_eq!(input.scope(), session.scope().unwrap());
    let bad_projection = ImageProjection::Selection(ViewRect(RectF::new(500.0, 500.0, 4.0, 5.0)));
    let error = services.publish_session_output(session.id(), 5, bad_projection, None).unwrap_err();
    assert_eq!(error.kind, ErrorKind::InvalidInput);
    let snapshot = session.snapshot().unwrap();
    assert_eq!(snapshot.revision(), 4);
    assert!(Arc::ptr_eq(snapshot.image(), source.image()));
    assert!(services.is_current(input.scope()));
    let prepared = services.prepare_image(input.clone()).await.unwrap();
    assert_eq!(prepared.image.dimensions(), (8, 10));
    assert_eq!(prepared.image.pixels().get_pixel(0, 0), source.image().pixels().get_pixel(4, 6));
    assert_eq!(prepared.source_bounds, Some(RectF::new(-98.0, 43.0, 4.0, 5.0)));

    services.publish_session_revision(session.id(), 5).unwrap();
    assert_eq!(services.prepare_image(input).await.unwrap_err().kind, ErrorKind::Conflict);
    assert_eq!(
        services
            .publish_session_output(session.id(), 4, ImageProjection::Full, None)
            .unwrap_err()
            .kind,
        ErrorKind::Conflict,
    );
    services.close_session(session.id());
    assert_eq!(
        services
            .publish_session_output(session.id(), 6, ImageProjection::Full, None)
            .unwrap_err()
            .kind,
        ErrorKind::Conflict,
    );
    assert!(services.shutdown().await.drained);
}

#[tokio::test]
async fn a_revision_binds_one_output_and_idempotent_publication_keeps_existing_derived_work_current() {
    let (services, recognition) = services(Arc::new(Capture { frame: frame() }));
    let session = services.create_capture_session(frame(), 7);
    let mut editor = minnow_editor::AnnotationEngine::default();
    editor.set_tool(minnow_editor::AnnotationTool::Rectangle);
    let canvas = Some(RectF::new(0.0, 0.0, 32.0, 32.0));
    assert!(editor.start_draw((2.0, 2.0), canvas, true));
    assert!(editor.update_interaction((14.0, 14.0), canvas));
    assert!(editor.finish_interaction(8.0));
    let requested = ImageProjection::Selection(ViewRect(RectF::new(0.1, 0.1, 9.8, 9.8)));
    let canonical = ImageProjection::Selection(ViewRect(RectF::new(0.0, 0.0, 10.0, 10.0)));
    let input = services
        .publish_session_output(session.id(), 7, requested, Some(editor.snapshot()))
        .unwrap();
    let job = services.recognize(input.clone());
    recognition.started.notified().await;
    for _ in 0..3 {
        services.publish_session_revision(session.id(), 7).unwrap();
        let repeated = services
            .publish_session_output(session.id(), 7, canonical, Some(editor.prepare_output_snapshot(8.0).unwrap()))
            .expect("fresh snapshots of unchanged content and equivalent crops are idempotent");
        assert_eq!(repeated.scope(), input.scope());
    }
    let different_crop = ImageProjection::Selection(ViewRect(RectF::new(1.0, 0.0, 10.0, 10.0)));
    services.publish_session_revision(session.id(), 7).unwrap();
    assert_eq!(
        services
            .publish_session_output(session.id(), 7, different_crop, Some(editor.snapshot()))
            .unwrap_err()
            .kind,
        ErrorKind::Conflict,
    );
    let mut edited = editor.clone();
    assert!(edited.set_color(0x123456ff));
    assert_eq!(
        services
            .publish_session_output(session.id(), 7, canonical, Some(edited.snapshot()))
            .unwrap_err()
            .kind,
        ErrorKind::Conflict,
    );
    let invalid = ImageProjection::Selection(ViewRect(RectF::new(500.0, 500.0, 10.0, 10.0)));
    assert_eq!(
        services
            .publish_session_output(session.id(), 8, invalid, Some(edited.snapshot()))
            .unwrap_err()
            .kind,
        ErrorKind::InvalidInput,
    );
    assert_eq!(session.snapshot().unwrap().revision(), 7);
    services
        .publish_session_output(session.id(), 7, requested, Some(editor.snapshot()))
        .unwrap();
    assert_eq!(
        job.phase(),
        JobPhase::Running,
        "neither idempotent nor rejected publication cancels valid work"
    );
    services.publish_session_revision(session.id(), 8).unwrap();
    assert_eq!(
        job.phase(),
        JobPhase::CancellationRequested,
        "an actual document revision still invalidates work immediately"
    );
    recognition.release.notify_one();
    assert_eq!(job.await.unwrap_err().kind, ErrorKind::Conflict);
    let current = services
        .publish_session_output(session.id(), 8, canonical, Some(edited.snapshot()))
        .unwrap();
    services.prepare_image(current).await.unwrap();
    assert!(services.shutdown().await.drained);
}

#[tokio::test]
async fn empty_annotations_and_full_capture_selection_have_one_canonical_publication() {
    let (services, _) = services(Arc::new(Capture { frame: frame() }));
    let session = services.create_capture_session(frame(), 1);
    let input = services.publish_session_output(session.id(), 1, ImageProjection::Full, None).unwrap();
    let selected = ImageProjection::Selection(ViewRect(RectF::new(0.0, 0.0, 32.0, 32.0)));
    services
        .publish_session_output(session.id(), 1, selected, Some(minnow_editor::AnnotationEngine::default().snapshot()))
        .unwrap();
    services
        .publish_session_output(
            session.id(),
            1,
            ImageProjection::Full,
            Some(minnow_editor::AnnotationEngine::default().snapshot()),
        )
        .unwrap();
    assert!(services.is_current(input.scope()));
    assert_eq!(services.prepare_image(input).await.unwrap().image.dimensions(), (64, 64));
    assert!(services.shutdown().await.drained);
}

#[tokio::test]
async fn incomplete_rendering_never_advances_publication_or_commits_an_external_effect() {
    let (services, _) = services_with_output(
        Arc::new(Capture { frame: frame() }),
        SettingsStore::in_memory(AppSettings::default()),
        Arc::new(Output {
            fail_copy: AtomicBool::new(true),
            ..Default::default()
        }),
    );
    let image = frame().image().clone();
    let session = services.create_session(image.clone(), 1);
    let mut editor = minnow_editor::AnnotationEngine::default();
    editor.set_tool(minnow_editor::AnnotationTool::Text);
    assert!(editor.start_draw((2.0, 3.0), Some(RectF::new(0.0, 0.0, 64.0, 64.0)), true));
    let snapshot = editor.prepare_output_snapshot(8.0).unwrap();
    let expected = OperationError::new(ErrorKind::Unavailable, minnow_editor::AnnotationOutputError::MissingFont.to_string());
    assert_eq!(
        services
            .publish_session_output(session.id(), 2, ImageProjection::Full, Some(snapshot.clone()))
            .unwrap_err(),
        expected,
    );
    assert_eq!(session.snapshot().unwrap().revision(), 1);
    assert_eq!(ImageInput::image(image, Some(snapshot)).unwrap_err(), expected);
    let valid = services.publish_session_output(session.id(), 1, ImageProjection::Full, None).unwrap();
    assert_eq!(
        services.execute_image_command(ImageCommand::Copy, valid).await.unwrap_err(),
        OperationError::new(ErrorKind::Native, "clipboard unavailable"),
        "an invalid render must not even call the external output port",
    );
    assert!(services.shutdown().await.drained);
}

#[tokio::test]
async fn standalone_inputs_cannot_claim_a_document_scope_and_session_credentials_are_not_reused_between_services() {
    assert_eq!(
        ImageInput::image(ImageAsset::new(RgbaImage::new(0, 0)), None).unwrap_err().kind,
        ErrorKind::InvalidInput,
        "empty standalone sources must be rejected before a job can be submitted",
    );
    let (first, _) = services(Arc::new(Capture { frame: frame() }));
    let (second, _) = services(Arc::new(Capture { frame: frame() }));
    let source = frame().image().clone();
    let a = first.create_session(source.clone(), 1);
    let b = second.create_session(source.clone(), 1);
    assert_ne!(a.id(), b.id());
    let input = first.publish_session_output(a.id(), 1, ImageProjection::Full, None).unwrap();
    assert_eq!(second.prepare_image(input).await.unwrap_err().kind, ErrorKind::Conflict);
    assert_eq!(
        second.publish_session_output(a.id(), 1, ImageProjection::Full, None).unwrap_err().kind,
        ErrorKind::Conflict,
    );

    let external = ImageAsset::new(RgbaImage::from_pixel(3, 2, Rgba([9, 8, 7, 255])));
    let frozen = ImageInput::image(external.clone(), None).unwrap();
    assert_eq!(frozen.scope(), JobScope::Application);
    first.publish_session_revision(a.id(), 2).unwrap();
    first.close_session(a.id());
    let prepared = first.prepare_image(frozen).await.unwrap();
    assert!(Arc::ptr_eq(&prepared.image, &external));
    assert!(first.shutdown().await.drained);
    assert!(second.shutdown().await.drained);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_finished_long_capture_can_be_exported_again_after_an_output_failure() {
    let (services, _) = services(Arc::new(Capture { frame: frame() }));
    let run = services
        .start_long_capture(CaptureTarget::Primary, ViewRect(RectF::new(0.0, 0.0, 32.0, 32.0)))
        .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while run.preview().is_none() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let image = run.finish().await.unwrap();
    let first = ImageInput::image(image.clone(), None).unwrap();
    assert_eq!(
        services
            .execute_image_command(
                ImageCommand::Save {
                    destination: SaveDestination::Preferences
                },
                first
            )
            .await
            .unwrap_err()
            .kind,
        ErrorKind::Io
    );
    let retained = run.clone().finish().await.unwrap();
    assert!(Arc::ptr_eq(&image, &retained));
    assert!(Arc::ptr_eq(&image, &run.result().unwrap()));
    assert!(matches!(
        services
            .execute_image_command(
                ImageCommand::Save {
                    destination: SaveDestination::Preferences
                },
                ImageInput::image(retained, None).unwrap()
            )
            .await,
        Ok(ImageCommandResult::Saved(_))
    ));
    services.shutdown().await;
}

#[tokio::test]
async fn preparing_a_long_capture_does_not_acquire_pixels_and_can_be_cancelled_before_start() {
    struct ObservedCapture(Arc<AtomicBool>);
    impl CaptureBackend for ObservedCapture {
        fn capture(&self, _: &CaptureTarget) -> Result<CapturedFrame, CaptureError> {
            self.0.store(true, Ordering::SeqCst);
            Ok(frame())
        }
        fn displays(&self) -> Result<Vec<DisplayInfo>, CaptureError> {
            Ok(vec![])
        }
        fn windows(&self, _: &DisplayId) -> Result<Vec<WindowInfo>, CaptureError> {
            Ok(vec![])
        }
    }
    let called = Arc::new(AtomicBool::new(false));
    let (services, _) = services(Arc::new(ObservedCapture(called.clone())));
    let handle = services
        .prepare_long_capture(CaptureTarget::Primary, ViewRect(RectF::new(0.0, 0.0, 32.0, 32.0)))
        .unwrap();
    assert!(matches!(handle.status(), LongCaptureStatus::Queued));
    assert!(!called.load(Ordering::SeqCst));
    assert!(handle.clone().cancel());
    assert!(!handle.start());
    assert_eq!(handle.wait().await.unwrap_err().kind, ErrorKind::Cancelled);
    assert!(services.shutdown().await.drained);
    assert!(!called.load(Ordering::SeqCst));
}

#[tokio::test]
async fn finishing_a_prepared_capture_settles_without_starting_acquisition() {
    let (services, _) = services(Arc::new(Capture { frame: frame() }));
    let handle = services
        .prepare_long_capture(CaptureTarget::Primary, ViewRect(RectF::new(0.0, 0.0, 32.0, 32.0)))
        .unwrap();
    let result = tokio::time::timeout(std::time::Duration::from_secs(1), handle.finish())
        .await
        .expect("finish before start must reach a terminal state");
    assert_eq!(result.unwrap_err().kind, ErrorKind::Cancelled);
    assert!(matches!(handle.status(), LongCaptureStatus::Cancelled { partial: None }));
    assert!(!handle.start());
    assert!(handle.result().is_none());
    assert!(services.shutdown().await.drained);
}
