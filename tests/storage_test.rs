use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::io::AsRawFd;

#[allow(deprecated)]
use nix::fcntl::{flock, FlockArg};

use sshc::error::StorageError;
use sshc::storage::{inject_include, is_include_present, with_locked_write};

fn unique_path(label: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("sshc_{}_{}_{}", label, std::process::id(), nanos))
}

#[test]
fn test_atomic_write_round_trip() {
    let path = unique_path("storage_atomic");
    let content = "hello\n";

    with_locked_write(&path, true, |_| content.to_string()).expect("write");

    let read_back = fs::read_to_string(&path).expect("read");
    assert_eq!(read_back, content);

    let mode = fs::metadata(&path).expect("metadata").permissions().mode() & 0o777;
    assert_eq!(mode, 0o600, "expected 0600 permissions on atomic write");

    let _ = fs::remove_file(&path);
}

#[test]
fn test_with_locked_write_returns_lock_held_by_other() {
    let path = unique_path("lock_contention");

    fs::write(&path, "initial content").expect("setup write");

    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .expect("open for lock");

    #[allow(deprecated)]
    flock(file.as_raw_fd(), FlockArg::LockExclusiveNonblock).expect("manual flock");

    let result = with_locked_write(&path, false, |_| String::new());

    assert!(result.is_err(), "expected Err, got {:?}", result.is_ok());
    let err = result.unwrap_err();
    assert!(
        matches!(err, StorageError::LockHeldByOther),
        "expected LockHeldByOther, got {err:?}"
    );

    drop(file);
    let _ = fs::remove_file(&path);
}

#[test]
fn test_inject_include_idempotent() {
    let temp_dir = unique_path("inject_dir");
    fs::create_dir_all(&temp_dir).expect("mkdir");

    let main_config = temp_dir.join("config");
    let sshc_conf = temp_dir.join("sshc.conf");

    fs::write(&main_config, "Host other\n  HostName ex.com\n").expect("write main");
    fs::File::create(&sshc_conf).expect("create sshc_conf");

    inject_include(&main_config, &sshc_conf).expect("inject 1");

    assert!(
        is_include_present(&main_config, &sshc_conf).expect("check present"),
        "Include must be present after first inject"
    );

    let len_after_first = fs::read_to_string(&main_config).expect("read 1").len();

    inject_include(&main_config, &sshc_conf).expect("inject 2 (idempotent)");

    let len_after_second = fs::read_to_string(&main_config).expect("read 2").len();
    assert_eq!(
        len_after_second, len_after_first,
        "second inject_include must be no-op"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}
