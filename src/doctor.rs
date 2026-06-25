//! `sshc --doctor`: report-only environment check.
//!
//! Inspects `~/.ssh/config`, `~/.ssh` mode, `~/.ssh/config.d/sshc.conf`,
//! the `Include` line, the `ssh` binary on PATH, and `SSH_AUTH_SOCK`.
//! Prints `PASS` / `WARN` / `FAIL` per check and exits 0 unless any
//! check is `FAIL`. Never mutates state.

#[cfg(unix)]
use std::path::Path;
use std::path::PathBuf;
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

#[cfg(unix)]
fn check_ssh_auth_sock() -> Check {
    match std::env::var("SSH_AUTH_SOCK") {
        Ok(p) if !p.is_empty() => Check {
            name: "SSH_AUTH_SOCK",
            status: Status::Pass,
            detail: format!("set ({})", short_path(Path::new(&p))),
        },
        _ => Check {
            name: "SSH_AUTH_SOCK",
            status: Status::Warn,
            detail: "not set — ssh-agent identities won't be available".into(),
        },
    }
}

#[cfg(windows)]
fn check_ssh_auth_sock() -> Check {
    // On Windows, `SSH_AUTH_SOCK` is the wrong signal: agents communicate
    // over named pipes, not Unix sockets. v0.7 reported `not applicable`
    // unconditionally; v0.8 actually probes the two well-known pipe
    // names. Identity enumeration is explicitly out of scope (anti-features
    // 1 + 2) — presence only.
    const OPENSSH_PIPE: &str = r"\\.\pipe\openssh-ssh-agent";
    const PAGEANT_PIPE: &str = r"\\.\pipe\pageant";
    let openssh = windows_agent_pipe_present(OPENSSH_PIPE);
    let pageant = windows_agent_pipe_present(PAGEANT_PIPE);
    match (openssh, pageant) {
        (true, true) => Check {
            name: "SSH_AUTH_SOCK",
            status: Status::Pass,
            detail: "Windows OpenSSH agent + Pageant pipes present".into(),
        },
        (true, false) => Check {
            name: "SSH_AUTH_SOCK",
            status: Status::Pass,
            detail: format!("Windows OpenSSH agent pipe present ({OPENSSH_PIPE})"),
        },
        (false, true) => Check {
            name: "SSH_AUTH_SOCK",
            status: Status::Pass,
            detail: format!("Pageant pipe present ({PAGEANT_PIPE})"),
        },
        (false, false) => Check {
            name: "SSH_AUTH_SOCK",
            status: Status::Warn,
            detail: "no agent pipe found — start Windows OpenSSH agent \
                     (`Start-Service ssh-agent`) or run Pageant"
                .into(),
        },
    }
}

/// Windows-only: probe a named-pipe path with `CreateFileW(OPEN_EXISTING)`.
/// Any open failure (`ERROR_FILE_NOT_FOUND`, `ERROR_PIPE_NOT_CONNECTED`,
/// access denied, etc.) is treated as "pipe not available" — sshc only
/// asks "is something listening on this name", not "can I talk to it".
/// On success the handle is immediately closed.
#[cfg(windows)]
fn windows_agent_pipe_present(path: &str) -> bool {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FILE_ATTRIBUTE_NORMAL, OPEN_EXISTING,
    };

    // GENERIC_READ from `windows-sys` lives under `Win32_System_SystemServices`,
    // which we don't pull in. Define the constant locally; it's stable.
    const GENERIC_READ: u32 = 0x8000_0000;

    let wide: Vec<u16> = OsStr::new(path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
            GENERIC_READ,
            0,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            std::ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return false;
    }
    unsafe {
        CloseHandle(handle);
    }
    true
}

#[cfg(all(test, windows))]
mod windows_agent_tests {
    use super::windows_agent_pipe_present;

