use super::ApplicationServices;
use crate::{
    AnnotationSnapshot, CapturedFrame, ImageInput, ImageProjection, ImageRef, JobScope, OperationResult, SessionHandle, SessionId, ShutdownReport,
};
use std::time::Duration;

impl ApplicationServices {
    pub fn create_session(&self, image: ImageRef, revision: u64) -> SessionHandle {
        self.sessions.create(image, revision)
    }

    pub fn create_capture_session(&self, frame: CapturedFrame, revision: u64) -> SessionHandle {
        self.sessions.create_capture(frame, revision)
    }

    pub fn publish_session_revision(&self, id: SessionId, revision: u64) -> OperationResult<()> {
        self.sessions.publish_revision(id, revision)?;
        self.jobs.cancel_obsolete_session_jobs(id, revision);
        Ok(())
    }

    pub fn publish_session_output(
        &self,
        id: SessionId,
        revision: u64,
        projection: ImageProjection,
        annotations: Option<AnnotationSnapshot>,
    ) -> OperationResult<ImageInput> {
        let input = self.sessions.publish_output(id, revision, projection, annotations)?;
        self.jobs.cancel_obsolete_session_jobs(id, revision);
        Ok(input)
    }

    pub fn close_session(&self, id: SessionId) -> bool {
        let closed = self.sessions.close(id);
        self.jobs.cancel_derived_session_jobs(id);
        closed
    }

    pub fn close_all_sessions(&self) {
        for id in self.sessions.close_all() {
            self.jobs.cancel_derived_session_jobs(id);
        }
    }

    pub fn is_current(&self, scope: JobScope) -> bool {
        self.sessions.accepts(scope)
    }

    pub async fn shutdown(&self) -> ShutdownReport {
        self.close_all_sessions();
        self.jobs.shutdown().await
    }

    pub async fn shutdown_with_timeout(&self, timeout: Duration) -> ShutdownReport {
        self.close_all_sessions();
        self.jobs.shutdown_with_timeout(timeout).await
    }
}
