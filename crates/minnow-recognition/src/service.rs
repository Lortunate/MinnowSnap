use minnow_application::ports::{
    ModelStatus, OcrBlock, OperationFuture, OperationResult, RecognitionAvailability, RecognitionModels, TextRecognition,
};
use minnow_application::settings::OcrModelType;
use minnow_imaging::ImageRef;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::runtime::Handle;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

use crate::{InferenceBudget, ModelSources};

/// Resource and lifetime policy for a single application-level recognition service.
#[derive(Clone)]
pub struct RecognitionOptions {
    pub model_type: OcrModelType,
    pub sources: Option<ModelSources>,
    pub max_concurrent_inference: usize,
    pub inference_threads: usize,
    pub inference_budget: InferenceBudget,
    pub max_artifact_bytes: u64,
    /// The service uses a child token, so dropping it cannot cancel its parent.
    pub shutdown: CancellationToken,
}

impl Default for RecognitionOptions {
    fn default() -> Self {
        Self {
            model_type: OcrModelType::default(),
            sources: None,
            max_concurrent_inference: 1,
            inference_threads: 4,
            inference_budget: InferenceBudget::default(),
            max_artifact_bytes: 512 * 1024 * 1024,
            shutdown: CancellationToken::new(),
        }
    }
}

/// Application-owned OCR lifetime, including builds without a local OCR runtime.
///
/// Unavailable OCR does not construct model storage, HTTP clients, or native
/// runtime objects. QR decoding has its own independent [`crate::LocalQrDecoder`].
#[derive(Clone)]
pub struct RecognitionService {
    state: Arc<State>,
}

enum State {
    #[cfg(feature = "local-ocr")]
    Local(crate::local::LocalRecognitionService),
    Unavailable {
        availability: RecognitionAvailability,
        status: watch::Sender<ModelStatus>,
    },
}

impl RecognitionService {
    pub fn new(models_dir: impl Into<PathBuf>, runtime: Handle, options: RecognitionOptions) -> OperationResult<Self> {
        options.inference_budget.validate()?;
        #[cfg(feature = "local-ocr")]
        {
            match crate::local::LocalRecognitionService::new(models_dir, runtime, options) {
                Ok(local) => Ok(Self {
                    state: Arc::new(State::Local(local)),
                }),
                Err(error) if error.kind == minnow_application::ports::ErrorKind::Unavailable => Ok(Self::unavailable(error.message)),
                Err(error) => Err(error),
            }
        }
        #[cfg(not(feature = "local-ocr"))]
        {
            let _ = (models_dir, runtime, options);
            Ok(Self::unavailable(
                "Local OCR is unavailable: this build does not include the local-ocr feature",
            ))
        }
    }

    fn unavailable(reason: impl Into<String>) -> Self {
        let (status, _) = watch::channel(ModelStatus::default());
        Self {
            state: Arc::new(State::Unavailable {
                availability: RecognitionAvailability::Unavailable { reason: reason.into() },
                status,
            }),
        }
    }

    /// Cancels service-owned work and acknowledges completion of blocking inference.
    pub async fn shutdown(&self) -> OperationResult<()> {
        match self.state.as_ref() {
            #[cfg(feature = "local-ocr")]
            State::Local(local) => local.shutdown().await,
            State::Unavailable { .. } => Ok(()),
        }
    }
}

impl TextRecognition for RecognitionService {
    fn availability(&self) -> RecognitionAvailability {
        match self.state.as_ref() {
            #[cfg(feature = "local-ocr")]
            State::Local(local) => local.availability(),
            State::Unavailable { availability, .. } => availability.clone(),
        }
    }

    fn recognize(&self, image: ImageRef, cancel: CancellationToken) -> OperationFuture<'_, Vec<OcrBlock>> {
        match self.state.as_ref() {
            #[cfg(feature = "local-ocr")]
            State::Local(local) => local.recognize(image, cancel),
            State::Unavailable { availability, .. } => {
                let _ = (image, cancel);
                Box::pin(async move {
                    availability.require_available()?;
                    Ok(Vec::new())
                })
            }
        }
    }
}

impl RecognitionModels for RecognitionService {
    fn model_status(&self) -> ModelStatus {
        match self.state.as_ref() {
            #[cfg(feature = "local-ocr")]
            State::Local(local) => local.model_status(),
            State::Unavailable { status, .. } => status.borrow().clone(),
        }
    }

    fn subscribe_model_status(&self) -> watch::Receiver<ModelStatus> {
        match self.state.as_ref() {
            #[cfg(feature = "local-ocr")]
            State::Local(local) => local.subscribe_model_status(),
            State::Unavailable { status, .. } => status.subscribe(),
        }
    }

    fn ensure_models(&self, force: bool, cancel: CancellationToken) -> OperationFuture<'_, ()> {
        match self.state.as_ref() {
            #[cfg(feature = "local-ocr")]
            State::Local(local) => local.ensure_models(force, cancel),
            State::Unavailable { availability, .. } => {
                let _ = (force, cancel);
                Box::pin(async move { availability.require_available() })
            }
        }
    }
}

#[cfg(all(test, any(not(feature = "local-ocr"), all(target_os = "macos", target_arch = "x86_64"))))]
mod tests {
    use super::*;
    use minnow_application::ports::{ErrorKind, QrDecoder};
    use minnow_imaging::ImageAsset;

    #[tokio::test]
    async fn unavailable_build_preserves_qr_and_returns_unavailable_without_model_work() {
        let models = std::env::temp_dir().join(format!("minnowsnap-base-contract-{}", std::process::id()));
        assert!(!models.exists());
        let service = RecognitionService::new(&models, Handle::current(), RecognitionOptions::default()).unwrap();
        assert!(!service.availability().is_available());
        let before = service.model_status();
        let status = service.subscribe_model_status();
        let image = ImageAsset::new(image::RgbaImage::new(2, 2));
        assert_eq!(
            service.recognize(image, CancellationToken::new()).await.unwrap_err().kind,
            ErrorKind::Unavailable
        );
        assert_eq!(
            service.ensure_models(true, CancellationToken::new()).await.unwrap_err().kind,
            ErrorKind::Unavailable
        );
        assert_eq!(service.model_status(), before);
        assert_eq!(*status.borrow(), before);
        assert!(!models.exists());
        assert_eq!(crate::LocalQrDecoder::new().decode_qr(&image::RgbaImage::new(8, 8)).unwrap(), None);
        service.shutdown().await.unwrap();
    }
}
