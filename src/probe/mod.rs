pub mod state;
pub mod worker;

pub use state::{ProbeJob, ProbeState, ProbeUpdate};

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

use crate::config::model::Host;

pub struct ProbePool {
    generation: Arc<AtomicU64>,
    job_tx: mpsc::Sender<Option<ProbeJob>>,
    result_tx: mpsc::Sender<ProbeUpdate>,
    result_rx: Mutex<mpsc::Receiver<ProbeUpdate>>,
    worker_handles: Vec<thread::JoinHandle<()>>,
}

impl ProbePool {
    /// Spawn `min(8, host_count.max(1))` worker threads and run a first
    /// probe round against `initial`.
    pub fn start(initial: &[Host]) -> Self {
        let worker_count = std::cmp::min(8, initial.len().max(1));

        let (job_tx, job_rx) = mpsc::channel::<Option<ProbeJob>>();
        let (result_tx, result_rx) = mpsc::channel::<ProbeUpdate>();

        let shared_job_rx = Arc::new(Mutex::new(job_rx));
        let mut worker_handles = Vec::with_capacity(worker_count);
        for _ in 0..worker_count {
            let rx = Arc::clone(&shared_job_rx);
            let tx = result_tx.clone();
            worker_handles.push(thread::spawn(move || {
                worker::run_worker(rx, tx);
            }));
        }

        let pool = Self {
            generation: Arc::new(AtomicU64::new(0)),
            job_tx,
            result_tx,
            result_rx: Mutex::new(result_rx),
            worker_handles,
        };
        if !initial.is_empty() {
            pool.refresh(initial);
        }
        pool
    }

    /// Increment generation and dispatch one ProbeJob per host. Emits an
    /// `InFlight` ProbeUpdate for each host BEFORE dispatching jobs so
    /// the UI can show the in-flight state immediately.
    pub fn refresh(&self, hosts: &[Host]) {
        let gen = self.generation.fetch_add(1, Ordering::SeqCst) + 1;

        for (idx, host) in hosts.iter().enumerate() {
            let target_host = host
                .hostname
                .as_ref()
                .cloned()
                .unwrap_or_else(|| host.alias.clone());
            let port = host.port.unwrap_or(22);

            let _ = self.result_tx.send(ProbeUpdate {
                host_idx: idx,
                state: ProbeState::InFlight,
                generation: gen,
            });

            let _ = self.job_tx.send(Some(ProbeJob {
                host_idx: idx,
                target_host,
                port,
                generation: gen,
            }));
        }
    }

    /// Non-blocking drain of pending probe updates.
    pub fn poll_updates(&self) -> Vec<ProbeUpdate> {
        let mut updates = Vec::new();
        if let Ok(rx) = self.result_rx.lock() {
            while let Ok(update) = rx.try_recv() {
                updates.push(update);
            }
        }
        updates
    }

    pub fn current_generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }
}

impl Drop for ProbePool {
    fn drop(&mut self) {
        // Signal each worker to exit.
        for _ in 0..self.worker_handles.len() {
            let _ = self.job_tx.send(None);
        }
        // Workers exit promptly because they are blocked on recv().
        // Sequential join is acceptable: each worker is already on its way out.
        for handle in self.worker_handles.drain(..) {
            let _ = handle.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::path::PathBuf;

    fn mock_host(alias: &str) -> Host {
        Host {
            alias: alias.to_string(),
            hostname: None,
            user: None,
            port: None,
            identity_file: None,
            line_start: 0,
            source_file: PathBuf::new(),
            tags: vec![],
            extra: Vec::new(),
            local_forward: None,
            remote_forward: None,
            dynamic_forward: None,
        }
    }

    #[test]
    fn test_probe_open_to_local_listener() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        assert_eq!(worker::probe_once("127.0.0.1", port), ProbeState::Open);
    }

    #[test]
    #[ignore]
    fn test_probe_failed_to_unreachable() {
        assert_eq!(worker::probe_once("192.0.2.1", 22), ProbeState::Failed);
    }

    #[test]
    fn test_probe_failed_to_unresolvable() {
        assert_eq!(
            worker::probe_once("this-host-does-not-exist.invalid", 22),
            ProbeState::Failed
        );
    }

    #[test]
    fn test_pool_start_emits_inflight() {
        let hosts = vec![mock_host("test")];
        let pool = ProbePool::start(&hosts);
        // Allow workers a moment to start; poll a few times.
        let mut found_inflight = false;
        for _ in 0..10 {
            let updates = pool.poll_updates();
            if updates.iter().any(|u| u.state == ProbeState::InFlight) {
                found_inflight = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(found_inflight, "expected at least one InFlight update");
    }

    #[test]
    fn test_pool_generation_increments() {
        let hosts = vec![mock_host("test")];
        let pool = ProbePool::start(&hosts);
        let gen0 = pool.current_generation();
        pool.refresh(&hosts);
        assert_eq!(pool.current_generation(), gen0 + 1);
    }
}
