//! HTTP transfer, cancellation, and per-artifact size checks.
use super::{ArtifactFetcher, Progress, check_cancelled, io_error};
use futures_util::StreamExt;
use minnow_application::ports::{ErrorKind, OperationError, OperationFuture, OperationResult};
use std::path::Path;
use tokio::io::AsyncWriteExt;
use tokio_util::sync::CancellationToken;

pub(crate) struct HttpFetcher {
    client: reqwest::Client,
    max_bytes: u64,
}

impl HttpFetcher {
    pub(crate) fn new(max_bytes: u64) -> OperationResult<Self> {
        let client = reqwest::Client::builder()
            .user_agent("MinnowSnap/0.1 local-recognition")
            .connect_timeout(std::time::Duration::from_secs(20))
            .timeout(std::time::Duration::from_secs(20 * 60))
            .build()
            .map_err(|error| OperationError::new(ErrorKind::Unavailable, error.to_string()))?;
        Ok(Self { client, max_bytes })
    }
}

impl ArtifactFetcher for HttpFetcher {
    fn fetch<'a>(&'a self, url: &'a str, destination: &'a Path, cancel: CancellationToken, progress: Progress) -> OperationFuture<'a, ()> {
        Box::pin(async move {
            let response = tokio::select! {
                _ = cancel.cancelled() => return Err(OperationError::cancelled()),
                response = self.client.get(url).send() => response
                    .and_then(reqwest::Response::error_for_status)
                    .map_err(|error| OperationError::new(ErrorKind::Download, error.to_string()))?,
            };
            let expected = response.content_length();
            if expected.is_some_and(|size| size == 0 || size > self.max_bytes) {
                return Err(OperationError::new(
                    ErrorKind::Validation,
                    "model response length is empty or exceeds the artifact budget",
                ));
            }
            let mut file = tokio::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(destination)
                .await
                .map_err(io_error)?;
            let mut stream = response.bytes_stream();
            let mut downloaded = 0u64;
            loop {
                let chunk = tokio::select! {
                    _ = cancel.cancelled() => return Err(OperationError::cancelled()),
                    chunk = stream.next() => chunk,
                };
                let Some(chunk) = chunk else { break };
                let chunk = chunk.map_err(|error| OperationError::new(ErrorKind::Download, error.to_string()))?;
                downloaded = downloaded
                    .checked_add(chunk.len() as u64)
                    .filter(|count| *count <= self.max_bytes)
                    .ok_or_else(|| OperationError::new(ErrorKind::Validation, "model artifact exceeds the byte budget"))?;
                file.write_all(&chunk).await.map_err(io_error)?;
                if let Some(total) = expected {
                    progress(downloaded as f32 / total as f32);
                }
            }
            if downloaded == 0 || expected.is_some_and(|total| total != downloaded) {
                return Err(OperationError::new(
                    ErrorKind::Validation,
                    "model download did not contain a complete artifact",
                ));
            }
            file.flush().await.map_err(io_error)?;
            file.sync_all().await.map_err(io_error)?;
            check_cancelled(&cancel)?;
            progress(1.0);
            Ok(())
        })
    }
}
