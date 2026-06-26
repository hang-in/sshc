use std::fmt;
use std::path::PathBuf;

use nucleo::pattern::{AtomKind, CaseMatching, Normalization, Pattern};
use nucleo::{Matcher, Utf32Str};

/// Represents a single SSH host entry parsed from ~/.ssh/config.
#[derive(Debug, Clone)]
pub struct Host {
    pub alias: String,
    pub hostname: Option<String>,
    pub user: Option<String>,
    pub port: Option<u16>,
    /// v0.12 G1: list of `IdentityFile` paths. OpenSSH allows
    /// multiple per host (tried in order); v0.10 G1 promoted the
    /// three Forwarding kinds to Vec for the same reason, this
    /// closes the symmetric gap. Empty Vec ↔ no IdentityFile.
    pub identity_file: Vec<PathBuf>,
    pub line_start: usize,
    pub source_file: PathBuf,
    /// Tags parsed from a `# @tags: a, b` comment immediately above
    /// the Host block (no blank line between). Lowercase, deduped.
    pub tags: Vec<String>,
    /// SSH config directives we don't model as typed fields. Each entry
    /// is a single line in `Key Value` form (no leading indent). The
    /// parser preserves them verbatim; the serializer emits them with
    /// the standard 4-space indent after the typed fields. Use for
    /// `ProxyJump`, `ForwardAgent`, and any directives not yet promoted
    /// to typed fields.
    pub extra: Vec<String>,
    /// v0.10 G1: typed Forwarding directives. OpenSSH allows the same
    /// directive multiple times per host; each entry here is one such
    /// value (`port[:bind]:host:hostport` for Local/Remote,
    /// `port[:bind]` for Dynamic). The parser collects them in the
    /// order they appear; the serializer emits one line per entry.
    /// Empty Vec ↔ directive not set on this host.
    pub local_forward: Vec<String>,
    pub remote_forward: Vec<String>,
    pub dynamic_forward: Vec<String>,
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
    /// Returns the fuzzy match score against the query using nucleo.
    /// Higher score = better match. Returns 0 if no match.
    pub fn fuzzy_score(&self, query: &str, matcher: &mut Matcher) -> u32 {
        if query.is_empty() {
            return 1; // Empty query matches everything with minimal score
        }

        let pattern = Pattern::new(
            query,
            CaseMatching::Ignore,
            Normalization::default(),
            AtomKind::Fuzzy,
        );

        // Score against alias (primary match target)
        let mut buf = Vec::new();
        let alias_str = Utf32Str::new(&self.alias, &mut buf);
        let alias_score = pattern.score(alias_str, matcher).unwrap_or(0);

        // Score against hostname (secondary match target)
        let hostname_score = self
            .hostname
            .as_ref()
            .map(|h| {
                let haystack = Utf32Str::new(h, &mut buf);
                pattern.score(haystack, matcher).unwrap_or(0)
            })
            .unwrap_or(0);

        // Return best score
        alias_score.max(hostname_score)
    }

    /// Returns true if the host matches the query with fuzzy matching.
    pub fn fuzzy_match(&self, query: &str, matcher: &mut Matcher) -> bool {
        self.fuzzy_score(query, matcher) > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_host(alias: &str, hostname: Option<&str>) -> Host {
        Host {
            alias: alias.to_string(),
            hostname: hostname.map(|h| h.to_string()),
            user: Some("deploy".to_string()),
            port: Some(22),
            identity_file: Vec::new(),
            line_start: 1,
            source_file: PathBuf::from("/test/config"),
            tags: Vec::new(),
            extra: Vec::new(),
            local_forward: Vec::new(),
            remote_forward: Vec::new(),
            dynamic_forward: Vec::new(),
        }
    }

    #[test]
    fn test_fuzzy_match_exact() {
        let mut matcher = Matcher::new(nucleo::Config::DEFAULT);
        let host = make_host("web-server", Some("web.example.com"));
        assert!(host.fuzzy_match("web-server", &mut matcher));
    }

    #[test]
    fn test_fuzzy_match_prefix() {
        let mut matcher = Matcher::new(nucleo::Config::DEFAULT);
        let host = make_host("web-server", Some("web.example.com"));
        assert!(host.fuzzy_match("web", &mut matcher));
    }

    #[test]
    fn test_fuzzy_match_non_contiguous() {
        let mut matcher = Matcher::new(nucleo::Config::DEFAULT);
        let host = make_host("web-server", Some("web.example.com"));
        // "wbsrv" should match "web-server" via fuzzy (non-contiguous)
        assert!(host.fuzzy_match("wbsrv", &mut matcher));
    }

    #[test]
    fn test_fuzzy_match_case_insensitive() {
        let mut matcher = Matcher::new(nucleo::Config::DEFAULT);
        let host = make_host("web", Some("web.example.com"));
        assert!(host.fuzzy_match("WEB", &mut matcher));
    }

    #[test]
    fn test_fuzzy_match_no_match() {
        let mut matcher = Matcher::new(nucleo::Config::DEFAULT);
        let host = make_host("web-server", Some("web.example.com"));
        assert!(!host.fuzzy_match("xyz", &mut matcher));
    }

    #[test]
    fn test_fuzzy_match_empty_query() {
        let mut matcher = Matcher::new(nucleo::Config::DEFAULT);
        let host = make_host("web", Some("web.example.com"));
        assert!(host.fuzzy_match("", &mut matcher));
    }

    #[test]
    fn test_fuzzy_match_hostname() {
        let mut matcher = Matcher::new(nucleo::Config::DEFAULT);
        let host = make_host("prod", Some("production.example.com"));
        // Should match on hostname even if alias doesn't contain query
        assert!(host.fuzzy_match("example", &mut matcher));
    }

    #[test]
    fn test_fuzzy_score_ordering() {
        let mut matcher = Matcher::new(nucleo::Config::DEFAULT);
        let host = make_host("web-server", Some("web.example.com"));
        // Exact match should score higher than prefix match
        let exact = host.fuzzy_score("web-server", &mut matcher);
        let prefix = host.fuzzy_score("web", &mut matcher);
        assert!(exact >= prefix);
    }
}
