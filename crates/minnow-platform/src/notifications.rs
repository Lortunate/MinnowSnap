use minnow_application::ports::{ErrorKind, NotificationRequest, Notifications, OperationError, OperationResult};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct DesktopNotifications {
    app_id: String,
    app_name: String,
    icon_file: Option<PathBuf>,
}

impl DesktopNotifications {
    /// Pure construction. Call `register` at the host's platform-safe startup
    /// phase, after GPUI and components have initialized on macOS.
    pub fn new(app_id: impl Into<String>, app_name: impl Into<String>, icon_file: Option<PathBuf>) -> Self {
        Self {
            app_id: app_id.into(),
            app_name: app_name.into(),
            icon_file,
        }
    }

    pub fn icon_file(&self) -> Option<&Path> {
        self.icon_file.as_deref()
    }

    /// macOS notify-rust installs an NSBundle identifier hook even on failure.
    /// Keeping registration explicit prevents a constructor from invalidating
    /// another framework's unbundled-process detection.
    pub fn register(&self, icon_bytes: Option<&[u8]>) -> OperationResult<()> {
        if self.app_id.is_empty() || self.app_name.is_empty() {
            return Err(OperationError::new(ErrorKind::InvalidInput, "notification application identity is empty"));
        }
        #[cfg(target_os = "windows")]
        {
            self.register_windows(icon_bytes)
        }
        #[cfg(target_os = "macos")]
        {
            let _ = icon_bytes;
            notify_rust::set_application(&self.app_id)
                .map_err(|error| OperationError::new(ErrorKind::Native, format!("failed to register notification identity: {error}")))
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        {
            let _ = icon_bytes;
            Ok(())
        }
    }

    #[cfg(target_os = "windows")]
    fn register_windows(&self, icon_bytes: Option<&[u8]>) -> OperationResult<()> {
        use std::io::Write;
        use winreg::{RegKey, enums::HKEY_CURRENT_USER};

        let (key, _) = RegKey::predef(HKEY_CURRENT_USER)
            .create_subkey(format!(r"Software\Classes\AppUserModelId\{}", self.app_id))
            .map_err(|error| native_error("create notification application key", error))?;
        key.set_value("DisplayName", &self.app_name)
            .map_err(|error| native_error("set notification display name", error))?;
        key.set_value("IconBackgroundColor", &"0")
            .map_err(|error| native_error("set notification icon background", error))?;

        if let Some(path) = &self.icon_file {
            if let Some(bytes) = icon_bytes {
                let parent = path
                    .parent()
                    .ok_or_else(|| OperationError::new(ErrorKind::InvalidInput, "notification icon has no parent directory"))?;
                std::fs::create_dir_all(parent).map_err(|error| native_error("create notification icon directory", error))?;
                let mut staging = tempfile::NamedTempFile::new_in(parent).map_err(|error| native_error("stage notification icon", error))?;
                staging.write_all(bytes).map_err(|error| native_error("write notification icon", error))?;
                staging.flush().map_err(|error| native_error("flush notification icon", error))?;
                staging
                    .persist(path)
                    .map_err(|error| native_error("commit notification icon", error.error))?;
            }
            key.set_value("IconUri", &path.to_string_lossy().as_ref())
                .map_err(|error| native_error("set notification icon path", error))?;
        }
        Ok(())
    }
}

impl Notifications for DesktopNotifications {
    fn show(&self, request: &NotificationRequest) -> OperationResult<()> {
        #[cfg(target_os = "windows")]
        {
            tauri_winrt_notification::Toast::new(&self.app_id)
                .title(&request.title)
                .text1(&request.message)
                .show()
                .map_err(|error| native_error("show notification", error))
        }
        #[cfg(not(target_os = "windows"))]
        {
            notify_rust::Notification::new()
                .summary(&request.title)
                .body(&request.message)
                .appname(&self.app_name)
                .show()
                .map(|_| ())
                .map_err(|error| native_error("show notification", error))
        }
    }
}

fn native_error(operation: &str, error: impl std::fmt::Display) -> OperationError {
    OperationError::new(ErrorKind::Native, format!("failed to {operation}: {error}"))
}
