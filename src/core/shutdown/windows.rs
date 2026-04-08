use super::{ShutdownTrigger, cancellation_token, request_shutdown};
use std::sync::Once;
use thiserror::Error;
use tracing::{error, warn};

pub const CONTROL_PIPE_NAME: &str = r"\\.\pipe\MinnowSnap.Control.v1";

const SHUTDOWN_REQUEST: &[u8] = b"shutdown\n";
const SHUTDOWN_RESPONSE: &[u8] = b"ok\n";
const PIPE_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(1500);

static CTRL_C_HANDLER_ONCE: Once = Once::new();
static PIPE_SERVER_ONCE: Once = Once::new();

#[derive(Debug, Error)]
pub enum ShutdownClientError {
    #[error("no running MinnowSnap instance was found")]
    NotRunning,
    #[error("{0}")]
    Transport(String),
    #[error("{0}")]
    Protocol(String),
}

pub fn install_ctrl_c_handler() {
    CTRL_C_HANDLER_ONCE.call_once(|| {
        if let Err(err) = ctrlc::set_handler(|| {
            request_shutdown(ShutdownTrigger::CtrlC);
        }) {
            error!("Failed to install Ctrl+C handler: {err}");
        }
    });
}

pub fn start_control_pipe_server() {
    PIPE_SERVER_ONCE.call_once(|| {
        crate::core::RUNTIME.spawn(async {
            run_control_pipe_server().await;
        });
    });
}

async fn run_control_pipe_server() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::windows::named_pipe::ServerOptions;

    let Some(shutdown_token) = cancellation_token() else {
        warn!("Shutdown control plane is not initialized; skip control pipe server.");
        return;
    };

    while !shutdown_token.is_cancelled() {
        let mut server = match ServerOptions::new().create(CONTROL_PIPE_NAME) {
            Ok(server) => server,
            Err(err) => {
                warn!("Failed to create shutdown control pipe server: {err}");
                return;
            }
        };

        tokio::select! {
            _ = shutdown_token.cancelled() => return,
            result = server.connect() => {
                if let Err(err) = result {
                    warn!("Shutdown control pipe connect failed: {err}");
                    continue;
                }
            }
        }

        let mut request_buf = [0u8; 64];
        let read_len = tokio::select! {
            _ = shutdown_token.cancelled() => return,
            result = server.read(&mut request_buf) => {
                match result {
                    Ok(n) => n,
                    Err(err) => {
                        warn!("Failed to read shutdown control pipe request: {err}");
                        continue;
                    }
                }
            }
        };

        if read_len == 0 {
            continue;
        }

        if &request_buf[..read_len] != SHUTDOWN_REQUEST {
            warn!("Received invalid shutdown control pipe payload");
            continue;
        }

        request_shutdown(ShutdownTrigger::PipeCommand);

        if let Err(err) = server.write_all(SHUTDOWN_RESPONSE).await {
            warn!("Failed to write shutdown control pipe response: {err}");
            continue;
        }
        if let Err(err) = server.flush().await {
            warn!("Failed to flush shutdown control pipe response: {err}");
        }
    }
}

pub fn shutdown_running_instance() -> Result<(), ShutdownClientError> {
    crate::core::RUNTIME.block_on(send_shutdown_request())
}

async fn send_shutdown_request() -> Result<(), ShutdownClientError> {
    use std::io::ErrorKind;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::windows::named_pipe::ClientOptions;

    let mut client = match ClientOptions::new().open(CONTROL_PIPE_NAME) {
        Ok(client) => client,
        Err(err) if err.kind() == ErrorKind::NotFound => return Err(ShutdownClientError::NotRunning),
        Err(err) => return Err(ShutdownClientError::Transport(format!("failed to open shutdown control pipe: {err}"))),
    };

    tokio::time::timeout(PIPE_TIMEOUT, client.write_all(SHUTDOWN_REQUEST))
        .await
        .map_err(|_| ShutdownClientError::Transport("timed out writing shutdown request".to_string()))?
        .map_err(|err| ShutdownClientError::Transport(format!("failed to write shutdown request: {err}")))?;

    tokio::time::timeout(PIPE_TIMEOUT, client.flush())
        .await
        .map_err(|_| ShutdownClientError::Transport("timed out flushing shutdown request".to_string()))?
        .map_err(|err| ShutdownClientError::Transport(format!("failed to flush shutdown request: {err}")))?;

    let mut response_buf = [0u8; 16];
    let read_len = tokio::time::timeout(PIPE_TIMEOUT, client.read(&mut response_buf))
        .await
        .map_err(|_| ShutdownClientError::Transport("timed out reading shutdown response".to_string()))?
        .map_err(|err| ShutdownClientError::Transport(format!("failed to read shutdown response: {err}")))?;

    if read_len == 0 {
        return Err(ShutdownClientError::Protocol("shutdown response was empty".to_string()));
    }

    if &response_buf[..read_len] == SHUTDOWN_RESPONSE {
        return Ok(());
    }

    Err(ShutdownClientError::Protocol(format!(
        "unexpected shutdown response: {}",
        String::from_utf8_lossy(&response_buf[..read_len]).trim()
    )))
}
