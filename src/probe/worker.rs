use std::net::{TcpStream, ToSocketAddrs};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use crate::probe::state::{ProbeJob, ProbeState, ProbeUpdate};

pub(crate) fn run_worker(
    job_rx: Arc<Mutex<mpsc::Receiver<Option<ProbeJob>>>>,
    result_tx: mpsc::Sender<ProbeUpdate>,
) {
    loop {
        let job = {
            let lock = match job_rx.lock() {
                Ok(g) => g,
                Err(_) => break,
            };
            match lock.recv() {
                Ok(j) => j,
                Err(_) => break,
            }
        };
        match job {
            Some(job) => {
                let state = probe_once(&job.target_host, job.port);
                let update = ProbeUpdate {
                    host_idx: job.host_idx,
                    state,
                    generation: job.generation,
                };
                if result_tx.send(update).is_err() {
                    break;
                }
            }
            None => break,
        }
    }
}

pub(crate) fn probe_once(target_host: &str, port: u16) -> ProbeState {
    let addr_str = format!("{}:{}", target_host, port);
    let addr = match addr_str.to_socket_addrs() {
        Ok(mut it) => match it.next() {
            Some(a) => a,
            None => return ProbeState::Failed,
        },
        Err(_) => return ProbeState::Failed,
    };
    match TcpStream::connect_timeout(&addr, Duration::from_secs(2)) {
        Ok(_) => ProbeState::Open,
        Err(_) => ProbeState::Failed,
    }
}
