//! Model sources and the contracts shared by download and generation storage.
mod download;
mod store;

pub(crate) use download::HttpFetcher;
pub(crate) use store::ModelStore;

pub use crate::sources::ModelSources;
use minnow_application::ports::{ErrorKind, OperationError, OperationFuture, OperationResult};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

pub(crate) type Progress = Arc<dyn Fn(f32) + Send + Sync>;

#[derive(Clone, Debug)]
pub(crate) struct ModelPaths {
    pub(crate) detector: PathBuf,
    pub(crate) recognizer: PathBuf,
    pub(crate) keys: PathBuf,
    pub(crate) spec: Arc<ModelSources>,
}

#[derive(Clone, Debug)]
pub(crate) struct PreparedModels {
    pub(crate) generation: String,
    pub(crate) paths: ModelPaths,
}

pub(crate) trait ArtifactValidator: Send + Sync {
    fn validate(&self, paths: &ModelPaths) -> OperationResult<()>;
}

pub(crate) trait ArtifactFetcher: Send + Sync {
    fn fetch<'a>(&'a self, url: &'a str, destination: &'a Path, cancel: CancellationToken, progress: Progress) -> OperationFuture<'a, ()>;
}

pub(crate) fn check_cancelled(token: &CancellationToken) -> OperationResult<()> {
    if token.is_cancelled() {
        Err(OperationError::cancelled())
    } else {
        Ok(())
    }
}

pub(crate) fn io_error(error: impl std::fmt::Display) -> OperationError {
    OperationError::new(ErrorKind::Io, error.to_string())
}

pub(crate) fn task_error(error: impl std::fmt::Display) -> OperationError {
    OperationError::new(ErrorKind::Task, error.to_string())
}
