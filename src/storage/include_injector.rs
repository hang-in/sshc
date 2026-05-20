use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::error::StorageError;

/// Returns true if `main_config` already contains an Include directive whose
/// resolved path canonicalizes to `sshc_conf_path`.
pub fn is_include_present(main_config: &Path, sshc_conf_path: &Path) -> Result<bool, StorageError> {
    let content = fs::read_to_string(main_config).map_err(StorageError::ReadFailed)?;
    let target = sshc_conf_path
        .canonicalize()
        .unwrap_or_else(|_| sshc_conf_path.to_path_buf());

    for line in content.lines() {
        let trimmed = line.trim();
        let rest = match trimmed.strip_prefix("Include") {
            Some(r) if r.starts_with(char::is_whitespace) => r,
            _ => continue,
        };
        let path_token = rest.split_whitespace().next().unwrap_or("");
        if path_token.is_empty() {
            continue;
        }
        let resolved = expand_user(path_token);
        let canonical = resolved.canonicalize().unwrap_or(resolved);
        if canonical == target {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Append a sshc-managed Include line to the end of `main_config`. No-op if
/// already present. Creates a dated `.bak.sshc-YYYYMMDD` before mutating.
///
/// Returns `Ok(true)` when a new Include line was added, `Ok(false)` when
/// it was already present (no file mutation, no backup). Callers use the
/// outcome to render the right status message — "Include added …" vs
/// "Include already present …" — which matters when 'i' is pressed in
/// manage mode where the user can't tell if writes were already enabled.
pub fn inject_include(main_config: &Path, sshc_conf_path: &Path) -> Result<bool, StorageError> {
    if is_include_present(main_config, sshc_conf_path)? {
        return Ok(false);
    }
    let backup_path = backup_path_for(main_config);
    fs::copy(main_config, &backup_path).map_err(StorageError::BackupFailed)?;

    let include_value = preferred_include_form(sshc_conf_path);
    let mut file = OpenOptions::new()
        .append(true)
        .open(main_config)
        .map_err(StorageError::WriteFailed)?;
    writeln!(file, "\n# Added by sshc; do not remove.").map_err(StorageError::WriteFailed)?;
    writeln!(file, "Include {}", include_value).map_err(StorageError::WriteFailed)?;
    Ok(true)
}

fn expand_user(path: &str) -> PathBuf {
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

fn preferred_include_form(sshc_conf: &Path) -> String {
    if let Some(home) = dirs::home_dir() {
        let default = home.join(".ssh").join("config.d").join("sshc.conf");
        let conf_canon = sshc_conf.canonicalize().ok();
        let default_canon = default.canonicalize().ok();
        if sshc_conf == default || (conf_canon.is_some() && conf_canon == default_canon) {
            return "~/.ssh/config.d/sshc.conf".to_string();
        }
    }
    sshc_conf.display().to_string()
}

fn backup_path_for(main_config: &Path) -> PathBuf {
    let date = current_date_ymd();
    let stem = main_config
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "config".to_string());
    let parent = main_config.parent().unwrap_or_else(|| Path::new("."));
    parent.join(format!("{}.bak.sshc-{}", stem, date))
}

/// Today's date as YYYYMMDD in UTC, computed from std::time without external deps.
fn current_date_ymd() -> String {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = (now / 86_400) as i64;
    let (y, m, d) = days_to_ymd(days);
    format!("{:04}{:02}{:02}", y, m, d)
}

/// Convert days-since-1970-01-01 (UTC) to (year, month, day) via Howard Hinnant.
fn days_to_ymd(days: i64) -> (i32, u32, u32) {
    let days = days + 719468;
    let era = if days >= 0 { days } else { days - 146096 } / 146097;
    let doe = (days - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y_final = if m <= 2 { y + 1 } else { y };
    (y_final as i32, m as u32, d as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::prelude::*;

    #[test]
    fn test_is_include_present_when_absent() {
        let temp = assert_fs::TempDir::new().unwrap();
        let main = temp.child("config");
        main.write_str("Host *\n    Port 22\n").unwrap();
        let sshc_conf_handle = temp.child("sshc.conf");
        sshc_conf_handle.touch().unwrap();
        assert!(!is_include_present(main.path(), sshc_conf_handle.path()).unwrap());
    }

    #[test]
    fn test_is_include_present_when_present() {
        let temp = assert_fs::TempDir::new().unwrap();
        let main = temp.child("config");
        let sshc_conf_handle = temp.child("sshc.conf");
        sshc_conf_handle.touch().unwrap();
        let abs = sshc_conf_handle.path().canonicalize().unwrap();
        main.write_str(&format!("Include {}\n", abs.display()))
            .unwrap();
        assert!(is_include_present(main.path(), sshc_conf_handle.path()).unwrap());
    }

    #[test]
    fn test_inject_idempotent() {
        let temp = assert_fs::TempDir::new().unwrap();
        let main = temp.child("config");
        main.write_str("Host *\n").unwrap();
        let sshc_conf_handle = temp.child("sshc.conf");
        sshc_conf_handle.touch().unwrap();

        inject_include(main.path(), sshc_conf_handle.path()).unwrap();
        inject_include(main.path(), sshc_conf_handle.path()).unwrap();

        let content = fs::read_to_string(main.path()).unwrap();
        let count = content
            .lines()
            .filter(|l| l.trim_start().starts_with("Include"))
            .count();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_days_to_ymd_known_dates() {
        // 1970-01-01
        assert_eq!(days_to_ymd(0), (1970, 1, 1));
        // 2000-01-01 (30 years from 1970, 7 leap years: 72,76,80,84,88,92,96)
        // = 30*365 + 7 = 10957
        assert_eq!(days_to_ymd(10957), (2000, 1, 1));
        // 2024-01-01: 54 years from 1970, leap years 72,76,...,2020 = 13 (every 4 except 100 not 400, but no 100-multiples in range)
        // 54*365 + 13 = 19723
        assert_eq!(days_to_ymd(19723), (2024, 1, 1));
    }
}
