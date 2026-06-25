use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::config::model::Host;

const MAX_INCLUDE_DEPTH: usize = 16;

/// Parses an SSH config file and returns all non-wildcard host entries.
/// Returns an empty Vec if the file does not exist (no panic).
pub fn parse_config(path: &Path) -> Vec<Host> {
    parse_config_with_depth(path, &mut HashSet::new(), 0)
}

fn parse_config_with_depth(path: &Path, visited: &mut HashSet<PathBuf>, depth: usize) -> Vec<Host> {
    if depth > MAX_INCLUDE_DEPTH {
        log::warn!(
            "Include depth limit ({}) exceeded for: {}",
            MAX_INCLUDE_DEPTH,
            path.display()
        );
        return Vec::new();
    }

    let canonical = match path.canonicalize() {
        Ok(c) => c,
        Err(_) => {
            log::warn!("Config file not found: {}", path.display());
            return Vec::new();
        }
    };

    if visited.contains(&canonical) {
        log::warn!("Circular include detected: {}", path.display());
        return Vec::new();
    }
    visited.insert(canonical.clone());

    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            log::warn!("Failed to read config file {}: {}", path.display(), e);
            return Vec::new();
        }
    };

    parse_config_content(&content, path, visited, depth)
}

/// State for tracking the current block being parsed.
struct BlockState {
    aliases: Vec<String>,
    hostname: Option<String>,
    user: Option<String>,
    port: Option<u16>,
    identity_file: Option<PathBuf>,
    line_start: usize,
    tags: Vec<String>,
    extra: Vec<String>,
    local_forward: Option<String>,
    remote_forward: Option<String>,
    dynamic_forward: Option<String>,
}

impl BlockState {
    fn new() -> Self {
        Self {
            aliases: Vec::new(),
            hostname: None,
            user: None,
            port: None,
            identity_file: None,
            line_start: 0,
            tags: Vec::new(),
            extra: Vec::new(),
            local_forward: None,
            remote_forward: None,
            dynamic_forward: None,
        }
    }

    fn reset(&mut self, aliases: Vec<String>, line_start: usize) {
        self.aliases = aliases;
        self.hostname = None;
        self.user = None;
        self.port = None;
        self.identity_file = None;
        self.line_start = line_start;
        self.tags = Vec::new();
        self.extra = Vec::new();
        self.local_forward = None;
        self.remote_forward = None;
        self.dynamic_forward = None;
    }

    fn is_active(&self) -> bool {
        !self.aliases.is_empty()
    }
}

/// Flush a completed host block into the hosts list.
fn flush_block(hosts: &mut Vec<Host>, block: &BlockState, source_file: &Path) {
    if block.is_active() {
        for alias in &block.aliases {
            hosts.push(Host {
                alias: alias.clone(),
                hostname: block.hostname.clone(),
                user: block.user.clone(),
                port: block.port,
                identity_file: block.identity_file.clone(),
                line_start: block.line_start,
                source_file: source_file.to_path_buf(),
                tags: block.tags.clone(),
                extra: block.extra.clone(),
                local_forward: block.local_forward.clone(),
                remote_forward: block.remote_forward.clone(),
                dynamic_forward: block.dynamic_forward.clone(),
            });
        }
    }
}

