use crate::settings::SETTINGS;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::task::JoinError;

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
    minnow_ocr::check_models_ready(minnow_ocr::OcrModelType::Mobile)
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

fn join_error_message(task_name: &str, err: JoinError) -> String {
    if err.is_cancelled() {
        return format!("OCR task '{task_name}' was cancelled");
    }

    if err.is_panic() {
        let panic_payload = err.into_panic();
        let panic_message = if let Some(message) = panic_payload.downcast_ref::<&str>() {
            (*message).to_string()
        } else if let Some(message) = panic_payload.downcast_ref::<String>() {
            message.clone()
        } else {
            "unknown panic payload".to_string()
        };

        return format!("OCR task '{task_name}' panicked: {panic_message}");
    }

    format!("OCR task '{task_name}' failed to join")
}

async fn run_on_core_runtime<T, F>(task_name: &'static str, task: F) -> Result<T, String>
where
    T: Send + 'static,
    F: Future<Output = Result<T, String>> + Send + 'static,
{
    crate::RUNTIME.spawn(task).await.map_err(|err| join_error_message(task_name, err))?
}

pub async fn download_mobile_models(force: bool, on_progress: Option<Arc<dyn Fn(f32) + Send + Sync>>) -> Result<(), String> {
    run_on_core_runtime("download mobile OCR models", async move {
        minnow_ocr::download_models(minnow_ocr::OcrModelType::Mobile, force, on_progress)
            .await
            .map_err(|err| err.to_string())
    })
    .await
}

pub async fn recognize_image_blocks(path: impl AsRef<Path>) -> Result<Vec<super::OcrBlock>, String> {
    let image_path: PathBuf = path.as_ref().to_path_buf();

    run_on_core_runtime("recognize image OCR blocks", async move {
        let image = tokio::task::spawn_blocking(move || image::open(&image_path).map_err(|err| err.to_string()))
            .await
            .map_err(|err| join_error_message("open OCR image", err))??;

        let (img_w, img_h) = (image.width() as f64, image.height() as f64);
        let mut context = minnow_ocr::OcrContext::new(None::<PathBuf>, minnow_ocr::OcrModelType::Mobile, None)
            .await
            .map_err(|err| err.to_string())?;

        let ocr_results = tokio::task::spawn_blocking(move || context.recognize(&image).map_err(|err| err.to_string()))
            .await
            .map_err(|err| join_error_message("run OCR inference", err))??;

        Ok(super::build_ocr_blocks(ocr_results, img_w, img_h))
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::{OcrModelStatus, model_status_from, run_on_core_runtime};

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

    #[test]
    fn runtime_bridge_returns_async_value() {
        let value = crate::RUNTIME
            .block_on(run_on_core_runtime("test value", async move { Ok::<u8, String>(7) }))
            .expect("runtime bridge should return async result");
        assert_eq!(value, 7);
    }

    #[test]
    fn runtime_bridge_returns_inner_error() {
        let err = crate::RUNTIME
            .block_on(run_on_core_runtime::<(), _>("test error", async move { Err("boom".to_string()) }))
            .expect_err("runtime bridge should forward inner errors");
        assert_eq!(err, "boom");
    }

    #[test]
    fn runtime_bridge_converts_panic_to_error() {
        let err = crate::RUNTIME
            .block_on(run_on_core_runtime::<(), _>("panic task", async move {
                panic!("panic-marker");
            }))
            .expect_err("runtime bridge should convert panic to error");

        assert!(err.contains("panic task"));
        assert!(err.contains("panic-marker"));
    }
}
