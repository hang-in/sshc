use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

pub mod schema;
pub use schema::{MemorySection, SetupSection, State, CURRENT_VERSION};

use crate::error::{SetupError, StorageError};

/// Resolve the state.toml path: $XDG_CONFIG_HOME/sshs/state.toml,
/// fallback ~/.config/sshs/state.toml. Returns None if no home dir.
pub fn state_path() -> Option<PathBuf> {
    let base = std::env::var("XDG_CONFIG_HOME")
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|h| h.join(".config")));

    base.map(|b| b.join("sshs").join("state.toml"))
}

/// Load state.toml. Returns State::default() if the file does not exist
/// or if no state_path can be resolved. Returns Err on parse or version error.
pub fn load() -> Result<State, SetupError> {
    match state_path() {
        Some(p) => load_from(&p),
        None => Ok(State::default()),
    }
}

/// Save state.toml atomically with 0600 permissions. Creates parent dir if missing.
pub fn save(state: &State) -> Result<(), SetupError> {
    let path = state_path().ok_or(SetupError::HomeDirMissing)?;
    save_to(&path, state)
}

fn load_from(path: &Path) -> Result<State, SetupError> {
    if !path.exists() {
        return Ok(State::default());
    }
    let mut file =
        File::open(path).map_err(|e| SetupError::Storage(StorageError::ReadFailed(e)))?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .map_err(|e| SetupError::Storage(StorageError::ReadFailed(e)))?;

    let state: State =
        toml::from_str(&contents).map_err(|e| SetupError::StateParseFailed(e.to_string()))?;
    if state.version != CURRENT_VERSION {
        return Err(SetupError::StateParseFailed(format!(
            "unsupported schema version: {}",
            state.version
        )));
    }
    Ok(state)
}

fn save_to(path: &Path, state: &State) -> Result<(), SetupError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(SetupError::MkdirFailed)?;
    }

    let content =
        toml::to_string_pretty(state).map_err(|e| SetupError::StateParseFailed(e.to_string()))?;
    let tmp_path = path.with_extension(format!("tmp.{}", process::id()));

    {
        let mut file = File::create(&tmp_path)
            .map_err(|e| SetupError::StateWriteFailed(StorageError::WriteFailed(e)))?;
        file.write_all(content.as_bytes())
            .map_err(|e| SetupError::StateWriteFailed(StorageError::WriteFailed(e)))?;
        file.sync_all()
            .map_err(|e| SetupError::StateWriteFailed(StorageError::WriteFailed(e)))?;
    }

    #[cfg(unix)]
    {
        let mut perms = fs::metadata(&tmp_path)
            .map_err(|e| SetupError::StateWriteFailed(StorageError::WriteFailed(e)))?
            .permissions();
        perms.set_mode(0o600);
        fs::set_permissions(&tmp_path, perms)
            .map_err(|e| SetupError::StateWriteFailed(StorageError::WriteFailed(e)))?;
    }

    fs::rename(&tmp_path, path)
        .map_err(|e| SetupError::StateWriteFailed(StorageError::RenameFailed(e)))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::TempDir;

    #[test]
    fn test_default_state() {
        let state = State::default();
        assert_eq!(state.version, CURRENT_VERSION);
        assert!(!state.setup.declined_include_injection);
        assert!(!state.setup.include_check_done);
        assert_eq!(state.memory.last_connected_alias, None);
    }

    #[test]
    fn test_save_then_load_roundtrip() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("nested/state.toml");
        let mut state = State::default();
        state.memory.last_connected_alias = Some("test-host".to_string());
        state.setup.declined_include_injection = true;

        save_to(&path, &state).unwrap();
        let loaded = load_from(&path).unwrap();
        assert_eq!(state, loaded);
    }

    #[test]
    fn test_load_nonexistent_returns_default() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("nonexistent.toml");
        let loaded = load_from(&path).unwrap();
        assert_eq!(loaded, State::default());
    }

    #[test]
    fn test_load_bad_version() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("bad_version.toml");
        fs::write(&path, "version = 99\n\n[setup]\n[memory]\n").unwrap();

        let result = load_from(&path);
        match result {
            Err(SetupError::StateParseFailed(msg)) => {
                assert!(msg.contains("unsupported schema version"));
            }
            other => panic!("Expected StateParseFailed, got {:?}", other),
        }
    }
}
