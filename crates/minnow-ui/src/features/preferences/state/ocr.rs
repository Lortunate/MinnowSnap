use crate::presentation::i18n;
use minnow_application::ports::{ModelDownloadStatus, ModelStatus};

pub(crate) fn ocr_status_label(status: &ModelStatus) -> String {
    let model = if status.is_ready() {
        i18n::preferences::ocr_status_ready()
    } else {
        i18n::preferences::ocr_status_missing()
    };
    match &status.download {
        ModelDownloadStatus::Idle => model,
        ModelDownloadStatus::Downloading { progress_percent } => {
            i18n::preferences::ocr_model_and_download(model, i18n::preferences::ocr_status_downloading(*progress_percent))
        }
        ModelDownloadStatus::Failed { message } => i18n::preferences::ocr_model_and_download(model, i18n::preferences::ocr_status_failed(message)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ocr_status_label_keeps_ready_models_visible_after_an_update_fails() {
        let label = ocr_status_label(&ModelStatus {
            active_generation: Some("previous".into()),
            download: ModelDownloadStatus::Failed {
                message: "network error".into(),
            },
        });
        assert!(label.contains(&i18n::preferences::ocr_status_ready()));
        assert!(label.contains(&i18n::preferences::ocr_status_failed("network error")));
    }
}
