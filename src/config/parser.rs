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

fn parse_config_content(
    content: &str,
    source_file: &Path,
    visited: &mut HashSet<PathBuf>,
    depth: usize,
) -> Vec<Host> {
    let mut hosts = Vec::new();
    let mut current_aliases: Vec<String> = Vec::new();
    let mut current_hostname: Option<String> = None;
    let mut current_user: Option<String> = None;
    let mut current_port: Option<u16> = None;
    let mut current_identity_file: Option<PathBuf> = None;
    let mut current_line_start: usize = 0;
    let mut in_host_block = false;

    let base_dir = source_file.parent().unwrap_or_else(|| Path::new("."));

    for (line_idx, raw_line) in content.lines().enumerate() {
        let line_num = line_idx + 1; // 1-indexed
        let line = raw_line.trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Split into keyword and value
        let (keyword, value) = match split_directive(line) {
            Some(pair) => pair,
            None => continue, // malformed line, skip
        };

        match keyword.to_lowercase().as_str() {
            "host" => {
                // Flush previous host block
                if in_host_block && !current_aliases.is_empty() {
                    let hostname = current_hostname.take();
                    let user = current_user.take();
                    let port = current_port.take();
                    let identity_file = current_identity_file.take();
                    let line_start = current_line_start;

                    for alias in current_aliases.drain(..) {
                        hosts.push(Host {
                            alias,
                            hostname: hostname.clone(),
                            user: user.clone(),
                            port,
                            identity_file: identity_file.clone(),
                            line_start,
                            source_file: source_file.to_path_buf(),
                        });
                    }
                }

                // Start new host block
                let aliases: Vec<String> =
                    value.split_whitespace().map(|s| s.to_string()).collect();

                // Filter out wildcard-only aliases
                let non_wildcard_aliases: Vec<String> = aliases
                    .into_iter()
                    .filter(|a| !is_wildcard_only(a))
                    .collect();

                if non_wildcard_aliases.is_empty() {
                    // All aliases are wildcards, skip this block
                    in_host_block = false;
                    continue;
                }

                current_aliases = non_wildcard_aliases;
                current_line_start = line_num;
                current_hostname = None;
                current_user = None;
                current_port = None;
                current_identity_file = None;
                in_host_block = true;
            }
            "hostname" if in_host_block => {
                current_hostname = Some(value.to_string());
            }
            "user" if in_host_block => {
                current_user = Some(value.to_string());
            }
            "port" if in_host_block => {
                current_port = value.parse::<u16>().ok();
            }
            "identityfile" if in_host_block => {
                current_identity_file = Some(resolve_path(value, base_dir));
            }
            "include" if in_host_block => {
                // Include inside a Host block is unusual but handle it
                let included = resolve_include(value, base_dir, visited, depth);
                hosts.extend(included);
            }
            "include" => {
                let included = resolve_include(value, base_dir, visited, depth);
                hosts.extend(included);
            }
            _ if in_host_block => {
                // Other directives inside host block — ignore but keep block active
            }
            _ => {
                // Directive outside host block — ignore
            }
        }
    }

    // Flush last host block
    if in_host_block && !current_aliases.is_empty() {
        for alias in current_aliases {
            hosts.push(Host {
                alias,
                hostname: current_hostname.clone(),
                user: current_user.clone(),
                port: current_port,
                identity_file: current_identity_file.clone(),
                line_start: current_line_start,
                source_file: source_file.to_path_buf(),
            });
        }
    }

    hosts
}

/// Splits a directive line into (keyword, value).
/// Returns None for lines that cannot be parsed (malformed).
fn split_directive(line: &str) -> Option<(&str, &str)> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }

    // Find the end of the keyword (first whitespace or '=')
    let keyword_end = line
        .char_indices()
        .find(|(_, c)| c.is_whitespace() || *c == '=')
        .map(|(i, _)| i)
        .unwrap_or(line.len());

    if keyword_end == 0 {
        return None;
    }

    let keyword = &line[..keyword_end];
    let value = line[keyword_end..].trim_start_matches(|c: char| c.is_whitespace() || c == '=');

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

    // Support glob patterns in include paths
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

    // Expand tilde
    if path_str.starts_with("~/") || path_str == "~" {
        if let Some(home) = dirs::home_dir() {
            let rest = path_str.strip_prefix('~').unwrap();
            return home.join(rest.trim_start_matches('/'));
        }
    }

    let path = PathBuf::from(path_str);

    // Make relative paths relative to base_dir
    if path.is_relative() {
        base_dir.join(path)
    } else {
        path
    }
}

/// Glob-expand a path pattern.
fn glob_paths(pattern: &Path) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let mut results = Vec::new();

    // Simple glob implementation for common patterns like *.conf
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
        // Try matching * with 0 or more characters
        for i in si..=s.len() {
            if glob_match_internal(p, s, pi + 1, i) {
                return true;
            }
        }
        return false;
    }
    if si >= s.len() {
        return false;
    }
    if p[pi] == '?' || p[pi] == s[si] {
        return glob_match_internal(p, s, pi + 1, si + 1);
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_directive() {
        assert_eq!(
            split_directive("HostName web.example.com"),
            Some(("HostName", "web.example.com"))
        );
        assert_eq!(split_directive("  Host = web  "), Some(("Host", "web")));
        assert_eq!(split_directive("# comment"), None);
        assert_eq!(split_directive(""), None);
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
}
