use crate::core::app::APP_NAME;
use crate::core::settings::SETTINGS;
use log::error;

#[cfg(not(target_os = "windows"))]
use notify_rust::Notification;
#[cfg(target_os = "windows")]
use tauri_winrt_notification::{IconCrop, Toast};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationType {
    Save,
    Copy,
    QrCode,
    Info,
}

pub fn play_shutter() {
    let settings = match SETTINGS.lock() {
        Ok(guard) => guard.get(),
        Err(_) => return,
    };

    if !settings.notification.shutter_sound {
        return;
    }

    crate::core::RUNTIME.spawn_blocking(|| {
        let sound_data = include_bytes!("../../resources/raw/capture.mp3");
        let cursor = std::io::Cursor::new(&sound_data[..]);
        match rodio::OutputStreamBuilder::open_default_stream() {
            Ok(stream_handle) => {
                let sink = rodio::Sink::connect_new(stream_handle.mixer());
                match rodio::Decoder::new(cursor) {
                    Ok(source) => {
                        sink.append(source);
                        sink.sleep_until_end();
                    }
                    Err(e) => log::error!("Failed to decode audio stream: {}", e),
                }
            }
            Err(e) => log::error!("Failed to open default audio stream: {}", e),
        }
    });
}

pub fn show(title: &str, message: &str, type_: NotificationType) {
    let settings = match SETTINGS.lock() {
        Ok(guard) => guard.get(),
        Err(_) => return,
    };

    if !settings.notification.enabled {
        return;
    }

    let allowed = match type_ {
        NotificationType::Copy => settings.notification.copy_notification,
        NotificationType::Save => settings.notification.save_notification,
        NotificationType::QrCode => settings.notification.qr_code_notification,
        _ => true,
    };
    if !allowed {
        return;
    }

    #[cfg(target_os = "windows")]
    {
        let toast = if let Some(icon_path) = crate::core::app::windows_notification_icon_path() {
            Toast::new(crate::core::app::APP_ID)
                .icon(icon_path.as_path(), IconCrop::Circular, APP_NAME)
                .title(title)
                .text1(message)
        } else {
            Toast::new(crate::core::app::APP_ID).title(title).text1(message)
        };

        if let Err(e) = toast.show() {
            error!("Failed to send notification: {}", e);
        }
    }

    #[cfg(not(target_os = "windows"))]
    if let Err(e) = Notification::new().summary(title).body(message).appname(APP_NAME).show() {
        error!("Failed to send notification: {}", e);
    }
}
