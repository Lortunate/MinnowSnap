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
                if path.extension().and_then(|s| s.to_str()) == Some("qml") {
                    let file_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
                    if file_name != "AppTheme.qml" {
                        files.push(path.to_string_lossy().into_owned());
                    }
                }
            }
        }
    }
    files
}
