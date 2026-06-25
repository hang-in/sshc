//! v0.9 G6: TCP reachability check. `g` key in manage mode resolves a
//! selected alias through `ssh -G` (to learn the effective hostname and
//! port) and then attempts a raw TCP connect to that endpoint with a
//! short timeout. No SSH handshake, no banner read — just "is the
//! port reachable?" The intent is to distinguish "host is down"
//! from "ssh config is wrong" without spawning ssh itself.
//!
//! Anti-feature 1 compatibility: this is `nc -z` semantics, not a
//! self-built SSH client. Reachability ≠ authentication.

use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

/// Result of a single TCP reachability probe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReachResult {
    /// `connect_timeout` returned Ok within the budget. `ms` is the
    /// wall-clock connect latency for display ("(320 ms)").
    Reachable { ms: u128 },
    /// Resolution or connect failed inside the budget. `error` carries a
    /// short, user-friendly fragment for the status bar.
    Unreachable { error: String },
}

/// Default per-call budget. Matches v0.6's `ssh -G` 5s guard pattern
/// in spirit — long enough for slow VPN handshakes, short enough that
/// a wedged remote doesn't freeze the UI thread.
pub const DEFAULT_BUDGET: Duration = Duration::from_secs(2);

/// Resolve `host:port` and attempt a TCP connect inside `budget`.
/// Resolution counts against the budget; the connect attempt itself
/// gets whatever time remains. Failure modes are flattened into a
/// single `Unreachable` arm so the status bar doesn't need to switch
/// on syscall errno.
pub fn check_tcp_reach(host: &str, port: u16, budget: Duration) -> ReachResult {
    let start = Instant::now();
    let mut addrs = match (host, port).to_socket_addrs() {
        Ok(it) => it,
        Err(e) => {
            return ReachResult::Unreachable {
                error: format!("dns: {e}"),
            };
        }
    };
    let Some(addr) = addrs.next() else {
        return ReachResult::Unreachable {
            error: "dns returned no addresses".to_string(),
        };
    };
    let elapsed = start.elapsed();
    let Some(remaining) = budget.checked_sub(elapsed) else {
        return ReachResult::Unreachable {
            error: format!("dns took >{}s", budget.as_secs()),
        };
    };
    match TcpStream::connect_timeout(&addr, remaining) {
        Ok(_) => ReachResult::Reachable {
            ms: start.elapsed().as_millis(),
        },
        Err(e) => ReachResult::Unreachable {
            error: e.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn check_tcp_reach_succeeds_against_listening_socket() {
        // Bind to an ephemeral port so the test is deterministic and
        // doesn't collide with anything the host happens to be running.
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral");
        let addr = listener.local_addr().expect("local_addr");
        let result = check_tcp_reach("127.0.0.1", addr.port(), DEFAULT_BUDGET);
        match result {
            ReachResult::Reachable { ms } => {
                assert!(ms < 1_000, "localhost connect should be <1s, got {ms} ms")
            }
            ReachResult::Unreachable { error } => panic!("expected reachable, got: {error}"),
        }
    }

    #[test]
    fn check_tcp_reach_fails_against_closed_port() {
        // RFC 6890 reserved port 1; on every platform sshc cares about
        // this returns ECONNREFUSED almost immediately.
        let result = check_tcp_reach("127.0.0.1", 1, DEFAULT_BUDGET);
        assert!(
            matches!(result, ReachResult::Unreachable { .. }),
            "expected Unreachable, got {result:?}"
        );
    }

    #[test]
    fn check_tcp_reach_returns_unreachable_on_dns_failure() {
        let result = check_tcp_reach(
            "this-host-should-not-resolve.sshc-test-only.invalid",
            22,
            DEFAULT_BUDGET,
        );
        assert!(matches!(result, ReachResult::Unreachable { .. }));
    }
}