    #[test]
    fn nonexistent_pipe_returns_false() {
        // A name that no Windows component should ever claim. If this
        // returns `true`, either CI is running a process called
        // "sshc-doctor-nonexistent-…" (vanishingly unlikely) or the
        // function has stopped doing what it advertises.
        assert!(!windows_agent_pipe_present(
            r"\\.\pipe\sshc-doctor-no-such-pipe-1d3f9b"
        ));
    }
}

#[cfg(unix)]
fn short_path(p: &Path) -> String {
    let s = p.display().to_string();
    if s.len() > 50 {
        format!("…{}", &s[s.len() - 49..])
    } else {
        s
    }
}

/// v0.8 G3: ask GitHub if there's a newer release. *Only* fires during
/// `sshc --doctor` — every other code path leaves the network alone
/// (anti-feature 4: no always-on / background calls). One sync HTTP
/// call with a 5-second total budget.
fn check_latest_version() -> Check {
    const NAME: &str = "update";
    const URL: &str = "https://api.github.com/repos/hang-in/sshc/releases/latest";
    const RELEASES_PAGE: &str = "https://github.com/hang-in/sshc/releases/latest";
    let current = env!("CARGO_PKG_VERSION");

    if std::env::var("SSHC_NO_UPDATE_CHECK").is_ok() {
        return Check {
            name: NAME,
            status: Status::Pass,
            detail: format!("{current} (update check skipped: SSHC_NO_UPDATE_CHECK)"),
        };
    }

    // ureq 2.10's per-call `.timeout()` covers both connect and read.
    let user_agent = format!("sshc/{current}");
    let body = match ureq::get(URL)
        .set("User-Agent", &user_agent)
        .timeout(std::time::Duration::from_secs(5))
        .call()
    {
        Ok(resp) => match resp.into_string() {
            Ok(s) => s,
            Err(_) => {
                return Check {
                    name: NAME,
                    status: Status::Warn,
                    detail: "could not read GitHub response body".into(),
                };
            }
        },
        Err(_) => {
            return Check {
                name: NAME,
                status: Status::Warn,
                detail: "could not reach github (offline?)".into(),
            };
        }
    };
    let Some(tag) = extract_tag_name(&body) else {
        return Check {
            name: NAME,
            status: Status::Warn,
            detail: "unexpected response from GitHub releases".into(),
        };
    };
    let latest = tag.strip_prefix('v').unwrap_or(tag);
    match compare_versions(current, latest) {
        std::cmp::Ordering::Equal => Check {
            name: NAME,
            status: Status::Pass,
            detail: format!("{current} (latest)"),
        },
        std::cmp::Ordering::Greater => Check {
            name: NAME,
            status: Status::Pass,
            detail: format!("{current} (ahead of latest {latest})"),
        },
        std::cmp::Ordering::Less => Check {
            name: NAME,
            status: Status::Warn,
            detail: format!("{current} (latest is {latest} — see {RELEASES_PAGE})"),
        },
    }
}

/// Extract `tag_name` from a GitHub `/releases/latest` response body.
/// Treats the value as opaque text — no JSON parser. The substring
/// pattern is robust because the field appears once per response and
/// GitHub formats it consistently.
fn extract_tag_name(body: &str) -> Option<&str> {
    let key = "\"tag_name\"";
    let after_key = body.find(key)?;
    let rest = &body[after_key + key.len()..];
    let colon = rest.find(':')?;
    let after_colon = &rest[colon + 1..];
    let open = after_colon.find('"')?;
    let value_start = open + 1;
    let value_rel = &after_colon[value_start..];
    let close = value_rel.find('"')?;
    Some(&value_rel[..close])
}

/// Compare two dot-triple SemVer-ish versions (no pre-release / build
/// metadata support — sshc only ships `x.y.z` releases today). Falls
/// back to `Equal` for malformed input rather than failing the check.
fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    let parse = |s: &str| -> [u32; 3] {
        let mut out = [0u32; 3];
        for (i, part) in s.split('.').take(3).enumerate() {
            out[i] = part.parse().unwrap_or(0);
        }
        out
    };
    parse(a).cmp(&parse(b))
}

