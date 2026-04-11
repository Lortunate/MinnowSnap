#[cfg(not(feature = "portable"))]
use directories::ProjectDirs;
use std::env;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

#[cfg(not(feature = "portable"))]
const APP_QUALIFIER: &str = "com";
#[cfg(not(feature = "portable"))]
const APP_ORGANIZATION: &str = "lortunate";
#[cfg(not(feature = "portable"))]
const APP_NAME: &str = "MinnowSnap";
const DATA_DIR_NAME: &str = "data";
const CONFIG_FILE_NAME: &str = "config.toml";
const LOGS_DIR_NAME: &str = "logs";
#[cfg(feature = "portable")]
const TEMP_DIR_NAME: &str = "temp";
const OCR_MODELS_DIR_NAME: &str = "ocr_models";

#[derive(Debug)]
pub struct AppPaths {
    data_dir: PathBuf,
    config_file: PathBuf,
    logs_dir: PathBuf,
    temp_dir: PathBuf,
    ocr_models_dir: PathBuf,
}

static APP_PATHS: OnceLock<AppPaths> = OnceLock::new();

#[cfg(feature = "portable")]
fn executable_dir() -> PathBuf {
    env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| env::current_dir().unwrap_or_default())
}

fn resolve_app_paths() -> AppPaths {
    #[cfg(feature = "portable")]
    {
        let data_dir = executable_dir().join(DATA_DIR_NAME);
        AppPaths {
            config_file: data_dir.join(CONFIG_FILE_NAME),
            logs_dir: data_dir.join(LOGS_DIR_NAME),
            temp_dir: data_dir.join(TEMP_DIR_NAME),
            ocr_models_dir: data_dir.join(OCR_MODELS_DIR_NAME),
            data_dir,
        }
    }

    #[cfg(not(feature = "portable"))]
    {
        let current_dir = env::current_dir().unwrap_or_default();
        let project_dirs = ProjectDirs::from(APP_QUALIFIER, APP_ORGANIZATION, APP_NAME);
        let config_file = project_dirs
            .as_ref()
            .map(|dirs| dirs.config_dir().join(CONFIG_FILE_NAME))
            .unwrap_or_else(|| current_dir.join(CONFIG_FILE_NAME));
        let data_dir = project_dirs
            .as_ref()
            .map(|dirs| dirs.data_local_dir().to_path_buf())
            .unwrap_or_else(|| current_dir.join(DATA_DIR_NAME));

        AppPaths {
            logs_dir: data_dir.join(LOGS_DIR_NAME),
            ocr_models_dir: data_dir.join(OCR_MODELS_DIR_NAME),
            data_dir,
            config_file,
            temp_dir: env::temp_dir(),
        }
    }
}

pub fn app_paths() -> &'static AppPaths {
    APP_PATHS.get_or_init(resolve_app_paths)
}

impl AppPaths {
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
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

    pub fn temp_file(&self, file_name: &str) -> PathBuf {
        self.temp_dir.join(file_name)
    }

    pub fn ocr_models_dir(&self) -> &Path {
        &self.ocr_models_dir
    }
}

#[cfg(test)]
mod tests {
    use super::{APP_PATHS, app_paths};

    #[test]
    fn app_paths_is_cached() {
        let first = app_paths();
        let second = app_paths();
        assert!(std::ptr::eq(first, second));
        assert!(APP_PATHS.get().is_some());
    }

    #[test]
    fn resolved_paths_have_expected_layout() {
        let paths = app_paths();

        #[cfg(feature = "portable")]
        {
            assert!(paths.data_dir().ends_with("data"));
            assert!(paths.config_file().ends_with("data/config.toml"));
            assert!(paths.logs_dir().ends_with("data/logs"));
            assert!(paths.temp_dir().ends_with("data/temp"));
            assert_eq!(paths.temp_file("test.lock"), paths.temp_dir().join("test.lock"));
            assert!(paths.ocr_models_dir().ends_with("data/ocr_models"));
        }

        #[cfg(not(feature = "portable"))]
        {
            assert!(paths.config_file().ends_with("config.toml"));
            assert!(paths.logs_dir().ends_with("logs"));
            assert_eq!(paths.temp_dir(), std::env::temp_dir());
            assert_eq!(paths.temp_file("test.lock"), paths.temp_dir().join("test.lock"));
            assert!(paths.ocr_models_dir().ends_with("ocr_models"));
        }
    }
}
