use crate::error::SetupError;
use std::path::Path;

#[cfg(unix)]
use crate::error::StorageError;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

/// Ensure file/dir at `path` has the given mode (octal, e.g. 0o600). Reads
/// metadata; if the mode already matches, returns Ok. Otherwise applies the
/// requested mode via `set_permissions`. Re-verifies and reports
/// `PermissionMismatch` if the filesystem refused the change.
///
/// On Windows this is a no-op — Windows ACLs aren't the same model as Unix
/// permission bits, and v0.7 explicitly defers ACL enforcement. The function
/// still returns `Ok(())` so callers don't need their own cfg branches.
#[cfg(unix)]
pub fn ensure_file_mode(path: &Path, mode: u32) -> Result<(), SetupError> {
    let meta =
        std::fs::metadata(path).map_err(|e| SetupError::Storage(StorageError::ReadFailed(e)))?;
    let current = meta.permissions().mode() & 0o777;
    if current == mode {
        return Ok(());
    }

    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
        .map_err(|e| SetupError::Storage(StorageError::WriteFailed(e)))?;

    let final_meta =
        std::fs::metadata(path).map_err(|e| SetupError::Storage(StorageError::ReadFailed(e)))?;
    let final_mode = final_meta.permissions().mode() & 0o777;
    if final_mode != mode {
        return Err(SetupError::Storage(StorageError::PermissionMismatch {
            path: path.to_path_buf(),
            expected: mode,
            actual: final_mode,
        }));
    }
    Ok(())
}

#[cfg(not(unix))]
pub fn ensure_file_mode(_path: &Path, _mode: u32) -> Result<(), SetupError> {
    // Windows: Unix permission bits don't apply. ACLs would be the
    // analog, but enforcing "owner-only" via ACLs across Windows
    // versions is out of scope for v0.7.
    Ok(())
}

/// Convenience alias for directories. Same semantics as `ensure_file_mode`.
pub fn ensure_dir_mode(path: &Path, mode: u32) -> Result<(), SetupError> {
    ensure_file_mode(path, mode)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::fs::File;

    fn unique_temp_path(label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "sshc_perm_{}_{}_{}.tmp",
            label,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ))
    }

    #[test]
    fn test_changes_mode_when_different() {
        let path = unique_temp_path("change");
        File::create(&path).expect("create temp file");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644))
            .expect("seed perms");

        ensure_file_mode(&path, 0o600).expect("ensure mode");
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_noop_when_already_correct() {
        let path = unique_temp_path("noop");
        File::create(&path).expect("create temp file");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
            .expect("seed perms");

        ensure_file_mode(&path, 0o600).expect("ensure mode");
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);

        let _ = std::fs::remove_file(&path);
    }
}
