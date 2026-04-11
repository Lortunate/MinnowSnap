use crate::app_meta::APP_LOCK_ID;
use std::io;
use std::path::{Path, PathBuf};

pub fn lock_file() -> PathBuf {
    minnow_paths::app_paths().temp_file(APP_LOCK_ID)
}

pub fn ensure_dir(path: &Path) -> io::Result<()> {
    std::fs::create_dir_all(path)
}

pub fn ensure_parent_dir(path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        ensure_dir(parent)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{ensure_dir, ensure_parent_dir, lock_file};
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_test_path(name: &str) -> PathBuf {
        let suffix = SystemTime::now().duration_since(UNIX_EPOCH).expect("system time").as_nanos();
        std::env::temp_dir().join(format!("minnowsnap-paths-test-{name}-{suffix}"))
    }

    #[test]
    fn lock_file_uses_temp_directory() {
        assert_eq!(lock_file(), minnow_paths::app_paths().temp_file(super::APP_LOCK_ID));
    }

    #[test]
    fn ensure_dir_creates_missing_directory() {
        let path = unique_test_path("dir");
        ensure_dir(&path).expect("create dir");
        assert!(path.is_dir());
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn ensure_parent_dir_creates_missing_parent_directory() {
        let path = unique_test_path("parent").join("nested").join("config.toml");
        ensure_parent_dir(&path).expect("create parent dir");
        assert!(path.parent().expect("parent").is_dir());
        let _ = std::fs::remove_dir_all(path.parent().and_then(|p| p.parent()).expect("root"));
    }
}
