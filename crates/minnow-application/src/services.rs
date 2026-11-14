mod capture;
mod feedback;
mod image_actions;
mod lifecycle;
mod recognition;

use crate::jobs::JobSupervisor;
use crate::ports::{ExportReceipt, ImageOutput, Notifications, QrDecoder, RecognitionModels, ShutterSound, TextRecognition};
use crate::preferences::PreferencesService;
use crate::sessions::SessionRegistry;
use crate::{ErrorKind, JobRecord, OperationError, OperationResult};
use image_actions::ExportHistory;
use minnow_capture::{CaptureBackend, CaptureError, WindowInfo};
use minnow_imaging::CapturedFrame;
use minnow_settings::{AppSettings, SettingsHandle};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::runtime::Handle;

/// Concrete dependencies are selected once by the executable composition root.
pub struct ApplicationDependencies {
    pub settings: SettingsHandle,
    pub capture: Arc<dyn CaptureBackend>,
    pub text_recognition: Arc<dyn TextRecognition>,
    pub qr_decoder: Arc<dyn QrDecoder>,
    pub recognition_models: Option<Arc<dyn RecognitionModels>>,
    pub output: Arc<dyn ImageOutput>,
    pub notifications: Arc<dyn Notifications>,
    pub shutter_sound: Arc<dyn ShutterSound>,
    pub runtime: Handle,
}

/// A captured image remains useful even if enumerating desktop windows fails.
#[derive(Clone, Debug)]
pub struct CaptureSurface {
    pub frame: CapturedFrame,
    pub windows: Vec<WindowInfo>,
    pub warnings: Vec<OperationError>,
}

/// Application commands and queries. Native ports remain owned by this boundary.
pub struct ApplicationServices {
    preferences: PreferencesService,
    capture: Arc<dyn CaptureBackend>,
    text_recognition: Arc<dyn TextRecognition>,
    qr_decoder: Arc<dyn QrDecoder>,
    recognition_models: Option<Arc<dyn RecognitionModels>>,
    output: Arc<dyn ImageOutput>,
    notifications: Arc<dyn Notifications>,
    shutter_sound: Arc<dyn ShutterSound>,
    runtime: Handle,
    sessions: SessionRegistry,
    jobs: JobSupervisor,
    exports: ExportHistory,
}

impl ApplicationServices {
    pub fn new(dependencies: ApplicationDependencies) -> Self {
        let sessions = SessionRegistry::default();
        let jobs = JobSupervisor::new(dependencies.runtime.clone(), sessions.clone());
        Self {
            preferences: PreferencesService::new(dependencies.settings),
            capture: dependencies.capture,
            text_recognition: dependencies.text_recognition,
            qr_decoder: dependencies.qr_decoder,
            recognition_models: dependencies.recognition_models,
            output: dependencies.output,
            notifications: dependencies.notifications,
            shutter_sound: dependencies.shutter_sound,
            runtime: dependencies.runtime,
            sessions,
            jobs,
            exports: ExportHistory::default(),
        }
    }

    pub fn preferences(&self) -> &PreferencesService {
        &self.preferences
    }

    pub fn settings_snapshot(&self) -> AppSettings {
        self.preferences.snapshot()
    }

    pub fn default_output_directory(&self) -> OperationResult<PathBuf> {
        self.output.default_directory()
    }

    pub fn job_records(&self) -> Vec<JobRecord> {
        self.jobs.records()
    }

    pub fn export_receipts(&self) -> Vec<ExportReceipt> {
        self.exports.receipts()
    }

    pub(crate) fn capture_backend(&self) -> &Arc<dyn CaptureBackend> {
        &self.capture
    }

    pub(crate) fn jobs(&self) -> &JobSupervisor {
        &self.jobs
    }

    pub(crate) fn runtime(&self) -> &Handle {
        &self.runtime
    }
}

pub(crate) async fn blocking<T, F>(runtime: &Handle, run: F) -> OperationResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> OperationResult<T> + Send + 'static,
{
    runtime
        .spawn_blocking(run)
        .await
        .map_err(|error| OperationError::new(ErrorKind::Task, format!("blocking operation failed: {error}")))?
}

pub(crate) fn capture_error(error: CaptureError) -> OperationError {
    let kind = match &error {
        CaptureError::Cancelled => ErrorKind::Cancelled,
        CaptureError::Empty | CaptureError::Geometry(_) | CaptureError::InvalidStitchConfig { .. } => ErrorKind::InvalidInput,
        CaptureError::TargetChanged => ErrorKind::Conflict,
        _ => ErrorKind::Native,
    };
    OperationError::new(kind, error.to_string())
}
