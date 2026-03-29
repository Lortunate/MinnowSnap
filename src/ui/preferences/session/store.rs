use crate::core::settings::{AppSettings, SETTINGS, SettingsManager};

pub(crate) fn snapshot() -> AppSettings {
    SETTINGS.lock().map(|guard| guard.get()).unwrap_or_default()
}

pub(crate) fn with_settings<R>(update: impl FnOnce(&mut SettingsManager) -> R) -> R {
    let mut settings = SETTINGS.lock().unwrap();
    update(&mut settings)
}