/// Run all checks, print the report, and return an `ExitCode` reflecting
/// the worst status seen (`FAIL` → `FAILURE`, anything else → `SUCCESS`).
/// v0.9 G1: detect CRLF line endings in `~/.ssh/config`. OpenSSH treats
/// `\r` as part of an alias token, so a Windows-origin config copied
/// onto macOS / Linux silently breaks every `Host …` match. Read-only
/// surface; check is omitted from the doctor output entirely when the
/// file is clean (`None`) so the green path stays uncluttered.
fn check_main_config_line_endings() -> Option<Check> {
    let path = home()?.join(".ssh").join("config");
    let content = std::fs::read_to_string(&path).ok()?;
    crlf_warning_for(&content)
}

/// Pure helper for `check_main_config_line_endings` so the CRLF
/// detection logic is unit-testable without touching disk.
fn crlf_warning_for(content: &str) -> Option<Check> {
    // Scan the first 100 logical lines. CRLF tends to be uniform across
    // a file; if it's there at all, one of the first 100 lines will
    // expose it. Bounding the scan keeps `--doctor` cheap on the
    // pathological case of a multi-megabyte config.
    let has_crlf = content.split('\n').take(100).any(|l| l.ends_with('\r'));
    if has_crlf {
        Some(Check {
            name: "line endings",
            status: Status::Warn,
            detail: "CRLF detected in ~/.ssh/config — OpenSSH treats '\\r' as part of alias \
                     tokens; convert with `tr -d '\\r' < ~/.ssh/config > ~/.ssh/config.tmp \
                     && mv ~/.ssh/config.tmp ~/.ssh/config && chmod 600 ~/.ssh/config`"
                .into(),
        })
    } else {
        None
    }
}

/// v0.9 G2: detect a sshc-managed `Include` line that ended up nested
/// inside a preceding `Host <pattern>` block. OpenSSH closes `Host` /
/// `Match` blocks only on the next such directive (or EOF) — blank
/// lines and comments do *not*. The Include then becomes conditional
/// scoped to that last alias, and every sshc-managed host looks
/// invisible to `ssh <alias>`. v0.8.4 fixed this for new injects by
/// emitting a `Match all` terminator; pre-v0.8.4 configs still need
/// surfacing.
fn check_include_scope() -> Option<Check> {
    let path = home()?.join(".ssh").join("config");
    let content = std::fs::read_to_string(&path).ok()?;
    let sshc_path = crate::storage::sshc_conf_path()?;
    nested_include_warning_for(&content, &sshc_path)
}

/// Pure detection helper — exposed for unit tests, takes content +
/// sshc.conf path verbatim so it can run without disk I/O on fixtures.
fn nested_include_warning_for(content: &str, sshc_path: &Path) -> Option<Check> {
    let include_lineno = find_sshc_include_line(content, sshc_path)?;
    let (host_lineno, header) = preceding_host_or_match(content, include_lineno)?;
    // `Match …` directives close any preceding Host block, and `Match
    // all` (or `Match host *`) applies unconditionally. `Host *` is a
    // wildcard that matches every alias, so the Include effectively
    // fires for every connection.
    let trimmed_header = header.trim();
    if trimmed_header.starts_with("Match") {
        return None;
    }
    // Treat `Host *` (with optional trailing whitespace / comments) as
    // unconditional. Anything else with a more specific pattern is
    // nested.
    let host_patterns: Vec<&str> = trimmed_header
        .strip_prefix("Host")
        .map(|s| s.split_whitespace().collect())
        .unwrap_or_default();
    if host_patterns == ["*"] {
        return None;
    }
    Some(Check {
        name: "Include scope",
        status: Status::Warn,
        detail: format!(
            "nested inside '{}' (line {}) — sshc-managed hosts only fire when that alias \
             matches. Add `Match all` directly above the Include line, or delete the \
             sshc-injected block and re-run `sshc -m` -> `i` on v0.8.4+ to re-inject \
             with the terminator.",
            trimmed_header,
            host_lineno + 1
        ),
    })
}

