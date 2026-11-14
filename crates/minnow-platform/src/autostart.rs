use auto_launch::{AutoLaunch, AutoLaunchBuilder, MacOSLaunchMode};
use minnow_application::ports::{ErrorKind, OperationError, OperationResult};
use std::path::Path;

pub struct AutoStartRegistration {
    backend: AutoLaunch,
}

impl AutoStartRegistration {
    pub fn new(application_name: &str, executable: &Path) -> OperationResult<Self> {
        let path = executable
            .to_str()
            .ok_or_else(|| OperationError::new(ErrorKind::InvalidInput, "auto-start requires an executable path representable as UTF-8"))?;
        let backend = AutoLaunchBuilder::new()
            .set_app_name(application_name)
            .set_app_path(path)
            .set_macos_launch_mode(MacOSLaunchMode::LaunchAgent)
            .build()
            .map_err(|error| native_error("prepare auto-start registration", error))?;
        Ok(Self { backend })
    }

    pub fn is_enabled(&self) -> OperationResult<bool> {
        self.backend
            .is_enabled()
            .map_err(|error| native_error("query auto-start registration", error))
    }

    pub fn set_enabled(&self, enabled: bool) -> OperationResult<()> {
        let result = if enabled { self.backend.enable() } else { self.backend.disable() };
        result.map_err(|error| native_error("update auto-start registration", error))
    }
}

fn native_error(operation: &str, error: impl std::fmt::Display) -> OperationError {
    OperationError::new(ErrorKind::Native, format!("failed to {operation}: {error}"))
}
