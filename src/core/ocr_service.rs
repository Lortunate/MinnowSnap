use crate::core::settings::SETTINGS;
use std::sync::Arc;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OcrModelStatus {
    Missing,
    Downloading { progress_percent: u8 },
    Ready,
    Failed { message: String },
}

impl OcrModelStatus {
    pub fn is_downloading(&self) -> bool {
        matches!(self, Self::Downloading { .. })
    }

    pub fn progress_percent(&self) -> u8 {
        match self {
            Self::Downloading { progress_percent } => *progress_percent,
            Self::Ready => 100,
            _ => 0,
        }
    }

    pub fn error_message(&self) -> Option<&str> {
        match self {
            Self::Failed { message } => Some(message.as_str()),
            _ => None,
        }
    }
}

pub fn is_enabled() -> bool {
    SETTINGS.lock().unwrap().get().ocr.enabled
}

pub fn set_enabled(enabled: bool) {
    SETTINGS.lock().unwrap().set_ocr_enabled(enabled);
}

pub fn mobile_models_ready() -> bool {
    ocr::check_models_ready(ocr::OcrModelType::Mobile)
}

pub fn model_status_from(ready: bool, is_downloading: bool, progress_percent: u8, last_error: Option<String>) -> OcrModelStatus {
    if is_downloading {
        return OcrModelStatus::Downloading { progress_percent };
    }

    if ready {
        return OcrModelStatus::Ready;
    }

    if let Some(message) = last_error {
        return OcrModelStatus::Failed { message };
    }

    OcrModelStatus::Missing
}

pub fn current_status(is_downloading: bool, progress_percent: u8, last_error: Option<String>) -> OcrModelStatus {
    model_status_from(mobile_models_ready(), is_downloading, progress_percent, last_error)
}

pub async fn download_mobile_models(force: bool, on_progress: Option<Arc<dyn Fn(f32) + Send + Sync>>) -> Result<(), String> {
    ocr::download_models(ocr::OcrModelType::Mobile, force, on_progress)
        .await
        .map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::{OcrModelStatus, model_status_from};

    #[test]
    fn status_maps_missing_downloading_ready_and_failed() {
        assert_eq!(model_status_from(false, false, 0, None), OcrModelStatus::Missing);
        assert_eq!(
            model_status_from(false, true, 42, None),
            OcrModelStatus::Downloading { progress_percent: 42 }
        );
        assert_eq!(model_status_from(true, false, 0, None), OcrModelStatus::Ready);
        assert_eq!(
            model_status_from(false, false, 0, Some("boom".to_string())),
            OcrModelStatus::Failed { message: "boom".to_string() }
        );
    }

    #[test]
    fn status_helpers_report_progress_and_errors() {
        let downloading = OcrModelStatus::Downloading { progress_percent: 42 };
        assert!(downloading.is_downloading());
        assert_eq!(downloading.progress_percent(), 42);
        assert_eq!(downloading.error_message(), None);

        let ready = OcrModelStatus::Ready;
        assert!(!ready.is_downloading());
        assert_eq!(ready.progress_percent(), 100);
        assert_eq!(ready.error_message(), None);

        let failed = OcrModelStatus::Failed {
            message: "network".to_string(),
        };
        assert!(!failed.is_downloading());
        assert_eq!(failed.progress_percent(), 0);
        assert_eq!(failed.error_message(), Some("network"));
    }
}
