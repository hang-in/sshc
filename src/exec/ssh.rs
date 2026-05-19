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
/// `ssh_binary` is the path/name of the ssh executable. Production callers pass
/// `"ssh"`; tests pass the path to a mock binary.
///
/// MVP simplification: `Command::status()` collapses spawn-failure and
/// wait-failure into one `io::Result`. We map both to `SshError::LaunchFailed`.
/// The `WaitFailed` variant is reserved for a future migration to
/// `Command::spawn() + Child::wait()` if we ever need to distinguish them.
pub fn ssh_run(host_alias: &str, ssh_binary: &str) -> Result<SshResult, SshError> {
    match Command::new(ssh_binary).arg(host_alias).status() {
        Ok(status) => Ok(classify_exit_status(status)),
        Err(e) => Err(SshError::LaunchFailed(e)),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(unix)]
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
