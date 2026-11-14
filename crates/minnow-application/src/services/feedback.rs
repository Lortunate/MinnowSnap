use super::{ApplicationServices, blocking};
use crate::ports::NotificationRequest;
use crate::{CompletionPolicy, JobAdmission, JobHandle, JobResource, JobScope};

impl ApplicationServices {
    /// Settings and worker ownership are application policy, including for sound.
    pub fn play_shutter(&self) -> JobHandle<()> {
        let sound = self.shutter_sound.clone();
        let enabled = self.settings_snapshot().notification.shutter_sound;
        self.jobs.spawn_blocking(
            "shutter sound",
            JobScope::Application,
            CompletionPolicy::PreserveResult,
            JobResource::Feedback,
            move |_| {
                if enabled { sound.play() } else { Ok(()) }
            },
        )
    }

    pub fn copy_text(&self, text: String) -> JobHandle<()> {
        let output = self.output.clone();
        let runtime = self.runtime.clone();
        self.jobs.spawn(
            "copy text",
            JobScope::Application,
            CompletionPolicy::PreserveResult,
            JobAdmission::Feedback,
            move |_| async move { blocking(&runtime, move || output.copy_text(&text)).await },
        )
    }

    pub fn notify(&self, request: NotificationRequest) -> JobHandle<()> {
        let notifications = self.notifications.clone();
        let runtime = self.runtime.clone();
        let settings = self.settings_snapshot().notification;
        self.jobs.spawn(
            "show notification",
            JobScope::Application,
            CompletionPolicy::PreserveResult,
            JobAdmission::Feedback,
            move |_| async move {
                use crate::ports::NotificationKind;
                let enabled = settings.enabled
                    && match request.kind {
                        NotificationKind::Save => settings.save_notification,
                        NotificationKind::Copy => settings.copy_notification,
                        NotificationKind::QrCode => settings.qr_code_notification,
                        NotificationKind::Info => true,
                    };
                if enabled {
                    blocking(&runtime, move || notifications.show(&request)).await?;
                }
                Ok(())
            },
        )
    }
}
