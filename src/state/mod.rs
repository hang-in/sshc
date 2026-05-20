use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

pub mod schema;
pub use schema::{MemorySection, RecentEntry, SetupSection, State, CURRENT_VERSION, RECENT_MAX};

use crate::error::{SetupError, StorageError};

/// Convert a file's last-modified time into Unix-epoch seconds, or 0
/// when the timestamp can't be read.
fn file_mtime_secs(path: &Path) -> u64 {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Resolve the state.toml path: $XDG_CONFIG_HOME/sshc/state.toml,
/// fallback ~/.config/sshc/state.toml. Returns None if no home dir.
pub fn state_path() -> Option<PathBuf> {
    let base = std::env::var("XDG_CONFIG_HOME")
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|h| h.join(".config")));

    base.map(|b| b.join("sshc").join("state.toml"))
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

    let mut state: State =
        toml::from_str(&contents).map_err(|e| SetupError::StateParseFailed(e.to_string()))?;
    if state.version != CURRENT_VERSION {
        return Err(SetupError::StateParseFailed(format!(
            "unsupported schema version: {}",
            state.version
        )));
    }
    migrate_legacy_recent(&mut state, file_mtime_secs(path));
    Ok(state)
}

impl State {
    /// Record a connection to `alias`: bump it to the front of
    /// `memory.recent`, dedupe, truncate at `RECENT_MAX`, and keep the
    /// legacy `last_connected_alias` field in sync for one release.
    /// Timestamp uses the current Unix epoch in seconds (0 on clock-
    /// resolution failure, which sorts last but never panics).
    pub fn record_recent(&mut self, alias: &str) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .map(|d| d.as_secs())
            .unwrap_or(0);
        self.memory.recent.retain(|e| e.alias != alias);
        self.memory.recent.insert(
            0,
            RecentEntry {
                alias: alias.to_string(),
                ts: now,
            },
        );
        if self.memory.recent.len() > RECENT_MAX {
            self.memory.recent.truncate(RECENT_MAX);
        }
        self.memory.last_connected_alias = Some(alias.to_string());
    }
}

/// If `memory.recent` is empty but the pre-v0.6 `last_connected_alias`
/// is present, seed `recent` with a single entry. Uses `file_mtime` (not
/// `0`) so the migrated entry sorts above absent history rather than
/// sinking to the bottom on the first v0.6 open.
fn migrate_legacy_recent(state: &mut State, file_mtime: u64) {
    if state.memory.recent.is_empty() {
        if let Some(alias) = state.memory.last_connected_alias.clone() {
            state.memory.recent.push(RecentEntry {
                alias,
                ts: file_mtime,
            });
        }
    }
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
        // Use the v0.6 fields. last_connected_alias would trigger the
        // legacy migration and round-trip wouldn't be an identity.
        state.memory.recent.push(RecentEntry {
            alias: "test-host".to_string(),
            ts: 1_700_000_000,
        });
        state.memory.favorites.push("test-host".to_string());
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
    fn test_load_migrates_legacy_last_connected_alias_into_recent() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("legacy.toml");
        // v0.5 schema: only last_connected_alias, no `recent` / `favorites`.
        fs::write(
            &path,
            "version = 1\n\n[setup]\n[memory]\nlast_connected_alias = \"prod-db\"\n",
        )
        .unwrap();

        let loaded = load_from(&path).unwrap();
        assert_eq!(loaded.memory.recent.len(), 1);
        assert_eq!(loaded.memory.recent[0].alias, "prod-db");
        // ts should be the file's mtime (non-zero on a freshly-written file).
        assert!(loaded.memory.recent[0].ts > 0);
        // Legacy field is still read so subsequent passes see the same
        // pointer; we just don't migrate twice.
        assert_eq!(
            loaded.memory.last_connected_alias,
            Some("prod-db".to_string())
        );
        assert!(loaded.memory.favorites.is_empty());
    }

    #[test]
    fn test_load_skips_migration_when_recent_already_populated() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("dual.toml");
        // Crafted file: both legacy + new fields present. Migration must
        // not double-insert.
        fs::write(
            &path,
            "version = 1\n\n[setup]\n[memory]\nlast_connected_alias = \"old\"\n\
             [[memory.recent]]\nalias = \"new\"\nts = 1700000000\n",
        )
        .unwrap();

        let loaded = load_from(&path).unwrap();
        assert_eq!(loaded.memory.recent.len(), 1);
        assert_eq!(loaded.memory.recent[0].alias, "new");
    }

    #[test]
    fn test_state_v05_fixture_loads() {
        // The shared fixture lives under tests/fixtures/ for cross-crate use.
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/state_v05.toml");
        let loaded = load_from(&fixture).unwrap();
        assert_eq!(loaded.memory.recent.len(), 1);
        assert_eq!(loaded.memory.recent[0].alias, "prod-db");
        assert!(loaded.setup.include_check_done);
    }

    #[test]
    fn test_record_recent_inserts_at_front() {
        let mut s = State::default();
        s.record_recent("alpha");
        s.record_recent("beta");
        assert_eq!(s.memory.recent.len(), 2);
        assert_eq!(s.memory.recent[0].alias, "beta");
        assert_eq!(s.memory.recent[1].alias, "alpha");
        assert_eq!(s.memory.last_connected_alias.as_deref(), Some("beta"));
    }

    #[test]
    fn test_record_recent_dedupes() {
        let mut s = State::default();
        s.record_recent("alpha");
        s.record_recent("beta");
        s.record_recent("alpha");
        // "alpha" moved to front; "beta" still present once.
        assert_eq!(s.memory.recent.len(), 2);
        assert_eq!(s.memory.recent[0].alias, "alpha");
        assert_eq!(s.memory.recent[1].alias, "beta");
    }

    #[test]
    fn test_record_recent_truncates_at_max() {
        let mut s = State::default();
        for i in 0..(RECENT_MAX + 5) {
            s.record_recent(&format!("host-{i}"));
        }
        assert_eq!(s.memory.recent.len(), RECENT_MAX);
        // Most-recent first: last pushed is at index 0.
        assert_eq!(s.memory.recent[0].alias, format!("host-{}", RECENT_MAX + 4));
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
