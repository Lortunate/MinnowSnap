//! Shared notification presentation; application services own delivery.
use gpui::App;
pub(crate) use minnow_application::ports::NotificationKind as NotificationType;

pub(crate) fn show_notification(title: &str, message: &str, notification_type: NotificationType, cx: &mut App) {
    let services = crate::application(cx);
    let request = minnow_application::ports::NotificationRequest {
        title: title.to_string(),
        message: message.to_string(),
        kind: notification_type,
    };
    cx.spawn(async move |_| {
        if let Err(error) = services.notify(request).await {
            tracing::warn!("Notification failed: {error}");
        }
    })
    .detach();
}
