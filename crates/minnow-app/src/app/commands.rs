use std::process::ExitCode;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Command {
    Run,
    Shutdown,
}

pub fn parse_command_from(mut args: impl Iterator<Item = String>) -> Result<Command, String> {
    match args.next().as_deref() {
        None | Some("run") => Ok(Command::Run),
        Some("shutdown") => Ok(Command::Shutdown),
        Some(other) => Err(format!("Unknown command '{other}'. Supported commands: run, shutdown")),
    }
}

pub fn parse_command() -> Result<Command, String> {
    parse_command_from(std::env::args().skip(1))
}

pub fn run_command(command: Command) -> ExitCode {
    match command {
        Command::Run => {
            super::runtime::run();
            ExitCode::SUCCESS
        }
        Command::Shutdown => ExitCode::from(super::runtime::shutdown_running_instance()),
    }
}

#[cfg(test)]
mod tests {
    use super::{Command, parse_command_from, run_command};
    use std::process::ExitCode;

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

    #[test]
    fn run_command_has_stable_exit_code_signature() {
        let _run_command: fn(Command) -> ExitCode = run_command;
    }
}
