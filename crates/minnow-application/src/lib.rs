//! Framework-independent application contracts and use cases.

mod actions;
mod jobs;
mod long_capture;
pub mod ports;
pub mod preferences;
mod services;
mod sessions;

pub use actions::{ImageCommand, ImageCommandResult, ImageInput, ImageProjection, PreparedImage, SaveDestination};
pub(crate) use jobs::{CompletionPolicy, JobAdmission, JobResource, JobSupervisor};
pub use jobs::{JobHandle, JobId, JobPhase, JobRecord, ShutdownReport};
pub use long_capture::{LongCaptureHandle, LongCaptureStatus};
/// Capture protocols and configuration accepted by application commands.
pub mod capture {
    pub use minnow_capture::{
        CaptureBackend, CaptureError, CaptureOutcome, CaptureRunConfig, CaptureTarget, DisplayInfo, PreviewSnapshot, ScrollCaptureConfig,
        StitchConfig, WindowInfo,
    };
}
pub use minnow_capture::{CaptureBackend, CaptureError, CaptureTarget, DisplayInfo, WindowInfo};
pub use minnow_editor::AnnotationSnapshot;
pub use minnow_imaging::{CaptureGeometry, CapturedFrame, DisplayId, ImageAsset, ImageRef, Rect, RectF, ScreenPoint, ViewRect};
/// Preference values, commands and observable persistence state.
pub mod settings {
    pub use minnow_settings::{
        AppSettings, DEFAULT_CAPTURE_SHORTCUT, DEFAULT_QUICK_CAPTURE_SHORTCUT, GeneralSettings, NotificationSettings, OcrModelType, OcrSettings,
        OutputSettings, PersistenceStage, PersistenceStatus, SaveDirectory, SaveDirectoryError, SettingsAction, SettingsError, SettingsLoadStatus,
        SettingsRevision, ShortcutSettings, THEME_DARK, THEME_LIGHT, THEME_SYSTEM,
    };
}
pub use ports::{ErrorKind, OperationError, OperationFuture, OperationResult};
pub use services::{ApplicationDependencies, ApplicationServices, CaptureSurface};
pub use sessions::{JobScope, SessionHandle, SessionId, SessionSnapshot};
pub use settings::{AppSettings, SettingsAction};
