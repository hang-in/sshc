//! `sshc --doctor`: report-only environment check.
//!
//! Inspects `~/.ssh/config`, `~/.ssh` mode, `~/.ssh/config.d/sshc.conf`,
//! the `Include` line, the `ssh` binary on PATH, and `SSH_AUTH_SOCK`.
//! Prints `PASS` / `WARN` / `FAIL` per check and exits 0 unless any
//! check is `FAIL`. Never mutates state.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[derive(Debug, Clone, Copy)]
enum Status {
    Pass,
    Warn,
    Fail,
}

impl Status {
    fn label(self) -> &'static str {
        match self {
            Status::Pass => "PASS",
            Status::Warn => "WARN",
            Status::Fail => "FAIL",
        }
    }
}

struct Check {
    name: &'static str,
    status: Status,
    detail: String,
}

fn home() -> Option<PathBuf> {
    dirs::home_dir()
}

fn check_ssh_config() -> Check {
    let Some(home) = home() else {
        return Check {
            name: "~/.ssh/config",
            status: Status::Fail,
            detail: "could not resolve home directory".into(),
        };
    };
    let path = home.join(".ssh").join("config");
    if path.exists() {
        Check {
            name: "~/.ssh/config",
            status: Status::Pass,
            detail: format!("found at {}", path.display()),
        }
    } else {
        Check {
            name: "~/.ssh/config",
            status: Status::Warn,
            detail: "not found — sshc has no hosts to browse until you create it".into(),
        }
    }
}

fn check_ssh_dir_perms() -> Check {
    let Some(home) = home() else {
        return Check {
            name: "~/.ssh permissions",
            status: Status::Fail,
            detail: "could not resolve home directory".into(),
        };
    };
    let dir = home.join(".ssh");
    let metadata = match std::fs::metadata(&dir) {
        Ok(m) => m,
        Err(_) => {
            return Check {
                name: "~/.ssh permissions",
                status: Status::Warn,
                detail: format!("{} does not exist", dir.display()),
            };
        }
    };
    if !metadata.is_dir() {
        return Check {
            name: "~/.ssh permissions",
            status: Status::Fail,
            detail: format!("{} is not a directory", dir.display()),
        };
    }
    #[cfg(unix)]
    {
        let mode = metadata.permissions().mode() & 0o777;
        if mode == 0o700 {
            Check {
                name: "~/.ssh permissions",
                status: Status::Pass,
                detail: "mode 0700".into(),
            }
        } else {
            Check {
                name: "~/.ssh permissions",
                status: Status::Warn,
                detail: format!("mode {mode:o} (OpenSSH expects 0700)"),
            }
        }
    }
    #[cfg(not(unix))]
    {
        // Windows ACLs aren't checked — different model from Unix mode bits.
        let _ = metadata;
        Check {
            name: "~/.ssh permissions",
            status: Status::Pass,
            detail: "Windows: ACL not checked (no Unix mode equivalent)".into(),
        }
    }
}

fn check_sshc_conf() -> Check {
    let Some(home) = home() else {
        return Check {
            name: "sshc.conf",
            status: Status::Fail,
            detail: "could not resolve home directory".into(),
        };
    };
    let path = home.join(".ssh").join("config.d").join("sshc.conf");
    if path.exists() {
        Check {
            name: "sshc.conf",
            status: Status::Pass,
            detail: format!("found at {}", path.display()),
        }
    } else {
        Check {
            name: "sshc.conf",
            status: Status::Warn,
            detail: "not created yet — run `sshc -m` and add a host".into(),
        }
    }
}

