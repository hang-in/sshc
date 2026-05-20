use crate::error::SetupError;
use std::path::Path;

/// True if `path` exists and is a regular file.
pub fn path_exists(path: &Path) -> bool {
    path.exists() && path.is_file()
}

/// Convenience wrapper around `crate::storage::is_include_present` that
/// converts `StorageError` into `SetupError::Storage`.
pub fn include_is_present(main_config: &Path, sshc_conf: &Path) -> Result<bool, SetupError> {
    crate::storage::is_include_present(main_config, sshc_conf).map_err(SetupError::Storage)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;

    #[test]
    fn test_path_exists() {
        let temp_dir = std::env::temp_dir();
        let file_path = temp_dir.join(format!(
            "sshc_detect_test_{}_{}.tmp",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));

        assert!(!path_exists(&file_path));
        File::create(&file_path).expect("create temp file");
        assert!(path_exists(&file_path));
        let _ = std::fs::remove_file(&file_path);
    }
}
