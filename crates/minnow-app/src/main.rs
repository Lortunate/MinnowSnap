#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#[cfg(not(feature = "dhat-heap"))]
use mimalloc::MiMalloc;
use std::process::ExitCode;

#[cfg(feature = "dhat-heap")]
#[global_allocator]
static GLOBAL: dhat::Alloc = dhat::Alloc;

#[cfg(not(feature = "dhat-heap"))]
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

fn main() -> ExitCode {
    #[cfg(feature = "dhat-heap")]
    let _dhat_profiler = dhat::Profiler::new_heap();

    match minnow_app::app::parse_command() {
        Ok(command) => minnow_app::app::run_command(command),
        Err(err) => {
            eprintln!("{err}");
            ExitCode::from(1)
        }
    }
}
