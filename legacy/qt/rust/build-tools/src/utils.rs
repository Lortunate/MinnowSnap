use std::path::Path;
use walkdir::WalkDir;

pub fn collect_qml_files(dir: &Path) -> Vec<String> {
    let mut files = Vec::new();
    if dir.exists() {
        for entry in WalkDir::new(dir).sort_by_file_name() {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };

            if entry.file_type().is_file() {
                let path = entry.path();
                if path.extension() == Some("qml".as_ref()) && path.file_name() != Some("AppTheme.qml".as_ref()) {
                    files.push(path.to_string_lossy().into_owned());
                }
            }
        }
    }
    files
}

pub fn collect_bridge_files(dir: &Path) -> Vec<String> {
    let mut files = Vec::new();
    if dir.exists() {
        for entry in WalkDir::new(dir).sort_by_file_name() {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };

            if entry.file_type().is_file() {
                let path = entry.path();
                let ext = path.extension().and_then(|s| s.to_str());
                if ext == Some("rs")
                    && let Ok(content) = std::fs::read_to_string(path)
                    && (content.contains("#[cxx_qt::bridge]") || content.contains("#[cxx::bridge]"))
                {
                    files.push(path.to_string_lossy().replace('\\', "/"));
                }
            }
        }
    }
    files
}