/// Returns the 0-based line number of the sshc-managed Include
/// directive, or None when no Include line targets `sshc_path`.
fn find_sshc_include_line(content: &str, sshc_path: &Path) -> Option<usize> {
    let target = sshc_path
        .canonicalize()
        .unwrap_or_else(|_| sshc_path.to_path_buf());
    for (lineno, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        let rest = match trimmed.strip_prefix("Include") {
            Some(r) if r.starts_with(char::is_whitespace) => r,
            _ => continue,
        };
        let path_token = rest.split_whitespace().next().unwrap_or("");
        if path_token.is_empty() {
            continue;
        }
        let resolved = expand_user_simple(path_token);
        let canonical = resolved.canonicalize().unwrap_or(resolved);
        if canonical == target {
            return Some(lineno);
        }
    }
    None
}

/// Walks backwards from `include_lineno` to find the most recent
/// `Host` or `Match` directive. Returns (line number, full trimmed
/// line). None when there's no enclosing stanza (Include lives at
/// top level — also unconditional).
fn preceding_host_or_match(content: &str, include_lineno: usize) -> Option<(usize, String)> {
    let lines: Vec<&str> = content.lines().collect();
    for lineno in (0..include_lineno).rev() {
        let trimmed = lines[lineno].trim();
        if trimmed.starts_with("Host ")
            || trimmed == "Host"
            || trimmed.starts_with("Match ")
            || trimmed == "Match"
        {
            return Some((lineno, trimmed.to_string()));
        }
    }
    None
}

/// Local `~/` expansion mirror of `include_injector::expand_user`,
/// duplicated here to keep `doctor` independent of write-side code.
fn expand_user_simple(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    } else if path == "~" {
        if let Some(home) = dirs::home_dir() {
            return home;
        }
    }
    PathBuf::from(path)
}

