pub fn parse_tag_line(line: &str) -> Option<Vec<String>> {
    let trimmed = line.trim_start();
    if !trimmed.starts_with('#') {
        return None;
    }

    let after_hash = &trimmed[1..];
    let trimmed_after_hash = after_hash.trim_start();
    if !trimmed_after_hash.starts_with("@tags:") {
        return None;
    }

    let content = &trimmed_after_hash["@tags:".len()..];
    let mut tags = Vec::new();

    for raw in content.split(',') {
        if let Some(normalized) = normalize_tag(raw) {
            if !tags.contains(&normalized) {
                tags.push(normalized);
            }
        }
    }

    Some(tags)
}

pub fn render_tag_line(tags: &[String]) -> String {
    if tags.is_empty() {
        return String::new();
    }
    format!("# @tags: {}", tags.join(", "))
}

pub fn normalize_tag(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_lowercase())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic() {
        assert_eq!(
            parse_tag_line("# @tags: a, b"),
            Some(vec!["a".to_string(), "b".to_string()])
        );
    }

    #[test]
    fn test_parse_empty_after_colon() {
        assert_eq!(parse_tag_line("# @tags:"), Some(vec![]));
        assert_eq!(parse_tag_line("# @tags:   "), Some(vec![]));
    }

    #[test]
    fn test_parse_non_tag_comment() {
        assert_eq!(parse_tag_line("# regular comment"), None);
        assert_eq!(parse_tag_line("HostName foo"), None);
        assert_eq!(parse_tag_line(""), None);
    }

    #[test]
    fn test_parse_normalizes_and_dedupes() {
        assert_eq!(
            parse_tag_line("  # @tags: A , b  , a "),
            Some(vec!["a".to_string(), "b".to_string()])
        );
    }

    #[test]
    fn test_parse_leading_hash_whitespace() {
        assert_eq!(parse_tag_line("#  @tags: x"), Some(vec!["x".to_string()]));
    }

    #[test]
    fn test_render_empty() {
        assert_eq!(render_tag_line(&[]), "");
    }

    #[test]
    fn test_render_single() {
        assert_eq!(render_tag_line(&["x".to_string()]), "# @tags: x");
    }

    #[test]
    fn test_render_multiple() {
        assert_eq!(
            render_tag_line(&["a".to_string(), "b".to_string()]),
            "# @tags: a, b"
        );
    }

    #[test]
    fn test_normalize_basic() {
        assert_eq!(normalize_tag("  Foo "), Some("foo".to_string()));
        assert_eq!(normalize_tag("BAR"), Some("bar".to_string()));
    }

    #[test]
    fn test_normalize_empty() {
        assert_eq!(normalize_tag(""), None);
        assert_eq!(normalize_tag("   "), None);
    }
}
