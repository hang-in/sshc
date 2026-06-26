use crate::config::model::Host;
use crate::config::tags::render_tag_line;

pub const SSHC_CONF_BANNER: &str =
    "# Managed by sshc. Manual edits inside Host blocks may be overwritten on next save.\n\n";

/// Render Host list into the canonical sshc.conf text body.
/// Blocks are separated by a blank line. Output starts with the banner and
/// ends with a newline.
pub fn host_blocks_to_text(hosts: &[Host]) -> String {
    let mut out = String::from(SSHC_CONF_BANNER);
    for (i, host) in hosts.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        if !host.tags.is_empty() {
            out.push_str(&render_tag_line(&host.tags));
            out.push('\n');
        }
        out.push_str(&format!("Host {}\n", host.alias));
        if let Some(ref hn) = host.hostname {
            out.push_str(&format!("    HostName {}\n", hn));
        }
        if let Some(ref u) = host.user {
            out.push_str(&format!("    User {}\n", u));
        }
        if let Some(p) = host.port {
            out.push_str(&format!("    Port {}\n", p));
        }
        // v0.12 G1: emit one line per IdentityFile entry. OpenSSH
        // tries them in declaration order; preserving that order is
        // the contract that lets round-trips re-write the file
        // faithfully.
        for id in &host.identity_file {
            out.push_str(&format!("    IdentityFile {}\n", id.display()));
        }
        // v0.10 G1: emit one line per typed Forwarding entry. Same
        // ordering across the three directive kinds: Local first,
        // then Remote, then Dynamic.
        for lf in &host.local_forward {
            out.push_str(&format!("    LocalForward {}\n", lf));
        }
        for rf in &host.remote_forward {
            out.push_str(&format!("    RemoteForward {}\n", rf));
        }
        for df in &host.dynamic_forward {
            out.push_str(&format!("    DynamicForward {}\n", df));
        }
        for line in &host.extra {
            out.push_str("    ");
            out.push_str(line.trim());
            out.push('\n');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn h(alias: &str, tags: Vec<&str>) -> Host {
        Host {
            alias: alias.to_string(),
            hostname: Some("1.2.3.4".to_string()),
            user: Some("user".to_string()),
            port: Some(22),
            identity_file: vec![PathBuf::from("~/.ssh/id_rsa")],
            line_start: 0,
            source_file: PathBuf::new(),
            tags: tags.into_iter().map(String::from).collect(),
            extra: Vec::new(),
            local_forward: Vec::new(),
            remote_forward: Vec::new(),
            dynamic_forward: Vec::new(),
        }
    }

    #[test]
    fn test_round_trip_forwarding_through_parser() {
        // Serialize a host with forwarding fields, parse the result
        // back, and assert the typed fields survived.
        use crate::config::parser::parse_config;
        use assert_fs::fixture::{FileWriteStr, PathChild};
        let temp = assert_fs::TempDir::new().unwrap();
        let path = temp.child("sshc.conf");

        let mut host = h("fwd", vec![]);
        host.local_forward = vec!["8080 localhost:80".to_string()];
        host.remote_forward = vec!["9090 127.0.0.1:9090".to_string()];
        host.dynamic_forward = vec!["1080".to_string()];
        path.write_str(&host_blocks_to_text(&[host])).unwrap();

        let parsed = parse_config(path.path());
        let h = parsed
            .iter()
            .find(|h| h.alias == "fwd")
            .expect("expected fwd host in parsed config");
        assert_eq!(h.local_forward, vec!["8080 localhost:80".to_string()]);
        assert_eq!(h.remote_forward, vec!["9090 127.0.0.1:9090".to_string()]);
        assert_eq!(h.dynamic_forward, vec!["1080".to_string()]);
    }

    #[test]
    fn test_round_trip_multi_identity_file_preserves_order() {
        // v0.12 G1: multiple IdentityFile lines on a single host
        // survive a serialize-then-parse cycle in declaration order.
        use crate::config::parser::parse_config;
        use assert_fs::fixture::{FileWriteStr, PathChild};
        let temp = assert_fs::TempDir::new().unwrap();
        let path = temp.child("sshc.conf");

        let mut host = h("multi-id", vec![]);
        host.identity_file = vec![
            PathBuf::from("/home/u/.ssh/id_ed25519"),
            PathBuf::from("/home/u/.ssh/id_rsa"),
            PathBuf::from("/home/u/.ssh/id_corp"),
        ];
        path.write_str(&host_blocks_to_text(&[host])).unwrap();

        let parsed = parse_config(path.path());
        let h = parsed
            .iter()
            .find(|h| h.alias == "multi-id")
            .expect("expected multi-id host");
        assert_eq!(
            h.identity_file,
            vec![
                PathBuf::from("/home/u/.ssh/id_ed25519"),
                PathBuf::from("/home/u/.ssh/id_rsa"),
                PathBuf::from("/home/u/.ssh/id_corp"),
            ]
        );
    }

    #[test]
    fn test_round_trip_multi_forwarding_preserves_order() {
        // v0.10 G1: multiple LocalForward / RemoteForward lines on a
        // single host survive a serialize-then-parse cycle in
        // declaration order.
        use crate::config::parser::parse_config;
        use assert_fs::fixture::{FileWriteStr, PathChild};
        let temp = assert_fs::TempDir::new().unwrap();
        let path = temp.child("sshc.conf");

        let mut host = h("multi", vec![]);
        host.local_forward = vec![
            "8080 localhost:80".to_string(),
            "9090 db.internal:5432".to_string(),
            "9091 cache.internal:6379".to_string(),
        ];
        path.write_str(&host_blocks_to_text(&[host])).unwrap();

        let parsed = parse_config(path.path());
        let h = parsed.iter().find(|h| h.alias == "multi").unwrap();
        assert_eq!(
            h.local_forward,
            vec![
                "8080 localhost:80".to_string(),
                "9090 db.internal:5432".to_string(),
                "9091 cache.internal:6379".to_string(),
            ]
        );
    }

    #[test]
    fn test_serialize_emits_typed_forwarding_fields() {
        let mut host = h("fwd", vec![]);
        host.local_forward = vec!["8080 localhost:80".to_string()];
        host.remote_forward = vec!["9000 127.0.0.1:9000".to_string()];
        host.dynamic_forward = vec!["1080".to_string()];
        let text = host_blocks_to_text(&[host]);
        assert!(text.contains("    LocalForward 8080 localhost:80\n"));
        assert!(text.contains("    RemoteForward 9000 127.0.0.1:9000\n"));
        assert!(text.contains("    DynamicForward 1080\n"));
    }

    #[test]
    fn test_serialize_host_no_tags() {
        let text = host_blocks_to_text(&[h("test", vec![])]);
        assert!(text.starts_with(SSHC_CONF_BANNER));
        assert!(text.contains("Host test\n"));
        assert!(!text.contains("# @tags"));
    }

    #[test]
    fn test_serialize_host_with_tags() {
        let text = host_blocks_to_text(&[h("test", vec!["work"])]);
        assert!(text.contains("# @tags: work\nHost test"));
    }

    #[test]
    fn test_serialize_multiple_blocks_separated_by_blank() {
        let text = host_blocks_to_text(&[h("h1", vec![]), h("h2", vec![])]);
        let h1_pos = text.find("Host h1\n").unwrap();
        let h2_pos = text.find("Host h2\n").unwrap();
        assert!(h2_pos > h1_pos);
        let between = &text[h1_pos..h2_pos];
        assert!(between.contains("\n\n"));
    }

    #[test]
    fn test_serialize_empty_list() {
        let text = host_blocks_to_text(&[]);
        assert_eq!(text, SSHC_CONF_BANNER);
    }
}
