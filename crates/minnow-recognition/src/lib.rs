//! Local recognition adapters with application-owned lifetime and model generations.
//!
//! A service instance is shared for the lifetime of the application. A caller's
//! cancellation token cancels that caller's wait; it never cancels a model update
//! shared by other callers. [`RecognitionService::shutdown`] cancels service work
//! and waits for download and blocking inference to finish.

mod budget;
mod catalog;
#[cfg(feature = "local-ocr")]
mod local;
#[cfg(feature = "local-ocr")]
mod models;
#[cfg(feature = "local-ocr")]
mod ocr;
mod qr;
mod service;
mod sources;

pub use budget::InferenceBudget;
pub use minnow_application::ports::{ModelDownloadStatus, ModelStatus, NormalizedOcrGeometry, OcrBlock, RecognitionAvailability};
pub use minnow_application::settings::OcrModelType;
pub use qr::LocalQrDecoder;
pub use service::{RecognitionOptions, RecognitionService};
pub use sources::{ArtifactSpec, ModelSources};
