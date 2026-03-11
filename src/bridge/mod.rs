pub mod annotation;
pub mod app;
pub mod capture_compositor;
pub mod capture_session;
pub mod config;
pub mod hotkey;
pub mod long_capture;
pub mod ocr;
pub mod ocr_overlay;
pub mod overlay_controller;
pub mod pin;
pub mod provider;
pub mod screen_capture;
pub mod shortcut_helper;
pub mod tray_menu;
pub mod window;

#[macro_export]
macro_rules! spawn_qt_task {
    ($qobject:expr, $task:expr, $callback:expr) => {{
        let qt_thread = $qobject.qt_thread();
        $crate::core::RUNTIME.spawn(async move {
            let result = $task.await;
            let _ = qt_thread.queue(move |qobject| {
                $callback(qobject, result);
            });
        });
    }};
}

#[macro_export]
macro_rules! notify_tr {
    ($context:expr, $title_key:expr, $msg_key:expr, $type:ident) => {{
        let title = $crate::bridge::app::tr($context, $title_key);
        let msg = $crate::bridge::app::tr($context, $msg_key);
        $crate::core::notify::show(&title.to_string(), &msg.to_string(), $crate::core::notify::NotificationType::$type);
    }};
}

#[macro_export]
macro_rules! spawn_clipboard_copy {
    ($self:expr, $text:expr, $msg_key:expr, $notif_type:ident) => {
        let text_str = $text.to_string();
        $crate::spawn_qt_task!(
            $self,
            async move {
                tokio::task::spawn_blocking(move || $crate::core::io::clipboard::copy_text_to_clipboard(text_str))
                    .await
                    .unwrap_or(false)
            },
            |_qobject: Pin<&mut qobject::ScreenCapture>, success| {
                if success {
                    $crate::notify_tr!("ScreenCapture", "Success", $msg_key, $notif_type);
                } else {
                    tracing::error!("Failed to copy to clipboard");
                }
            }
        );
    };
}
