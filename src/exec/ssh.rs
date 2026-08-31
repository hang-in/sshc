use std::env;
use std::process::{Command, ExitStatus};

#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;

use crate::error::SshError;

/// Outcome of a single ssh invocation.
#[derive(Debug, PartialEq, Eq)]
pub enum SshResult {
    Success,
    Interrupted,
    ConnectFailed(i32),
    Failed(i32),
    Crashed(i32),
    UnknownTermination,
}

/// Spawn ssh as a child, inherit stdio, wait for exit, classify.
///
/// PRE: caller has suspended the TUI terminal (raw mode off, alt screen exited).
///      This function does NOT manipulate terminal state.
///
/// `ssh_binary` is the path/name of the executable to spawn; `extra_args` are
/// inserted before `host_alias` (e.g. `["+kitten", "ssh"]` for kitty).
/// Production callers resolve both via [`resolve_ssh_command`]; tests pass
/// the path to a mock binary with no extra args.
///
/// MVP simplification: `Command::status()` collapses spawn-failure and
/// wait-failure into one `io::Result`. We map both to `SshError::LaunchFailed`.
/// The `WaitFailed` variant is reserved for a future migration to
/// `Command::spawn() + Child::wait()` if we ever need to distinguish them.
pub fn ssh_run(
    host_alias: &str,
    ssh_binary: &str,
    extra_args: &[String],
) -> Result<SshResult, SshError> {
    match Command::new(ssh_binary)
        .args(extra_args)
        .arg(host_alias)
        .status()
    {
        Ok(status) => Ok(classify_exit_status(status)),
        Err(e) => Err(SshError::LaunchFailed(e)),
    }
}

/// Resolves the program + leading args to spawn for an ssh connection.
///
/// `SSHC_SSH_COMMAND` wins when set to a non-empty value, split on
/// whitespace (e.g. `SSHC_SSH_COMMAND="kitty +kitten ssh"` launches
/// `kitty +kitten ssh <alias>`). When unset or empty, defaults to `ssh`
/// with no extra args.
pub fn resolve_ssh_command() -> (String, Vec<String>) {
    let raw = env::var("SSHC_SSH_COMMAND").unwrap_or_default();
    let mut parts = raw.split_whitespace().map(String::from);
    match parts.next() {
        Some(program) => (program, parts.collect()),
        None => ("ssh".to_string(), Vec::new()),
    }
}

/// Pure classification of a child's ExitStatus into an SshResult.
/// Exposed `pub(crate)` so it can be unit-tested without spawning a real process.
pub(crate) fn classify_exit_status(status: ExitStatus) -> SshResult {
    if let Some(code) = status.code() {
        match code {
            0 => SshResult::Success,
            130 => SshResult::Interrupted,
            255 => SshResult::ConnectFailed(255),
            other => SshResult::Failed(other),
        }
    } else {
        #[cfg(unix)]
        {
            if let Some(sig) = status.signal() {
                return match sig {
                    2 | 15 => SshResult::Interrupted,
                    _ => SshResult::Crashed(sig),
                };
            }
        }
        SshResult::UnknownTermination
    }
}

// Unix-only test block: classify_exit_status takes a std::process::ExitStatus
// and its `from_raw` ctor is unix-only. On Windows we'd need a different
// classification table entirely; v0.7 keeps the existing exit-code mapping
// Unix-only and lets the Windows path through `Command::new` use the default
// status interpretation.
#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn test_classify_success() {
        let status = ExitStatus::from_raw(0);
        assert_eq!(classify_exit_status(status), SshResult::Success);
    }

    #[test]
    #[cfg(unix)]
    fn test_classify_interrupted_130() {
        let status = ExitStatus::from_raw(130 << 8);
        assert_eq!(classify_exit_status(status), SshResult::Interrupted);
    }

    #[test]
    #[cfg(unix)]
    fn test_classify_connect_failed_255() {
        let status = ExitStatus::from_raw(255 << 8);
        assert_eq!(classify_exit_status(status), SshResult::ConnectFailed(255));
    }

    #[test]
    #[cfg(unix)]
    fn test_classify_failed_other() {
        let status = ExitStatus::from_raw(42 << 8);
        assert_eq!(classify_exit_status(status), SshResult::Failed(42));
    }

    #[test]
    #[cfg(unix)]
    fn test_classify_signal_sigint() {
        // Raw status word for "killed by SIGINT (2)": low 7 bits = signal, high bit 0
        let status = ExitStatus::from_raw(2);
        assert_eq!(classify_exit_status(status), SshResult::Interrupted);
    }

    #[test]
    #[cfg(unix)]
    fn test_classify_signal_other() {
        // Raw status word for "killed by SIGSEGV (11)"
        let status = ExitStatus::from_raw(11);
        assert_eq!(classify_exit_status(status), SshResult::Crashed(11));
    }
}

// Separate module: resolve_ssh_command has no unix-specific behavior, but
// its tests mutate the process-wide SSHC_SSH_COMMAND env var, so each test
// saves/restores it. Not safe under parallel test execution across other
// modules that touch the same var, but there are none today (see
// exec::editor's EDITOR tests for the established precedent).
#[cfg(test)]
mod resolve_ssh_command_tests {
    use super::*;

    #[test]
    fn test_default_is_ssh_with_no_args() {
        let original = env::var("SSHC_SSH_COMMAND").ok();
        env::remove_var("SSHC_SSH_COMMAND");

        assert_eq!(resolve_ssh_command(), ("ssh".to_string(), Vec::new()));

        if let Some(val) = original {
            env::set_var("SSHC_SSH_COMMAND", val);
        }
    }

    #[test]
    fn test_empty_env_var_falls_back_to_ssh() {
        let original = env::var("SSHC_SSH_COMMAND").ok();
        env::set_var("SSHC_SSH_COMMAND", "");

        assert_eq!(resolve_ssh_command(), ("ssh".to_string(), Vec::new()));

        if let Some(val) = original {
            env::set_var("SSHC_SSH_COMMAND", val);
        } else {
            env::remove_var("SSHC_SSH_COMMAND");
        }
    }

    #[test]
    fn test_multi_word_command_splits_program_and_args() {
        let original = env::var("SSHC_SSH_COMMAND").ok();
        env::set_var("SSHC_SSH_COMMAND", "kitty +kitten ssh");

        assert_eq!(
            resolve_ssh_command(),
            (
                "kitty".to_string(),
                vec!["+kitten".to_string(), "ssh".to_string()]
            )
        );

        if let Some(val) = original {
            env::set_var("SSHC_SSH_COMMAND", val);
        } else {
            env::remove_var("SSHC_SSH_COMMAND");
        }
    }
}
