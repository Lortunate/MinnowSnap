use super::{MutationResult, store};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NotificationsSnapshot {
    pub(crate) enabled: bool,
    pub(crate) save_notification: bool,
    pub(crate) copy_notification: bool,
    pub(crate) qr_code_notification: bool,
    pub(crate) shutter_sound: bool,
}

pub(crate) fn snapshot() -> NotificationsSnapshot {
    let settings = store::snapshot();

    NotificationsSnapshot {
        enabled: settings.notification.enabled,
        save_notification: settings.notification.save_notification,
        copy_notification: settings.notification.copy_notification,
        qr_code_notification: settings.notification.qr_code_notification,
        shutter_sound: settings.notification.shutter_sound,
    }
}

pub(crate) fn set_enabled(enabled: bool) -> MutationResult {
    store::with_settings(|settings| settings.set_notification_enabled(enabled));
    MutationResult::refresh_windows()
}

pub(crate) fn set_save_notification(enabled: bool) -> MutationResult {
    store::with_settings(|settings| settings.set_save_notification(enabled));
    MutationResult::refresh_windows()
}

pub(crate) fn set_copy_notification(enabled: bool) -> MutationResult {
    store::with_settings(|settings| settings.set_copy_notification(enabled));
    MutationResult::refresh_windows()
}

pub(crate) fn set_qr_code_notification(enabled: bool) -> MutationResult {
    store::with_settings(|settings| settings.set_qr_code_notification(enabled));
    MutationResult::refresh_windows()
}

pub(crate) fn set_shutter_sound(enabled: bool) -> MutationResult {
    store::with_settings(|settings| settings.set_shutter_sound(enabled));
    MutationResult::refresh_windows()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_reads_notification_flags() {
        let snapshot = snapshot();
        let settings = store::snapshot();

        assert_eq!(snapshot.enabled, settings.notification.enabled);
        assert_eq!(snapshot.save_notification, settings.notification.save_notification);
        assert_eq!(snapshot.copy_notification, settings.notification.copy_notification);
        assert_eq!(snapshot.qr_code_notification, settings.notification.qr_code_notification);
        assert_eq!(snapshot.shutter_sound, settings.notification.shutter_sound);
    }
}
