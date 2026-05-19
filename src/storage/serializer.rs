use crate::config::model::Host;
use crate::config::tags::render_tag_line;

pub const SSHS_CONF_BANNER: &str =
    "# Managed by sshs. Manual edits inside Host blocks may be overwritten on next save.\n\n";

/// Render Host list into the canonical sshs.conf text body.
/// Blocks are separated by a blank line. Output starts with the banner and
/// ends with a newline.
pub fn host_blocks_to_text(hosts: &[Host]) -> String {
    let mut out = String::from(SSHS_CONF_BANNER);
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
        if let Some(ref id) = host.identity_file {
            out.push_str(&format!("    IdentityFile {}\n", id.display()));
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
            identity_file: Some(PathBuf::from("~/.ssh/id_rsa")),
            line_start: 0,
            source_file: PathBuf::new(),
            tags: tags.into_iter().map(String::from).collect(),
        }
    }

    #[test]
    fn test_serialize_host_no_tags() {
        let text = host_blocks_to_text(&[h("test", vec![])]);
        assert!(text.starts_with(SSHS_CONF_BANNER));
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
        assert_eq!(text, SSHS_CONF_BANNER);
    }
}
