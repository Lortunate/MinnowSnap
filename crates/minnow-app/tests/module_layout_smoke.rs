#[allow(unused_imports)]
use minnow_app::{
    app::{self, bootstrap as _, composition as _, runtime as _},
    platform, services, ui,
};
use std::process::ExitCode;

#[test]
fn app_public_api_matches_task1_surface() {
    let command = app::parse_command_from(std::iter::empty::<String>()).expect("command");
    assert_eq!(command, app::Command::Run);

    let _parse_command: fn() -> Result<app::Command, String> = app::parse_command;
    let _run_command: fn(app::Command) -> ExitCode = app::run_command;
    let _runtime_run: fn() = app::runtime::run;
    let _runtime_shutdown: fn() -> u8 = app::runtime::shutdown_running_instance;
}

#[test]
fn top_level_modules_export_stub_surface() {
    assert_eq!(platform::module_tag, "platform");
    assert_eq!(services::module_tag, "services");
    assert_eq!(ui::module_tag, "ui");
}