fn parse_config_content(
    content: &str,
    source_file: &Path,
    visited: &mut HashSet<PathBuf>,
    depth: usize,
) -> Vec<Host> {
    let mut hosts = Vec::new();
    let mut block = BlockState::new();
    let mut in_host_block = false;
    // Pending tag list parsed from a `# @tags:` comment line. Consumed by the
    // next Host directive on the same uninterrupted run. Discarded by any
    // blank line, malformed line, or non-@tags comment.
    let mut pending_tags: Option<Vec<String>> = None;

    let base_dir = source_file.parent().unwrap_or_else(|| Path::new("."));

    for (line_idx, raw_line) in content.lines().enumerate() {
        let line_num = line_idx + 1;
        let line = raw_line.trim();

        if line.is_empty() {
            pending_tags = None;
            continue;
        }
        if line.starts_with('#') {
            pending_tags = crate::config::tags::parse_tag_line(line);
            continue;
        }

        let (keyword, value) = match split_directive(line) {
            Some(pair) => pair,
            None => {
                pending_tags = None;
                continue;
            }
        };
        let tags_for_block = pending_tags.take();

        match keyword.to_lowercase().as_str() {
            "host" => {
                // Flush previous host block
                if in_host_block {
                    flush_block(&mut hosts, &block, source_file);
                    block = BlockState::new();
                }

                let aliases: Vec<String> =
                    value.split_whitespace().map(|s| s.to_string()).collect();

                let non_wildcard_aliases: Vec<String> = aliases
                    .into_iter()
                    .filter(|a| !is_wildcard_only(a))
                    .collect();

                if non_wildcard_aliases.is_empty() {
                    in_host_block = false;
                    continue;
                }

                block.reset(non_wildcard_aliases, line_num);
                if let Some(tags) = tags_for_block {
                    block.tags = tags;
                }
                in_host_block = true;
            }
            "match" => {
                // Flush current host block and ignore Match block content
                if in_host_block {
                    flush_block(&mut hosts, &block, source_file);
                    block = BlockState::new();
                }
                in_host_block = false;
            }
            "hostname" if in_host_block => {
                block.hostname = Some(value.to_string());
            }
            "user" if in_host_block => {
                block.user = Some(value.to_string());
            }
            "port" if in_host_block => {
                block.port = value.parse::<u16>().ok();
            }
            "identityfile" if in_host_block => {
                block.identity_file = Some(resolve_path(&value, base_dir));
            }
            // v0.9 G5: typed Forwarding fields. Last value wins so a
            // round-trip rewrite keeps the most recent directive; any
            // earlier occurrence falls through to `extra` below to be
            // preserved as a free-form line.
            "localforward" if in_host_block => {
                if let Some(prev) = block.local_forward.replace(value.to_string()) {
                    block.extra.push(format!("LocalForward {prev}"));
                }
            }
            "remoteforward" if in_host_block => {
                if let Some(prev) = block.remote_forward.replace(value.to_string()) {
                    block.extra.push(format!("RemoteForward {prev}"));
                }
            }
            "dynamicforward" if in_host_block => {
                if let Some(prev) = block.dynamic_forward.replace(value.to_string()) {
                    block.extra.push(format!("DynamicForward {prev}"));
                }
            }
            "include" if in_host_block => {
                let included = resolve_include(&value, base_dir, visited, depth);
                hosts.extend(included);
            }
            "include" => {
                let included = resolve_include(&value, base_dir, visited, depth);
                hosts.extend(included);
            }
            _ if in_host_block => {
                // Unknown SSH directive inside a Host block — preserve it
                // verbatim so a round-trip rewrite of sshc.conf doesn't
                // drop options like ProxyJump / ForwardAgent / LocalForward.
                block.extra.push(format!("{} {}", keyword, value));
            }
            _ => {}
        }
    }

    // Flush last host block
    if in_host_block {
        flush_block(&mut hosts, &block, source_file);
    }

    hosts
}

/// Splits a directive line into (keyword, value).
/// Handles double-quoted values (quotes stripped) and inline `#` comments.
/// Returns None for lines that cannot be parsed (malformed) or have an empty value.
fn split_directive(line: &str) -> Option<(&str, String)> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }

    let keyword_end = line
        .char_indices()
        .find(|(_, c)| c.is_whitespace() || *c == '=')
        .map(|(i, _)| i)
        .unwrap_or(line.len());

    if keyword_end == 0 {
        return None;
    }

    let keyword = &line[..keyword_end];
    let value_raw = line[keyword_end..].trim_start_matches(|c: char| c.is_whitespace() || c == '=');

    if value_raw.is_empty() {
        return None;
    }

    // Quoted value: take content up to next `"`, discard anything after closing quote.
    // Unquoted value: strip trailing inline ` #` comment, then trim trailing whitespace.
    let value = if let Some(rest) = value_raw.strip_prefix('"') {
        match rest.find('"') {
            Some(end) => rest[..end].to_string(),
            None => rest.to_string(), // unclosed quote: take rest as best-effort
        }
    } else {
        match value_raw.find(" #") {
            Some(idx) => value_raw[..idx].trim_end().to_string(),
            None => value_raw.trim_end().to_string(),
        }
    };

    if value.is_empty() {
        return None;
    }

    Some((keyword, value))
}

/// Returns true if the alias is a wildcard pattern (contains * or ?).
fn is_wildcard_only(alias: &str) -> bool {
    alias.contains('*') || alias.contains('?')
}

/// Resolves an Include directive path.
fn resolve_include(
    value: &str,
    base_dir: &Path,
    visited: &mut HashSet<PathBuf>,
    depth: usize,
) -> Vec<Host> {
    let path = resolve_path(value, base_dir);

    if path.to_string_lossy().contains('*') || path.to_string_lossy().contains('?') {
        match glob_paths(&path) {
            Ok(paths) => {
                let mut all_hosts = Vec::new();
                for p in paths {
                    all_hosts.extend(parse_config_with_depth(&p, visited, depth + 1));
                }
                all_hosts
            }
            Err(e) => {
                log::warn!("Failed to glob include path {}: {}", path.display(), e);
                Vec::new()
            }
        }
    } else {
        parse_config_with_depth(&path, visited, depth + 1)
    }
}

