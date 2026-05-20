use std::net::TcpListener;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use sshc::config::model::Host;
use sshc::probe::{ProbePool, ProbeState};

fn make_host(hostname: &str, port: u16) -> Host {
    Host {
        alias: "test".to_string(),
        hostname: Some(hostname.to_string()),
        user: None,
        port: Some(port),
        identity_file: None,
        line_start: 1,
        source_file: PathBuf::from("/dev/null"),
        tags: Vec::new(),
        extra: Vec::new(),
    }
}

/// Poll `pool` until host_idx settles to a non-InFlight/non-Unknown state
/// or `timeout` elapses. Returns the settled state or None.
fn wait_for_settled(pool: &ProbePool, host_idx: usize, timeout: Duration) -> Option<ProbeState> {
    let deadline = Instant::now() + timeout;
    let mut latest: Option<ProbeState> = None;
    while Instant::now() < deadline {
        for u in pool.poll_updates() {
            if u.host_idx == host_idx
                && !matches!(u.state, ProbeState::InFlight | ProbeState::Unknown)
            {
                latest = Some(u.state);
            }
        }
        if latest.is_some() {
            return latest;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    latest
}

#[test]
fn test_probe_bound_listener_reports_open() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("local_addr").port();
    let host = make_host("127.0.0.1", port);

    let pool = ProbePool::start(&[host]);
    let result = wait_for_settled(&pool, 0, Duration::from_secs(2));

    assert_eq!(result, Some(ProbeState::Open));

    drop(listener);
    drop(pool);
}

#[test]
fn test_probe_unreachable_reports_failed() {
    // 192.0.2.0/24 is TEST-NET-1; guaranteed unroutable in production networks.
    let host = make_host("192.0.2.1", 22);
    let pool = ProbePool::start(&[host]);
    let result = wait_for_settled(&pool, 0, Duration::from_secs(5));

    assert_eq!(result, Some(ProbeState::Failed));

    drop(pool);
}
