//! Round-trip ssh integration tests.
//!
//! Use POSIX shell scripts in tests/fixtures/* as mock ssh binaries.
//! The tests pass each script's path as the `ssh_binary` argument to
//! ssh_run, so we exercise the full spawn+wait+classify pipeline
//! without depending on env::set_var (which is thread-unsafe under
//! cargo test).

use std::path::PathBuf;

use sshc::error::SshError;
use sshc::exec::ssh::{ssh_run, SshResult};

fn fixture(name: &str) -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests/fixtures");
    p.push(name);
    p
}

#[test]
#[cfg(unix)]
fn test_round_trip_exit_0() {
    let bin = fixture("mock_ssh_exit_0.sh");
    let result = ssh_run("dummy", bin.to_str().unwrap()).expect("ssh_run should succeed");
    assert_eq!(result, SshResult::Success);
}

#[test]
#[cfg(unix)]
fn test_round_trip_exit_130() {
    let bin = fixture("mock_ssh_exit_130.sh");
    let result = ssh_run("dummy", bin.to_str().unwrap()).expect("ssh_run should succeed");
    assert_eq!(result, SshResult::Interrupted);
}

#[test]
#[cfg(unix)]
fn test_round_trip_exit_255() {
    let bin = fixture("mock_ssh_exit_255.sh");
    let result = ssh_run("dummy", bin.to_str().unwrap()).expect("ssh_run should succeed");
    assert_eq!(result, SshResult::ConnectFailed(255));
}

#[test]
#[cfg(unix)]
fn test_round_trip_exit_signal() {
    let bin = fixture("mock_ssh_signal.sh");
    let result = ssh_run("dummy", bin.to_str().unwrap()).expect("ssh_run should succeed");
    // The mock self-kills with SIGSEGV (11). Result should be Crashed(11).
    assert_eq!(result, SshResult::Crashed(11));
}

#[test]
fn test_round_trip_launch_failed() {
    let bin = "/nonexistent/path/to/mock_ssh";
    let err = ssh_run("dummy", bin).expect_err("ssh_run should error");
    match err {
        SshError::LaunchFailed(_) => {}
        SshError::WaitFailed(_) => panic!("Expected LaunchFailed, got WaitFailed"),
    }
}