fn check_include_line() -> Check {
    let Some(home) = home() else {
        return Check {
            name: "Include line",
            status: Status::Fail,
            detail: "could not resolve home directory".into(),
        };
    };
    let config_path = home.join(".ssh").join("config");
    let target = "config.d/sshc.conf";
    let contents = match std::fs::read_to_string(&config_path) {
        Ok(s) => s,
        Err(_) => {
            return Check {
                name: "Include line",
                status: Status::Warn,
                detail: format!("{} not readable; nothing to check", config_path.display()),
            };
        }
    };
    let found = contents.lines().any(|line| {
        let trimmed = line.trim_start();
        let lower = trimmed.to_ascii_lowercase();
        lower.starts_with("include") && trimmed.contains(target)
    });
    if found {
        Check {
            name: "Include line",
            status: Status::Pass,
            detail: format!("present in {}", config_path.display()),
        }
    } else {
        Check {
            name: "Include line",
            status: Status::Warn,
            detail: "missing — managed hosts won't show; press 'i' in manage mode to inject".into(),
        }
    }
}

fn check_ssh_binary() -> Check {
    match Command::new("ssh").arg("-V").output() {
        Ok(out) => {
            // ssh -V emits to stderr.
            let version = String::from_utf8_lossy(&out.stderr).trim().to_string();
            let detail = if version.is_empty() {
                "found on PATH".into()
            } else {
                version
            };
            Check {
                name: "ssh binary",
                status: Status::Pass,
                detail,
            }
        }
        Err(e) => Check {
            name: "ssh binary",
            status: Status::Fail,
            detail: format!("ssh not on PATH: {e}"),
        },
    }
}

fn check_ssh_auth_sock() -> Check {
    match std::env::var("SSH_AUTH_SOCK") {
        Ok(p) if !p.is_empty() => Check {
            name: "SSH_AUTH_SOCK",
            status: Status::Pass,
            detail: format!("set ({})", short_path(Path::new(&p))),
        },
        _ => {
            #[cfg(unix)]
            {
                Check {
                    name: "SSH_AUTH_SOCK",
                    status: Status::Warn,
                    detail: "not set — ssh-agent identities won't be available".into(),
                }
            }
            #[cfg(not(unix))]
            {
                // Windows uses named pipes for the OpenSSH agent and
                // Pageant for PuTTY/WinSCP. SSH_AUTH_SOCK is irrelevant
                // there. Report informationally rather than as a WARN.
                Check {
                    name: "SSH_AUTH_SOCK",
                    status: Status::Pass,
                    detail: "Windows: not applicable (use Windows OpenSSH agent or Pageant)".into(),
                }
            }
        }
    }
}

fn short_path(p: &Path) -> String {
    let s = p.display().to_string();
    if s.len() > 50 {
        format!("…{}", &s[s.len() - 49..])
    } else {
        s
    }
}

/// Run all checks, print the report, and return an `ExitCode` reflecting
/// the worst status seen (`FAIL` → `FAILURE`, anything else → `SUCCESS`).
pub fn run() -> ExitCode {
    let checks = [
        check_ssh_config(),
        check_ssh_dir_perms(),
        check_sshc_conf(),
        check_include_line(),
        check_ssh_binary(),
        check_ssh_auth_sock(),
    ];

    let max_name = checks.iter().map(|c| c.name.len()).max().unwrap_or(0);

    println!("sshc {} — doctor", env!("CARGO_PKG_VERSION"));
    println!();
    let mut had_fail = false;
    let mut warn_count = 0;
    for c in &checks {
        println!(
            "  [{}] {:<width$}  {}",
            c.status.label(),
            c.name,
            c.detail,
            width = max_name
        );
        match c.status {
            Status::Fail => had_fail = true,
            Status::Warn => warn_count += 1,
            Status::Pass => {}
        }
    }
    println!();
    if had_fail {
        println!("Result: FAIL — one or more checks failed.");
        ExitCode::FAILURE
    } else if warn_count > 0 {
        println!("Result: OK with {warn_count} warning(s) — no FAIL.");
        ExitCode::SUCCESS
    } else {
        println!("Result: OK — all checks passed.");
        ExitCode::SUCCESS
    }
}
