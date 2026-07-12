//! Windows-safe background process spawning for OpenSSH and relay sidecars.
//!
//! GUI-subsystem binaries must not flash a console when probing `ssh -V` or
//! launching piped SSH transports.

use std::ffi::OsStr;
use std::process::Command;

/// Applies platform flags so a child process does not allocate a visible console.
pub fn configure_no_window(command: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;

        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

/// Builds a [`Command`] configured for silent background execution.
pub fn hidden_command(program: impl AsRef<OsStr>) -> Command {
    let mut command = Command::new(program);
    configure_no_window(&mut command);
    command
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_command_accepts_program_name() {
        let command = hidden_command("ssh");
        assert_eq!(command.get_program(), "ssh");
    }

    #[cfg(windows)]
    #[test]
    fn configure_no_window_accepts_mutable_command() {
        let mut command = Command::new("ssh");
        let configured = configure_no_window(&mut command);
        assert!(std::ptr::eq(configured, &mut command));
    }
}
