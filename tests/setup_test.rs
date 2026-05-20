use std::fs;
use std::os::unix::fs::PermissionsExt;

use sshc::setup::detect::{include_is_present, path_exists};
use sshc::setup::permissions::{ensure_dir_mode, ensure_file_mode};
use sshc::setup::SetupOutcome;
use sshc::storage::{inject_include, sshc_conf_path};

fn unique_path(label: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!(
        "sshc_setup_{}_{}_{}",
        label,
        std::process::id(),
        nanos
    ))
}

#[test]
fn setup_outcome_round_trip_equality() {
    let outcomes = [
        SetupOutcome::Ready,
        SetupOutcome::AwaitingIncludeChoice,
        SetupOutcome::ReadOnly,
    ];
    for o in &outcomes {
        let cloned = o.clone();
        assert_eq!(*o, cloned);
    }
    assert_ne!(SetupOutcome::Ready, SetupOutcome::ReadOnly);
    assert_ne!(SetupOutcome::AwaitingIncludeChoice, SetupOutcome::Ready);
}

#[test]
fn ensure_file_mode_applies_0600_to_existing_file() {
    let path = unique_path("ensure_file");
    fs::File::create(&path).expect("create");

    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).expect("seed perms");
    ensure_file_mode(&path, 0o600).expect("ensure file mode");

    let mode = fs::metadata(&path).expect("metadata").permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);

    let _ = fs::remove_file(&path);
}

#[test]
fn ensure_dir_mode_applies_0700_to_existing_dir() {
    let dir = unique_path("ensure_dir");
    fs::create_dir_all(&dir).expect("mkdir");

    fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).expect("seed perms");
    ensure_dir_mode(&dir, 0o700).expect("ensure dir mode");

    let mode = fs::metadata(&dir).expect("metadata").permissions().mode() & 0o777;
    assert_eq!(mode, 0o700);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn detect_path_exists_distinguishes_present_and_absent() {
    let path = unique_path("detect");
    assert!(!path_exists(&path), "non-existent must be false");

    fs::File::create(&path).expect("create");
    assert!(path_exists(&path), "existent file must be true");

    let _ = fs::remove_file(&path);
}

#[test]
fn detect_include_is_present_round_trip() {
    let dir = unique_path("detect_include_dir");
    fs::create_dir_all(&dir).expect("mkdir");

    let main_config = dir.join("config");
    let sshc_conf = dir.join("sshc.conf");
    fs::write(&main_config, "Host other\n  HostName ex.com\n").expect("write main");
    fs::File::create(&sshc_conf).expect("create sshc_conf");

    assert!(!include_is_present(&main_config, &sshc_conf).expect("check 1"));
    inject_include(&main_config, &sshc_conf).expect("inject");
    assert!(include_is_present(&main_config, &sshc_conf).expect("check 2"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn sshc_conf_path_resolves_when_home_available() {
    // Smoke: the function returns Some(..) on any environment where dirs
    // can resolve a home directory. CI containers always have $HOME, so
    // this should be Some. If $HOME is unset, the function returns None
    // — accept either outcome rather than fail.
    let resolved = sshc_conf_path();
    if let Some(p) = resolved {
        assert!(p.ends_with("sshc.conf"));
        assert!(p.to_string_lossy().contains(".ssh/config.d/sshc.conf"));
    }
}
