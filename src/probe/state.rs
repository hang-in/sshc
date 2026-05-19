#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeState {
    Open,
    Failed,
    InFlight,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct ProbeJob {
    pub host_idx: usize,
    pub target_host: String,
    pub port: u16,
    pub generation: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct ProbeUpdate {
    pub host_idx: usize,
    pub state: ProbeState,
    pub generation: u64,
}
