use super::{MutationResult, state::PreferencesState};
use minnow_core::{i18n, ocr::service, ocr::service::OcrModelStatus};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct OcrSnapshot {
    pub(crate) enabled: bool,
    pub(crate) status: OcrModelStatus,
}

pub(crate) fn set_enabled(enabled: bool) -> MutationResult {
    service::set_enabled(enabled);
    MutationResult::refresh_windows()
}

pub(crate) fn snapshot(state: &PreferencesState) -> OcrSnapshot {
    OcrSnapshot {
        enabled: service::is_enabled(),
        status: service::current_status(
            state.ocr_download.in_progress,
            state.ocr_download.progress_percent,
            state.ocr_download.last_error.clone(),
        ),
    }
}

pub(crate) fn ocr_status_label(status: &OcrModelStatus) -> String {
    match status {
        OcrModelStatus::Missing => i18n::preferences::ocr_status_missing(),
        OcrModelStatus::Downloading { progress_percent } => i18n::preferences::ocr_status_downloading(*progress_percent),
        OcrModelStatus::Ready => i18n::preferences::ocr_status_ready(),
        OcrModelStatus::Failed { message } => i18n::preferences::ocr_status_failed(message.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::preferences::state::state::PreferencesNotice;

    #[test]
    fn ocr_status_label_maps_failed_status() {
        assert_eq!(
            ocr_status_label(&OcrModelStatus::Failed {
                message: "network error".to_string(),
            }),
            i18n::preferences::ocr_status_failed("network error".to_string())
        );
    }

    #[test]
    fn finishing_failed_download_sets_error_notice() {
        let mut state = PreferencesState::new();
        state.start_ocr_download();

        assert!(state.update_ocr_download_progress(42));
        state.finish_ocr_download(Err("network error".to_string()));

        assert!(!state.ocr_download.in_progress);
        assert_eq!(state.ocr_download.progress_percent, 0);
        assert_eq!(state.ocr_download.last_error.as_deref(), Some("network error"));
        assert_eq!(
            ocr_status_label(&OcrModelStatus::Failed {
                message: "network error".to_string(),
            }),
            i18n::preferences::ocr_status_failed("network error".to_string())
        );
        assert!(state.notice.as_ref().is_some_and(PreferencesNotice::is_error));
    }
}
