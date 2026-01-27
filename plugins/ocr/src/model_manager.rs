use crate::config::{APP_DATA_DIR, MODEL_DIR};
use anyhow::{Context, Result};
use futures_util::StreamExt;
use log::info;
use std::path::{Path, PathBuf};
use tokio::fs::{self, File};
use tokio::io::AsyncWriteExt;

pub struct ModelManager {
    save_dir: PathBuf,
}

impl ModelManager {
    pub fn new<P: AsRef<Path>>(save_dir: P) -> Self {
        Self {
            save_dir: save_dir.as_ref().to_path_buf(),
        }
    }

    pub fn default_dir() -> Result<PathBuf> {
        let mut path = dirs::data_local_dir().context("Could not find data local dir")?;
        path.push(APP_DATA_DIR);
        path.push(MODEL_DIR);
        Ok(path)
    }

    pub fn check_models_existence(&self, filenames: &[&str]) -> bool {
        filenames.iter().all(|name| self.save_dir.join(name).exists())
    }

    pub async fn ensure_model(&self, url: &str, filename: &str, force: bool, on_progress: Option<Box<dyn Fn(f32) + Send + Sync>>) -> Result<PathBuf> {
        let file_path = self.save_dir.join(filename);

        if !force && file_path.exists() {
            info!("Model {} already exists at {:?}", filename, file_path);
            if let Some(cb) = on_progress {
                cb(1.0);
            }
            return Ok(file_path);
        }

        if !self.save_dir.exists() {
            fs::create_dir_all(&self.save_dir).await?;
        }

        info!("Downloading model from {} to {:?}", url, file_path);
        self.download_file(url, &file_path, on_progress).await?;

        Ok(file_path)
    }

    async fn download_file(&self, url: &str, path: &PathBuf, on_progress: Option<Box<dyn Fn(f32) + Send + Sync>>) -> Result<()> {
        let client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .build()?;
        let response = client.get(url).send().await?.error_for_status()?;
        let total_size = response.content_length().unwrap_or(0);

        let mut file = File::create(path).await?;
        let mut stream = response.bytes_stream();
        let mut downloaded: u64 = 0;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            file.write_all(&chunk).await?;
            downloaded += chunk.len() as u64;
            if let Some(ref cb) = on_progress
                && total_size > 0
            {
                cb(downloaded as f32 / total_size as f32);
            }
        }

        file.flush().await?;
        Ok(())
    }
}
