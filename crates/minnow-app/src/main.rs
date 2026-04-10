#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod bootstrap;
mod composition;
mod runtime;

#[cfg(not(feature = "dhat-heap"))]
use mimalloc::MiMalloc;
use std::process::ExitCode;

#[cfg(feature = "dhat-heap")]
#[global_allocator]
static GLOBAL: dhat::Alloc = dhat::Alloc;

#[cfg(not(feature = "dhat-heap"))]
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Command {
    Run,
    Shutdown,
}

fn parse_command_from(mut args: impl Iterator<Item = String>) -> Result<Command, String> {
    match args.next().as_deref() {
        None | Some("run") => Ok(Command::Run),
        Some("shutdown") => Ok(Command::Shutdown),
        Some(other) => Err(format!("Unknown command '{other}'. Supported commands: run, shutdown")),
    }
}

fn parse_command() -> Result<Command, String> {
    parse_command_from(std::env::args().skip(1))
}

fn main() -> ExitCode {
    #[cfg(feature = "dhat-heap")]
    let _dhat_profiler = dhat::Profiler::new_heap();

    let command = match parse_command() {
        Ok(command) => command,
        Err(err) => {
            eprintln!("{err}");
            return ExitCode::from(1);
        }
    };

    match command {
        Command::Run => {
            runtime::run();
            ExitCode::SUCCESS
        }
        Command::Shutdown => ExitCode::from(runtime::shutdown_running_instance()),
    }
}

#[cfg(test)]
mod tests {
    use super::{Command, parse_command_from};

    #[test]
    fn parse_command_defaults_to_run() {
        let cmd = parse_command_from(Vec::<String>::new().into_iter()).expect("command");
        assert_eq!(cmd, Command::Run);
    }

    #[test]
    fn parse_command_accepts_shutdown() {
        let cmd = parse_command_from(vec!["shutdown".to_string()].into_iter()).expect("command");
        assert_eq!(cmd, Command::Shutdown);
    }

    #[test]
    fn parse_command_rejects_unknown_values() {
        let err = parse_command_from(vec!["bad".to_string()].into_iter()).expect_err("expected parse error");
        assert!(err.contains("Unknown command"));
    }
}
