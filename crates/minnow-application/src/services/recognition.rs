use super::{ApplicationServices, blocking};
use crate::ports::{ModelStatus, OcrBlock, RecognitionAvailability};
use crate::{CompletionPolicy, ErrorKind, ImageInput, JobAdmission, JobHandle, JobResource, JobScope, OperationError};
use tokio::sync::watch;

impl ApplicationServices {
    pub fn model_status(&self) -> Option<ModelStatus> {
        self.recognition_models.as_ref().map(|models| models.model_status())
    }

    pub fn subscribe_model_status(&self) -> Option<watch::Receiver<ModelStatus>> {
        self.recognition_models.as_ref().map(|models| models.subscribe_model_status())
    }

    pub fn recognition_availability(&self) -> RecognitionAvailability {
        self.text_recognition.availability()
    }

    pub fn recognize(&self, input: ImageInput) -> JobHandle<Vec<OcrBlock>> {
        let recognition = self.text_recognition.clone();
        let runtime = self.runtime.clone();
        let jobs = self.jobs.clone();
        self.jobs.spawn(
            "recognize text",
            input.scope(),
            CompletionPolicy::RequireCurrentSession,
            JobAdmission::Recognition,
            move |cancel| async move {
                recognition.availability().require_available()?;
                let execution = jobs.acquire(JobResource::ImageOutput, &cancel).await?;
                let image = blocking(&runtime, move || input.resolve()).await?.image;
                drop(execution);
                if cancel.is_cancelled() {
                    return Err(OperationError::cancelled());
                }
                recognition.recognize(image, cancel).await
            },
        )
    }

    pub fn ensure_models(&self, force: bool) -> JobHandle<()> {
        let recognition = self.recognition_models.clone();
        self.jobs.spawn(
            "prepare recognition models",
            JobScope::Application,
            CompletionPolicy::PreserveResult,
            JobAdmission::Recognition,
            move |cancel| async move {
                let recognition =
                    recognition.ok_or_else(|| OperationError::new(ErrorKind::Unavailable, "this OCR provider has no managed models"))?;
                recognition.ensure_models(force, cancel).await
            },
        )
    }
}
