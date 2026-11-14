use crate::actions::{ImageContentIdentity, ImageSource};
use crate::{AnnotationSnapshot, ErrorKind, ImageInput, ImageProjection, OperationError, OperationResult};
use minnow_imaging::{CapturedFrame, ImageRef};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

static SESSION_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SessionId(u64);

impl SessionId {
    pub fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JobScope {
    Application,
    Session { id: SessionId, revision: u64 },
}

#[derive(Clone, Debug)]
pub struct SessionSnapshot {
    id: SessionId,
    revision: u64,
    image: ImageRef,
}

impl SessionSnapshot {
    pub fn id(&self) -> SessionId {
        self.id
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn image(&self) -> &ImageRef {
        &self.image
    }
    pub fn scope(&self) -> JobScope {
        JobScope::Session {
            id: self.id,
            revision: self.revision,
        }
    }
}

#[derive(Debug)]
struct SessionState {
    source: Option<ImageSource>,
    revision: u64,
    publication: Option<ImageContentIdentity>,
    closed: bool,
}

/// An image's lifetime is independent of any particular export attempt.
#[derive(Clone, Debug)]
pub struct SessionHandle {
    id: SessionId,
    state: Arc<Mutex<SessionState>>,
}

impl SessionHandle {
    pub fn id(&self) -> SessionId {
        self.id
    }

    pub fn snapshot(&self) -> Option<SessionSnapshot> {
        let state = lock(&self.state);
        if state.closed {
            return None;
        }
        Some(SessionSnapshot {
            id: self.id,
            revision: state.revision,
            image: state.source.as_ref()?.image().clone(),
        })
    }

    pub fn scope(&self) -> Option<JobScope> {
        self.snapshot().map(|snapshot| JobScope::Session {
            id: self.id,
            revision: snapshot.revision,
        })
    }

    fn publish_revision(&self, revision: u64) -> OperationResult<()> {
        let mut state = lock(&self.state);
        Self::validate_revision(&state, revision)?;
        if revision > state.revision {
            state.publication = None;
        }
        state.revision = revision;
        Ok(())
    }

    fn publish_output(&self, revision: u64, projection: ImageProjection, annotations: Option<AnnotationSnapshot>) -> OperationResult<ImageInput> {
        let mut state = lock(&self.state);
        Self::validate_revision(&state, revision)?;
        let source = state
            .source
            .as_ref()
            .ok_or_else(|| OperationError::new(ErrorKind::Conflict, "document session is closed"))?;
        // Geometry and renderer requirements are validated before the revision
        // changes. The returned input cannot replace this registered source.
        let input = ImageInput::from_source(source, projection, annotations, JobScope::Session { id: self.id, revision })?;
        let publication = input.content_identity();
        if revision == state.revision && state.publication.is_some_and(|current| current != publication) {
            return Err(OperationError::new(
                ErrorKind::Conflict,
                "the output revision already identifies different document content",
            ));
        }
        state.revision = revision;
        state.publication = Some(publication);
        Ok(input)
    }

    fn validate_revision(state: &SessionState, revision: u64) -> OperationResult<()> {
        if state.closed {
            return Err(OperationError::new(ErrorKind::Conflict, "document session is closed"));
        }
        if revision < state.revision {
            return Err(OperationError::new(
                ErrorKind::Conflict,
                "document publication belongs to an obsolete output revision",
            ));
        }
        Ok(())
    }

    pub fn is_closed(&self) -> bool {
        lock(&self.state).closed
    }
}

struct RegistryInner {
    sessions: Mutex<HashMap<SessionId, SessionHandle>>,
}

#[derive(Clone)]
pub(crate) struct SessionRegistry(Arc<RegistryInner>);

impl Default for SessionRegistry {
    fn default() -> Self {
        Self(Arc::new(RegistryInner {
            sessions: Mutex::new(HashMap::new()),
        }))
    }
}

impl SessionRegistry {
    pub fn create(&self, image: ImageRef, revision: u64) -> SessionHandle {
        self.create_source(ImageSource::Image(image), revision)
    }

    pub(crate) fn create_capture(&self, frame: CapturedFrame, revision: u64) -> SessionHandle {
        self.create_source(ImageSource::Capture(frame), revision)
    }

    fn create_source(&self, source: ImageSource, revision: u64) -> SessionHandle {
        let session = SessionHandle {
            id: SessionId(SESSION_SEQUENCE.fetch_add(1, Ordering::Relaxed)),
            state: Arc::new(Mutex::new(SessionState {
                source: Some(source),
                revision,
                publication: None,
                closed: false,
            })),
        };
        lock(&self.0.sessions).insert(session.id, session.clone());
        session
    }

    pub fn get(&self, id: SessionId) -> Option<SessionHandle> {
        lock(&self.0.sessions).get(&id).cloned()
    }

    pub(crate) fn publish_revision(&self, id: SessionId, revision: u64) -> OperationResult<()> {
        self.get(id)
            .ok_or_else(|| OperationError::new(ErrorKind::Conflict, "document session is closed"))?
            .publish_revision(revision)
    }

    pub(crate) fn publish_output(
        &self,
        id: SessionId,
        revision: u64,
        projection: ImageProjection,
        annotations: Option<AnnotationSnapshot>,
    ) -> OperationResult<ImageInput> {
        self.get(id)
            .ok_or_else(|| OperationError::new(ErrorKind::Conflict, "document session is closed"))?
            .publish_output(revision, projection, annotations)
    }

    pub fn accepts(&self, scope: JobScope) -> bool {
        match scope {
            JobScope::Application => true,
            JobScope::Session { id, revision } => self
                .get(id)
                .and_then(|session| session.snapshot())
                .is_some_and(|snapshot| snapshot.revision == revision),
        }
    }

    pub fn close(&self, id: SessionId) -> bool {
        let Some(session) = lock(&self.0.sessions).remove(&id) else {
            return false;
        };
        let mut state = lock(&session.state);
        state.closed = true;
        state.source = None;
        true
    }

    pub fn close_all(&self) -> Vec<SessionId> {
        let sessions = std::mem::take(&mut *lock(&self.0.sessions));
        let ids = sessions.keys().copied().collect();
        for session in sessions.into_values() {
            let mut state = lock(&session.state);
            state.closed = true;
            state.source = None;
        }
        ids
    }
}

pub(crate) fn lock<T>(value: &Mutex<T>) -> MutexGuard<'_, T> {
    value.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::RgbaImage;
    use minnow_imaging::ImageAsset;

    #[test]
    fn failed_attempts_do_not_consume_the_session_image() {
        let registry = SessionRegistry::default();
        let image = ImageAsset::new(RgbaImage::new(2, 2));
        let session = registry.create(image.clone(), 1);
        let first = session.snapshot().unwrap().image;
        drop(first);
        let retry = session.snapshot().unwrap().image;
        assert!(Arc::ptr_eq(&image, &retry));
    }

    #[test]
    fn updates_and_closes_reject_old_publications() {
        let registry = SessionRegistry::default();
        let session = registry.create(ImageAsset::new(RgbaImage::new(2, 2)), 1);
        let old_scope = session.scope().unwrap();
        registry.publish_revision(session.id(), 2).unwrap();
        assert!(!registry.accepts(old_scope));
        assert!(registry.accepts(session.scope().unwrap()));
        assert!(registry.close(session.id()));
        assert!(session.snapshot().is_none());
        assert!(!registry.accepts(JobScope::Session {
            id: session.id(),
            revision: 2
        }));
    }
}
