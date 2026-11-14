use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

#[derive(Clone, Copy, Debug)]
pub(crate) enum ShutdownTrigger {
    Tray,
    Signal,
    #[cfg(target_os = "windows")]
    ControlPipe,
}

/// A request signal, not proof that application work has finished.
#[derive(Clone, Default)]
pub(crate) struct ShutdownRequest {
    requested_at: Arc<OnceLock<Instant>>,
    completed: Arc<OnceLock<bool>>,
    token: CancellationToken,
}

impl ShutdownRequest {
    pub(crate) fn request(&self, trigger: ShutdownTrigger) -> bool {
        if self.requested_at.set(Instant::now()).is_err() {
            return false;
        }
        tracing::info!(?trigger, "Shutdown requested");
        self.token.cancel();
        true
    }

    pub(crate) fn is_requested(&self) -> bool {
        self.requested_at.get().is_some()
    }

    pub(crate) fn remaining(&self, budget: Duration) -> Duration {
        self.requested_at
            .get()
            .map_or(budget, |requested| budget.saturating_sub(requested.elapsed()))
    }

    pub(crate) fn completion(&self) -> Option<bool> {
        self.completed.get().copied()
    }

    pub(crate) fn complete(&self, drained: bool) {
        let _ = self.completed.set(drained);
    }

    pub(crate) async fn cancelled(&self) {
        self.token.cancelled().await;
    }

    pub(crate) fn install_signal_handler(&self) -> Result<(), ctrlc::Error> {
        let request = self.clone();
        ctrlc::set_handler(move || {
            request.request(ShutdownTrigger::Signal);
        })
    }
}

#[cfg(target_os = "windows")]
pub(crate) mod control_pipe {
    use super::{ShutdownRequest, ShutdownTrigger};
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::windows::named_pipe::{ClientOptions, ServerOptions};

    const NAME: &str = r"\\.\pipe\MinnowSnap.Control.v1";
    const REQUEST: &[u8] = b"shutdown\n";
    const ACCEPTED: &[u8] = b"ok\n";
    const TIMEOUT: Duration = Duration::from_millis(1500);

    pub(crate) async fn serve(request: ShutdownRequest) {
        while !request.is_requested() {
            let mut server = match ServerOptions::new().create(NAME) {
                Ok(server) => server,
                Err(error) => {
                    tracing::error!(%error, "Failed to create shutdown control pipe");
                    return;
                }
            };
            tokio::select! {
                _ = request.cancelled() => return,
                result = server.connect() => {
                    if let Err(error) = result {
                        tracing::warn!(%error, "Shutdown control connection failed");
                        continue;
                    }
                }
            }
            let mut bytes = [0; 64];
            let count = tokio::select! {
                _ = request.cancelled() => return,
                result = tokio::time::timeout(TIMEOUT, server.read(&mut bytes)) => match result {
                    Ok(Ok(count)) => count,
                    _ => continue,
                }
            };
            if &bytes[..count] != REQUEST {
                tracing::warn!("Invalid shutdown control message");
                continue;
            }
            // The protocol acknowledges admission. Work is drained by the host.
            let acknowledgement = tokio::time::timeout(TIMEOUT, async {
                server.write_all(ACCEPTED).await?;
                server.flush().await
            })
            .await;
            if !matches!(acknowledgement, Ok(Ok(()))) {
                tracing::warn!("Shutdown acknowledgement could not be delivered");
            }
            request.request(ShutdownTrigger::ControlPipe);
        }
    }

    pub(crate) async fn request_shutdown() -> Result<(), ClientError> {
        let mut client = ClientOptions::new().open(NAME).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                ClientError::NotRunning
            } else {
                ClientError::Transport(error.to_string())
            }
        })?;
        tokio::time::timeout(TIMEOUT, async {
            client.write_all(REQUEST).await?;
            client.flush().await?;
            let mut response = [0; 16];
            let count = client.read(&mut response).await?;
            Ok::<_, std::io::Error>(response[..count] == *ACCEPTED)
        })
        .await
        .map_err(|_| ClientError::Transport("shutdown request timed out".into()))?
        .map_err(|error| ClientError::Transport(error.to_string()))
        .and_then(|accepted| {
            if accepted {
                Ok(())
            } else {
                Err(ClientError::Transport("unexpected shutdown response".into()))
            }
        })
    }

    #[derive(Debug, thiserror::Error)]
    pub(crate) enum ClientError {
        #[error("no running MinnowSnap instance was found")]
        NotRunning,
        #[error("{0}")]
        Transport(String),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_are_idempotent_and_instances_do_not_share_state() {
        let first = ShutdownRequest::default();
        let clone = first.clone();
        let next = ShutdownRequest::default();
        assert!(first.request(ShutdownTrigger::Tray));
        assert!(!clone.request(ShutdownTrigger::Signal));
        assert!(clone.is_requested());
        assert!(!next.is_requested());
    }
}
