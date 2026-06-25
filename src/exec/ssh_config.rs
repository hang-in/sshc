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

/// v0.9 G4: run `ssh -G <alias>` and reduce the output to a one-line
/// `ssh user@host -p port -i key` invocation suitable for the clipboard
/// or a shared note. Drops options OpenSSH would default-fill anyway
/// (port 22, empty user, empty identityfile) so the resulting string
/// reads like a hand-written command.
pub fn ssh_command_for_alias(alias: &str) -> Result<String, ValidationError> {
    let out = validate_alias(alias)?;
    Ok(build_ssh_command(alias, &out))
}

/// Pure helper for `ssh_command_for_alias`. Takes a raw `ssh -G` dump
/// and produces the one-line command — exposed so unit tests can drive
/// it with fixture strings without spawning ssh.
pub fn build_ssh_command(alias: &str, ssh_g_output: &str) -> String {
    let mut user: Option<&str> = None;
    let mut hostname: Option<&str> = None;
    let mut port: Option<&str> = None;
    // `ssh -G` emits one `identityfile` line per default key plus any
    // explicitly configured ones, with the configured one first. Take
    // the first non-default-looking entry: if the user configured
    // /Users/.../id_ed25519 it shows up before the default ~/.ssh/id_rsa
    // fallbacks, so the first occurrence is what we want.
    let mut identity: Option<&str> = None;
    for line in ssh_g_output.lines() {
        let mut parts = line.splitn(2, ' ');
        let key = parts.next().unwrap_or("");
        let val = parts.next().unwrap_or("").trim();
        match key {
            "user" => user = Some(val),
            "hostname" => hostname = Some(val),
            "port" => port = Some(val),
            "identityfile" if identity.is_none() => identity = Some(val),
            _ => {}
        }
    }
    let host = hostname.filter(|s| !s.is_empty()).unwrap_or(alias);
    let mut cmd = String::from("ssh ");
    if let Some(u) = user.filter(|s| !s.is_empty()) {
        cmd.push_str(u);
        cmd.push('@');
    }
    cmd.push_str(host);
    if let Some(p) = port.filter(|s| !s.is_empty() && *s != "22") {
        cmd.push_str(" -p ");
        cmd.push_str(p);
    }
    if let Some(i) = identity.filter(|s| !s.is_empty()) {
        cmd.push_str(" -i ");
        cmd.push_str(i);
    }
    cmd
}

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

    // ----- v0.9 G4: build_ssh_command tests -----

    fn full_dump() -> String {
        // Trimmed snapshot of a real `ssh -G` output. Only the four
        // fields we extract matter; everything else is noise that
        // build_ssh_command must skip.
        "user d9ng
hostname yongseek.iptime.org
port 2232
identityfile /Users/d9ng/.ssh/id_ed25519
identityfile ~/.ssh/id_rsa
identityfile ~/.ssh/id_ecdsa
loglevel INFO
"
        .to_string()
    }

    #[test]
    fn build_full_command_with_all_fields() {
        let cmd = build_ssh_command("boxie2", &full_dump());
        assert_eq!(
            cmd,
            "ssh d9ng@yongseek.iptime.org -p 2232 -i /Users/d9ng/.ssh/id_ed25519"
        );
    }

    #[test]
    fn port_22_is_omitted() {
        let dump = "user d9ng\nhostname example.com\nport 22\n";
        assert_eq!(build_ssh_command("ex", dump), "ssh d9ng@example.com");
    }

    #[test]
    fn empty_user_is_omitted() {
        let dump = "user \nhostname example.com\nport 2222\n";
        assert_eq!(build_ssh_command("ex", dump), "ssh example.com -p 2222");
    }

    #[test]
    fn missing_hostname_falls_back_to_alias() {
        let dump = "user d9ng\nport 22\n";
        assert_eq!(build_ssh_command("ex-alias", dump), "ssh d9ng@ex-alias");
    }

    #[test]
    fn empty_identity_is_omitted() {
        let dump = "user d9ng\nhostname example.com\nport 22\nidentityfile \n";
        assert_eq!(build_ssh_command("ex", dump), "ssh d9ng@example.com");
    }

    #[test]
    fn first_identityfile_wins_over_defaults() {
        // OpenSSH emits the configured key first, then the rolling
        // defaults. build_ssh_command must pick the first one.
        let dump = "user d9ng
hostname example.com
port 22
identityfile ~/.ssh/id_picked
identityfile ~/.ssh/id_rsa
";
        assert_eq!(
            build_ssh_command("ex", dump),
            "ssh d9ng@example.com -i ~/.ssh/id_picked"
        );
    }
}
