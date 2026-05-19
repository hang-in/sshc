use std::fmt;
use std::path::PathBuf;

/// Represents a single SSH host entry parsed from ~/.ssh/config.
#[derive(Debug, Clone)]
pub struct Host {
    pub alias: String,
    pub hostname: Option<String>,
    pub user: Option<String>,
    pub port: Option<u16>,
    pub identity_file: Option<PathBuf>,
    pub line_start: usize,
    pub source_file: PathBuf,
}

impl fmt::Display for Host {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let alias = &self.alias;
        let hostname = self.hostname.as_deref().unwrap_or("<no hostname>");
        let user = self
            .user
            .as_deref()
            .map(|u| format!("{}@", u))
            .unwrap_or_default();
        let port = self.port.map(|p| format!(":{}", p)).unwrap_or_default();
        write!(f, "{:<20} {}{}{}", alias, user, hostname, port)
    }
}

impl Host {
    /// Returns true if the alias or hostname contains the query (case-insensitive).
    /// For MVP with <500 hosts, inline substring matching is sufficient.
    pub fn fuzzy_match(&self, query: &str) -> bool {
        if query.is_empty() {
            return true;
        }
        let query = query.to_lowercase();
        self.alias.to_lowercase().contains(&query)
            || self
                .hostname
                .as_ref()
                .is_some_and(|h| h.to_lowercase().contains(&query))
    }
}
