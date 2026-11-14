use image::RgbaImage;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;

mod recognition;
pub use recognition::{
    ModelDownloadStatus, ModelStatus, NormalizedOcrGeometry, OcrBlock, QrDecoder, RecognitionAvailability, RecognitionModels, TextRecognition,
};

pub type OperationResult<T> = Result<T, OperationError>;
pub type OperationFuture<'a, T> = Pin<Box<dyn Future<Output = OperationResult<T>> + Send + 'a>>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorKind {
    Cancelled,
    /// The per-purpose outstanding limit is full; the request was not accepted.
    Busy,
    Unavailable,
    InvalidInput,
    Conflict,
    Io,
    Download,
    Validation,
    ModelUnavailable,
    Inference,
    Task,
    Native,
    CompletionUnknown,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("{message}")]
pub struct OperationError {
    pub kind: ErrorKind,
    pub message: String,
}

impl OperationError {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub fn cancelled() -> Self {
        Self::new(ErrorKind::Cancelled, "operation cancelled")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportOptions {
    pub directory: PathBuf,
    pub optimize: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportReceipt {
    pub path: PathBuf,
    pub bytes_written: u64,
}

pub trait ImageOutput: Send + Sync {
    fn copy_image(&self, image: &RgbaImage) -> OperationResult<()>;
    fn copy_text(&self, text: &str) -> OperationResult<()>;
    fn default_directory(&self) -> OperationResult<PathBuf>;
    fn save_image(&self, image: &RgbaImage, options: &ExportOptions) -> OperationResult<ExportReceipt>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NotificationKind {
    Save,
    Copy,
    QrCode,
    Info,
}

#[derive(Clone, Debug)]
pub struct NotificationRequest {
    pub title: String,
    pub message: String,
    pub kind: NotificationKind,
}

pub trait Notifications: Send + Sync {
    fn show(&self, request: &NotificationRequest) -> OperationResult<()>;
}

pub trait ShutterSound: Send + Sync {
    fn play(&self) -> OperationResult<()>;
}