/// Resolves a path that may contain ~ or be relative to base_dir.
fn resolve_path(path_str: &str, base_dir: &Path) -> PathBuf {
    let path_str = path_str.trim();

    if path_str.starts_with("~/") || path_str == "~" {
        if let Some(home) = dirs::home_dir() {
            let rest = path_str.strip_prefix('~').unwrap();
            return home.join(rest.trim_start_matches('/'));
        }
    }

    let path = PathBuf::from(path_str);

    if path.is_relative() {
        base_dir.join(path)
    } else {
        path
    }
}

/// Glob-expand a path pattern.
fn glob_paths(pattern: &Path) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let mut results = Vec::new();

    if let Some(parent) = pattern.parent() {
        if parent.exists() {
            let file_pattern = pattern.file_name().and_then(|f| f.to_str()).unwrap_or("*");

            for entry in std::fs::read_dir(parent)? {
                let entry = entry?;
                let file_name = entry.file_name();
                let name = file_name.to_string_lossy();

                if simple_glob_match(file_pattern, &name) {
                    results.push(entry.path());
                }
            }
            results.sort();
        }
    }

    Ok(results)
}

/// Simple glob matching supporting * and ? wildcards.
fn simple_glob_match(pattern: &str, s: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let st: Vec<char> = s.chars().collect();
    glob_match_internal(&p, &st, 0, 0)
}

fn glob_match_internal(p: &[char], s: &[char], pi: usize, si: usize) -> bool {
    if pi == p.len() {
        return si == s.len();
    }
    if p[pi] == '*' {
        for i in si..=s.len() {
            if glob_match_internal(p, s, pi + 1, i) {
                return true;
            }
        }
        false
    } else if si >= s.len() {
        false
    } else if p[pi] == '?' || p[pi] == s[si] {
        glob_match_internal(p, s, pi + 1, si + 1)
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_directive() {
        assert_eq!(
            split_directive("HostName web.example.com"),
            Some(("HostName", String::from("web.example.com")))
        );
        assert_eq!(
            split_directive("  Host = web  "),
            Some(("Host", String::from("web")))
        );
        assert_eq!(split_directive("# comment"), None);
        assert_eq!(split_directive(""), None);
    }

    #[test]
    fn test_split_directive_quoted_value() {
        assert_eq!(
            split_directive(r#"HostName "my host""#),
            Some(("HostName", String::from("my host")))
        );
        assert_eq!(
            split_directive(r#"HostName "quoted" # ignored"#),
            Some(("HostName", String::from("quoted")))
        );
    }

    #[test]
    fn test_split_directive_inline_comment() {
        assert_eq!(
            split_directive("HostName a.com # comment"),
            Some(("HostName", String::from("a.com")))
        );
        // No space before `#` — not treated as a comment
        assert_eq!(
            split_directive("HostName a.com#notcomment"),
            Some(("HostName", String::from("a.com#notcomment")))
        );
    }

    #[test]
    fn test_split_directive_empty_value_after_trim() {
        assert_eq!(split_directive("HostName  "), None);
    }

    #[test]
    fn test_is_wildcard_only() {
        assert!(is_wildcard_only("*"));
        assert!(is_wildcard_only("*.example.com"));
        assert!(!is_wildcard_only("web"));
        assert!(!is_wildcard_only("web1"));
    }

    #[test]
    fn test_resolve_path_tilde() {
        let base = Path::new("/etc/ssh");
        let resolved = resolve_path("~/ssh/config", base);
        assert!(resolved.to_string_lossy().contains("ssh/config"));
        assert!(!resolved.is_relative());
    }

    #[test]
    fn test_resolve_path_relative() {
        let base = Path::new("/home/user/.ssh");
        let resolved = resolve_path("config.d/work", base);
        assert_eq!(resolved, PathBuf::from("/home/user/.ssh/config.d/work"));
    }

    #[test]
    fn test_resolve_path_absolute() {
        let base = Path::new("/home/user/.ssh");
        let resolved = resolve_path("/etc/ssh/ssh_config", base);
        assert_eq!(resolved, PathBuf::from("/etc/ssh/ssh_config"));
    }

    #[test]
    fn test_match_directive_does_not_leak() {
        use assert_fs::fixture::{FileWriteStr, PathChild};
        let dir = assert_fs::TempDir::new().unwrap();
        let config = dir.child("match_test.config");
        config
            .write_str(
                "Host web\n  HostName web.example.com\n\n\
                 Match host db\n  HostName should-not-leak.example.com\n  User leaked\n\n\
                 Host db\n  HostName 192.0.2.1\n",
            )
            .unwrap();

        let hosts = parse_config(config.path());
        assert_eq!(hosts.len(), 2, "Should have 2 Host entries");

        let web = hosts.iter().find(|h| h.alias == "web").unwrap();
        assert_eq!(web.hostname.as_deref(), Some("web.example.com"));
        assert!(
            web.user.is_none(),
            "web should not have leaked User from Match block"
        );

        let db = hosts.iter().find(|h| h.alias == "db").unwrap();
        assert_eq!(db.hostname.as_deref(), Some("192.0.2.1"));
    }
}
