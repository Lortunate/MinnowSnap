mod control_plane;

#[cfg(target_os = "windows")]
mod windows;

use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use tracing::info;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShutdownTrigger {
    TrayMenu,
    CtrlC,
    PipeCommand,
}

static SHUTTING_DOWN: AtomicBool = AtomicBool::new(false);

pub fn init_control_plane() {
    SHUTTING_DOWN.store(false, Ordering::SeqCst);
    control_plane::init();
}

pub fn clear_control_plane() {
    control_plane::clear();
}

pub fn subscribe() -> Option<broadcast::Receiver<ShutdownTrigger>> {
    control_plane::subscribe()
}

pub fn cancellation_token() -> Option<CancellationToken> {
    control_plane::cancellation_token()
}

pub fn is_shutting_down() -> bool {
    SHUTTING_DOWN.load(Ordering::SeqCst)
}

pub fn request_shutdown(trigger: ShutdownTrigger) -> bool {
    if SHUTTING_DOWN.swap(true, Ordering::SeqCst) {
        return false;
    }

    info!("Shutdown requested: {trigger:?}");
    control_plane::broadcast(trigger);
    control_plane::cancel();
    true
}

#[cfg(target_os = "windows")]
pub use windows::{CONTROL_PIPE_NAME, ShutdownClientError, install_ctrl_c_handler, shutdown_running_instance, start_control_pipe_server};

#[cfg(test)]
mod tests {
    use super::{ShutdownTrigger, clear_control_plane, init_control_plane, request_shutdown, subscribe};

    #[test]
    fn shutdown_request_is_idempotent() {
        init_control_plane();
        let mut rx = subscribe().expect("shutdown receiver");
        assert!(request_shutdown(ShutdownTrigger::CtrlC));
        assert!(!request_shutdown(ShutdownTrigger::PipeCommand));

        let received = crate::RUNTIME.block_on(async { rx.recv().await }).expect("shutdown trigger");
        assert_eq!(received, ShutdownTrigger::CtrlC);

        clear_control_plane();
    }
}
