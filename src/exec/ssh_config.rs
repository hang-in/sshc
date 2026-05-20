//! `ssh -G <alias>` validation: ask OpenSSH itself how it would resolve
//! an alias's config. Used by manage mode's `v` key to surface the
//! authoritative effective config in an Info modal, without forcing a
//! real connection.
//!
//! Lives in `src/exec/` (not `src/app/*`) so R-G1 stays clean — the
//! `app` module never spawns a process itself.

use std::process::Command;

/// Failure modes for `validate_alias`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    /// `ssh` binary not on PATH (or unreadable).
    SshNotFound(String),
    /// `ssh -G` exited non-zero. Includes the captured stderr verbatim
    /// so the modal can show OpenSSH's own diagnostic.
    NonZeroExit { code: Option<i32>, stderr: String },
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationError::SshNotFound(msg) => write!(f, "ssh not found: {msg}"),
            ValidationError::NonZeroExit { code, stderr } => {
                let c = code.map(|n| n.to_string()).unwrap_or_else(|| "?".into());
                write!(f, "ssh -G exited {c}\n\n{stderr}")
            }
        }
    }
}

impl std::error::Error for ValidationError {}

/// Run `ssh -G <alias>` and return the captured stdout on success. No
/// network I/O — `-G` parses local config only. Typical runtime <50 ms
/// for a 200-host config, so this is called synchronously from the UI
/// thread.
pub fn validate_alias(alias: &str) -> Result<String, ValidationError> {
    let output = Command::new("ssh")
        .arg("-G")
        .arg(alias)
        .output()
        .map_err(|e| ValidationError::SshNotFound(e.to_string()))?;

    if !output.status.success() {
        return Err(ValidationError::NonZeroExit {
            code: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ssh_not_found_error_displays_message() {
        let e = ValidationError::SshNotFound("no such file".into());
        let s = format!("{e}");
        assert!(s.contains("ssh not found"));
        assert!(s.contains("no such file"));
    }

    #[test]
    fn non_zero_exit_displays_stderr_and_code() {
        let e = ValidationError::NonZeroExit {
            code: Some(255),
            stderr: "unknown option foo".into(),
        };
        let s = format!("{e}");
        assert!(s.contains("255"));
        assert!(s.contains("unknown option foo"));
    }

    /// Integration check that only runs when `ssh` is present on PATH —
    /// most CI/dev environments have it. We invoke `-G` against a sentinel
    /// alias OpenSSH will happily resolve to defaults, so the test is
    /// resilient to whatever ~/.ssh/config the host has.
    #[test]
    fn validate_alias_against_dummy_succeeds_when_ssh_present() {
        if which_ssh_present() {
            let out = validate_alias("sshc-validate-test-dummy")
                .expect("ssh -G should resolve any alias to defaults");
            // `ssh -G` always emits a `hostname` line.
            assert!(
                out.lines().any(|l| l.starts_with("hostname")),
                "expected `hostname` line in ssh -G output, got:\n{out}"
            );
        }
    }

    fn which_ssh_present() -> bool {
        Command::new("ssh")
            .arg("-V")
            .output()
            .map(|o| o.status.success() || !o.stderr.is_empty())
            .unwrap_or(false)
    }
}
