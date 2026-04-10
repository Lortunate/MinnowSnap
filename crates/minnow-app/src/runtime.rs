use crate::bootstrap::{ensure_single_instance, get_instance_id, init_logger, set_auto_start};
use crate::composition::run_application;
#[cfg(target_os = "macos")]
use crate::bootstrap::hide_dock_icon;
use minnow_core::shutdown;
use tracing::info;

pub fn run() {
    let _guard = init_logger();
    info!("Starting MinnowSnap...");

    if !ensure_single_instance(&get_instance_id()) {
        info!("Another instance is running, exiting.");
        return;
    }

    shutdown::init_control_plane();
    #[cfg(target_os = "windows")]
    {
        shutdown::install_ctrl_c_handler();
        shutdown::start_control_pipe_server();
    }

    #[cfg(target_os = "macos")]
    run_application(set_auto_start, hide_dock_icon);
    #[cfg(not(target_os = "macos"))]
    run_application(set_auto_start, noop_hide_dock_icon);

    shutdown::clear_control_plane();
}

pub fn shutdown_running_instance() -> u8 {
    #[cfg(target_os = "windows")]
    {
        match shutdown::shutdown_running_instance() {
            Ok(()) => 0,
            Err(shutdown::ShutdownClientError::NotRunning) => 2,
            Err(err) => {
                eprintln!("Failed to request graceful shutdown: {err}");
                3
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        eprintln!("Shutdown command is only supported on Windows.");
        3
    }
}

#[cfg(not(target_os = "macos"))]
fn noop_hide_dock_icon() {}
