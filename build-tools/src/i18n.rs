use anyhow::{Result, anyhow};
use std::path::Path;
use std::process::Command;

/// Run lupdate to extract strings from QML files and update the .ts file
pub fn update_translations(qml_dir: &Path, ts_path: &Path) -> Result<()> {
    println!("Updating translations from {} to {}", qml_dir.display(), ts_path.display());

    if let Some(parent) = ts_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let status = Command::new("lupdate")
        .arg(qml_dir)
        .arg("-ts")
        .arg(ts_path)
        .status()
        .map_err(|e| anyhow!("Failed to execute lupdate: {}", e))?;

    if !status.success() {
        return Err(anyhow!("lupdate failed with status: {}", status));
    }

    Ok(())
}

/// Run lrelease to compile the .ts file into a .qm file
pub fn compile_translations(ts_path: &Path, qm_path: &Path) -> Result<()> {
    println!("Compiling translations from {} to {}", ts_path.display(), qm_path.display());

    if let Some(parent) = qm_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let status = Command::new("lrelease")
        .arg(ts_path)
        .arg("-qm")
        .arg(qm_path)
        .status()
        .map_err(|e| anyhow!("Failed to execute lrelease: {}", e))?;

    if !status.success() {
        return Err(anyhow!("lrelease failed with status: {}", status));
    }

    Ok(())
}

/// Compile all .ts files in a directory into .qm files
pub fn compile_all_translations(i18n_dir: &Path) -> Result<()> {
    if !i18n_dir.exists() {
        return Ok(());
    }

    for entry in std::fs::read_dir(i18n_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "ts") {
            let qm_path = path.with_extension("qm");
            compile_translations(&path, &qm_path)?;
        }
    }
    Ok(())
}