pub fn run() -> ExitCode {
    let mut checks: Vec<Check> = vec![
        check_ssh_config(),
        check_ssh_dir_perms(),
        check_sshc_conf(),
        check_include_line(),
    ];
    if let Some(c) = check_main_config_line_endings() {
        checks.push(c);
    }
    if let Some(c) = check_include_scope() {
        checks.push(c);
    }
    checks.extend([
        check_ssh_binary(),
        check_ssh_auth_sock(),
        check_latest_version(),
    ]);

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

#[cfg(test)]
mod line_endings_tests {
    use super::{crlf_warning_for, Status};

    #[test]
    fn lf_only_config_returns_none() {
        let content = "Host foo\n    HostName foo.example.com\n    Port 22\n";
        assert!(crlf_warning_for(content).is_none());
    }

    #[test]
    fn crlf_config_returns_warn() {
        let content = "Host foo\r\n    HostName foo.example.com\r\n    Port 22\r\n";
        let check = crlf_warning_for(content).expect("expected a Warn");
        assert!(matches!(check.status, Status::Warn));
        assert!(check.detail.contains("CRLF"));
        assert!(check.detail.contains("tr -d"));
    }

    #[test]
    fn empty_config_returns_none() {
        assert!(crlf_warning_for("").is_none());
    }

    #[test]
    fn crlf_in_first_few_lines_still_caught() {
        // Mixed CRLF/LF is still a problem because OpenSSH chokes on
        // any CR-bearing line — surface it.
        let content = "Host foo\r\n    HostName foo.example.com\n";
        assert!(crlf_warning_for(content).is_some());
    }
}

#[cfg(test)]
mod include_scope_tests {
    use super::{nested_include_warning_for, Status};
    use std::path::PathBuf;

    fn fake_sshc_path() -> PathBuf {
        // Use a string that the line-scanner can match literally —
        // canonicalize() will fail in the temp test context, so both
        // sides fall back to the as-given PathBuf, which compares
        // equal.
        PathBuf::from("/tmp/sshc-doctor-fixture/sshc.conf")
    }

    #[test]
    fn nested_under_host_pattern_warns() {
        // Common v0.4–v0.8.3 case: main config ends with a Host
        // stanza, sshc appended a bare Include — the Include is now
        // scoped to that last alias only.
        let content = "\
Host foo
    HostName foo.example.com
    Port 22

# Added by sshc; do not remove.
Include /tmp/sshc-doctor-fixture/sshc.conf
";
        let check = nested_include_warning_for(content, &fake_sshc_path())
            .expect("expected a Warn — Include is nested inside Host foo");
        assert!(matches!(check.status, Status::Warn));
        assert!(
            check.detail.contains("Host foo"),
            "detail must name the offending Host pattern, got {:?}",
            check.detail
        );
        assert!(check.detail.contains("Match all"));
    }

    #[test]
    fn match_all_terminator_clears_the_warning() {
        // v0.8.4+ inject format.
        let content = "\
Host foo
    HostName foo.example.com

# Added by sshc; do not remove.
Match all
Include /tmp/sshc-doctor-fixture/sshc.conf
";
        assert!(nested_include_warning_for(content, &fake_sshc_path()).is_none());
    }

    #[test]
    fn host_star_above_include_is_unconditional() {
        let content = "\
Host foo
    HostName foo.example.com

Host *
Include /tmp/sshc-doctor-fixture/sshc.conf
";
        assert!(nested_include_warning_for(content, &fake_sshc_path()).is_none());
    }

    #[test]
    fn no_sshc_include_returns_none() {
        let content = "Host foo\n    HostName foo.example.com\n";
        assert!(nested_include_warning_for(content, &fake_sshc_path()).is_none());
    }

    #[test]
    fn top_level_include_no_preceding_stanza_is_ok() {
        // Include at file start, before any Host stanza, is
        // unconditional by construction.
        let content = "\
# Added by sshc; do not remove.
Include /tmp/sshc-doctor-fixture/sshc.conf

Host foo
    HostName foo.example.com
";
        assert!(nested_include_warning_for(content, &fake_sshc_path()).is_none());
    }
}

#[cfg(test)]
mod update_check_tests {
    use super::{compare_versions, extract_tag_name};
    use std::cmp::Ordering;

    #[test]
    fn compare_versions_equal() {
        assert_eq!(compare_versions("0.8.0", "0.8.0"), Ordering::Equal);
    }

    #[test]
    fn compare_versions_current_behind_latest() {
        // doctor must surface this as a WARN.
        assert_eq!(compare_versions("0.7.3", "0.8.0"), Ordering::Less);
    }

    #[test]
    fn compare_versions_current_ahead_of_latest() {
        // Dev builds (`cargo install --path .` from a working tree
        // that bumped Cargo.toml past the latest tag) should not nag.
        assert_eq!(compare_versions("0.9.0", "0.8.0"), Ordering::Greater);
        assert_eq!(compare_versions("1.0.0", "0.99.99"), Ordering::Greater);
    }

    #[test]
    fn extract_tag_name_typical_github_payload() {
        // Trimmed to just the fields we care about; real responses
        // carry ~30 more keys but the substring pattern is robust.
        let body = r#"{
            "url": "https://api.github.com/repos/hang-in/sshc/releases/...",
            "tag_name": "v0.7.3",
            "name": "0.7.3 — 2026-05-20"
        }"#;
        assert_eq!(extract_tag_name(body), Some("v0.7.3"));
    }

    #[test]
    fn extract_tag_name_malformed_returns_none() {
        // No `tag_name` at all → None → check_latest_version surfaces
        // the "unexpected response" WARN.
        let body = r#"{"message": "Not Found"}"#;
        assert!(extract_tag_name(body).is_none());
    }
}
