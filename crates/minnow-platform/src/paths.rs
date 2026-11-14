use minnow_application::ports::{ErrorKind, OperationError, OperationResult};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataLayout {
    Installed,
    Portable { executable: PathBuf },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppPaths {
    config_file: PathBuf,
    logs_dir: PathBuf,
    temp_dir: PathBuf,
    ocr_models_dir: PathBuf,
}

impl AppPaths {
    pub fn resolve(qualifier: &str, organization: &str, application_name: &str, layout: DataLayout) -> OperationResult<Self> {
        match layout {
            DataLayout::Installed => {
                let dirs = directories::ProjectDirs::from(qualifier, organization, application_name)
                    .ok_or_else(|| OperationError::new(ErrorKind::Unavailable, "the application's data directories are unavailable"))?;
                Ok(Self {
                    config_file: dirs.config_dir().join("config.toml"),
                    logs_dir: dirs.data_local_dir().join("logs"),
                    temp_dir: std::env::temp_dir(),
                    ocr_models_dir: dirs.data_local_dir().join("ocr_models"),
                })
            }
            DataLayout::Portable { executable } => {
                let parent = executable
                    .parent()
                    .filter(|parent| !parent.as_os_str().is_empty())
                    .ok_or_else(|| OperationError::new(ErrorKind::InvalidInput, "portable mode requires an executable with a parent directory"))?;
                Ok(Self::from_portable_directory(parent))
            }
        }
    }

    fn from_portable_directory(directory: &Path) -> Self {
        let data = directory.join("data");
        Self {
            config_file: data.join("config.toml"),
            logs_dir: data.join("logs"),
            temp_dir: data.join("temp"),
            ocr_models_dir: data.join("ocr_models"),
        }
    }

    pub fn config_file(&self) -> &Path {
        &self.config_file
    }

    pub fn logs_dir(&self) -> &Path {
        &self.logs_dir
    }

    pub fn temp_dir(&self) -> &Path {
        &self.temp_dir
    }

    pub fn ocr_models_dir(&self) -> &Path {
        &self.ocr_models_dir
    }

    pub fn temp_file(&self, name: impl AsRef<Path>) -> PathBuf {
        self.temp_dir.join(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portable_paths_preserve_the_existing_layout_and_are_instance_owned() {
        let one = AppPaths::resolve(
            "com",
            "lortunate",
            "MinnowSnap",
            DataLayout::Portable {
                executable: PathBuf::from("/one/MinnowSnap"),
            },
        )
        .unwrap();
        let two = AppPaths::resolve(
            "com",
            "lortunate",
            "MinnowSnap",
            DataLayout::Portable {
                executable: PathBuf::from("/two/MinnowSnap"),
            },
        )
        .unwrap();
        assert_eq!(one.config_file(), Path::new("/one/data/config.toml"));
        assert_eq!(one.logs_dir(), Path::new("/one/data/logs"));
        assert_eq!(one.temp_dir(), Path::new("/one/data/temp"));
        assert_eq!(one.ocr_models_dir(), Path::new("/one/data/ocr_models"));
        assert_eq!(two.config_file(), Path::new("/two/data/config.toml"));
        assert_ne!(one, two);
    }
}
