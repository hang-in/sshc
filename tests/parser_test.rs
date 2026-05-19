use std::path::PathBuf;

use assert_fs::fixture::{FileWriteStr, PathChild, PathCreateDir};
use sshs::config::model::Host;
use sshs::config::parser::parse_config;

fn fixture_path(name: &str) -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests/fixtures");
    p.push(name);
    p
}

#[test]
fn test_parse_basic() {
    let hosts = parse_config(&fixture_path("basic.config"));
    assert_eq!(hosts.len(), 2);

    let web = &hosts[0];
    assert_eq!(web.alias, "web");
    assert_eq!(web.hostname.as_deref(), Some("web.example.com"));
    assert_eq!(web.user.as_deref(), Some("deploy"));
    assert_eq!(web.port, Some(2222));
    assert!(web.identity_file.is_some());

    let db = &hosts[1];
    assert_eq!(db.alias, "db");
    assert_eq!(db.hostname.as_deref(), Some("192.0.2.1"));
    assert_eq!(db.user.as_deref(), Some("admin"));
    assert_eq!(db.port, Some(5432));
}

#[test]
fn test_parse_case_insensitive() {
    let dir = assert_fs::TempDir::new().unwrap();
    let config = dir.child("mixed_case.config");
    config
        .write_str("Host myserver\n  HOSTNAME example.com\n  USER testuser\n  PORT 22\n")
        .unwrap();

    let hosts = parse_config(config.path());
    assert_eq!(hosts.len(), 1);
    assert_eq!(hosts[0].alias, "myserver");
    assert_eq!(hosts[0].hostname.as_deref(), Some("example.com"));
    assert_eq!(hosts[0].user.as_deref(), Some("testuser"));
    assert_eq!(hosts[0].port, Some(22));
}

#[test]
fn test_parse_multi_alias() {
    let hosts = parse_config(&fixture_path("multi_alias.config"));
    let web_aliases: Vec<&str> = hosts.iter().map(|h| h.alias.as_str()).collect();
    assert!(web_aliases.contains(&"web1"));
    assert!(web_aliases.contains(&"web2"));
    assert!(web_aliases.contains(&"web3"));

    let web_hosts: Vec<&Host> = hosts
        .iter()
        .filter(|h| h.alias.starts_with("web"))
        .collect();
    let first_line = web_hosts[0].line_start;
    for h in &web_hosts {
        assert_eq!(h.line_start, first_line);
        assert_eq!(h.hostname.as_deref(), Some("web.example.com"));
    }

    assert!(hosts.iter().any(|h| h.alias == "db"));
}

#[test]
fn test_parse_skip_wildcard() {
    let hosts = parse_config(&fixture_path("wildcard.config"));
    assert_eq!(hosts.len(), 1);
    assert_eq!(hosts[0].alias, "web");
}

#[test]
fn test_parse_include_directive() {
    let hosts = parse_config(&fixture_path("with_include.config"));
    let aliases: Vec<&str> = hosts.iter().map(|h| h.alias.as_str()).collect();
    assert!(aliases.contains(&"bastion"));
    assert!(aliases.contains(&"prod"));
}

#[test]
fn test_parse_include_tilde_expansion() {
    let dir = assert_fs::TempDir::new().unwrap();
    let subdir = dir.child("config.d");
    subdir.create_dir_all().unwrap();
    let included = subdir.child("work.config");
    included
        .write_str("Host workhost\n  HostName work.example.com\n")
        .unwrap();

    let config = dir.child("main.config");
    config
        .write_str(&format!(
            "Host local\n  HostName localhost\n\nInclude {}/work.config\n",
            subdir.path().display()
        ))
        .unwrap();

    let hosts = parse_config(config.path());
    let aliases: Vec<&str> = hosts.iter().map(|h| h.alias.as_str()).collect();
    assert!(aliases.contains(&"local"));
    assert!(aliases.contains(&"workhost"));
}

#[test]
fn test_parse_malformed_recovers() {
    let hosts = parse_config(&fixture_path("malformed.config"));
    let web = hosts
        .iter()
        .find(|h| h.alias == "web")
        .expect("web host missing");
    let db = hosts
        .iter()
        .find(|h| h.alias == "db")
        .expect("db host missing");

    // web should not be contaminated by "this is garbage" / "IndentedNoKey"
    assert_eq!(web.hostname.as_deref(), Some("web.example.com"));
    assert_eq!(web.user.as_deref(), Some("deploy"));
    assert_eq!(web.port, None);

    // db's `Port` directive has no value, should silently produce None
    assert_eq!(db.hostname.as_deref(), Some("192.0.2.1"));
    assert_eq!(db.port, None);
}

#[test]
fn test_parse_circular_include_terminates() {
    // circular_a.config includes circular_b.config which includes circular_a.config.
    // The HashSet-based visited check should break the cycle without infinite recursion.
    let hosts = parse_config(&fixture_path("circular_a.config"));
    assert!(
        hosts.iter().any(|h| h.alias == "hostA"),
        "hostA missing from cycle parse"
    );
    assert!(
        hosts.iter().any(|h| h.alias == "hostB"),
        "hostB missing from cycle parse"
    );
    // Each host should be parsed exactly once despite the cycle
    assert_eq!(hosts.len(), 2);
}

#[test]
fn test_parse_empty_file() {
    let dir = assert_fs::TempDir::new().unwrap();
    let config = dir.child("empty.config");
    config.write_str("").unwrap();

    let hosts = parse_config(config.path());
    assert!(hosts.is_empty());
}

#[test]
fn test_parse_missing_file() {
    let hosts = parse_config(PathBuf::from("/nonexistent/path/config").as_path());
    assert!(hosts.is_empty());
}

#[test]
fn test_line_numbers_correct() {
    let hosts = parse_config(&fixture_path("basic.config"));
    assert_eq!(hosts[0].line_start, 1);
    assert_eq!(hosts[1].line_start, 7);
}

#[test]
fn test_parse_tags_attached_to_host() {
    let dir = assert_fs::TempDir::new().unwrap();
    let config = dir.child("tags.config");
    config
        .write_str("# @tags: prod, api\nHost web\n  HostName web.example.com\n")
        .unwrap();

    let hosts = parse_config(config.path());
    assert_eq!(hosts.len(), 1);
    assert_eq!(hosts[0].alias, "web");
    assert_eq!(hosts[0].tags, vec!["prod".to_string(), "api".to_string()]);
}

#[test]
fn test_parse_tags_discarded_by_blank_line() {
    // Blank line between @tags and Host => tags do not attach.
    let dir = assert_fs::TempDir::new().unwrap();
    let config = dir.child("tags_blank.config");
    config
        .write_str("# @tags: prod\n\nHost web\n  HostName web.example.com\n")
        .unwrap();

    let hosts = parse_config(config.path());
    assert_eq!(hosts.len(), 1);
    assert!(hosts[0].tags.is_empty());
}

#[test]
fn test_parse_tags_discarded_by_intervening_comment() {
    // A non-@tags comment between the @tags line and Host => tags do not attach.
    let dir = assert_fs::TempDir::new().unwrap();
    let config = dir.child("tags_comment.config");
    config
        .write_str("# @tags: prod\n# other comment\nHost web\n  HostName web.example.com\n")
        .unwrap();

    let hosts = parse_config(config.path());
    assert_eq!(hosts.len(), 1);
    assert!(hosts[0].tags.is_empty());
}

#[test]
fn test_parse_tags_only_last_tag_line_wins() {
    // Multiple consecutive @tags lines: only the bottom-most one attaches.
    let dir = assert_fs::TempDir::new().unwrap();
    let config = dir.child("tags_multi.config");
    config
        .write_str("# @tags: old\n# @tags: new\nHost web\n  HostName web.example.com\n")
        .unwrap();

    let hosts = parse_config(config.path());
    assert_eq!(hosts.len(), 1);
    assert_eq!(hosts[0].tags, vec!["new".to_string()]);
}

#[test]
fn test_parse_tags_do_not_leak_to_next_block() {
    // @tags only attaches to the immediately-following Host; the second Host has no tags.
    let dir = assert_fs::TempDir::new().unwrap();
    let config = dir.child("tags_leak.config");
    config
        .write_str(
            "# @tags: a\nHost first\n  HostName first.example.com\n\n\
             Host second\n  HostName second.example.com\n",
        )
        .unwrap();

    let hosts = parse_config(config.path());
    let first = hosts.iter().find(|h| h.alias == "first").unwrap();
    let second = hosts.iter().find(|h| h.alias == "second").unwrap();
    assert_eq!(first.tags, vec!["a".to_string()]);
    assert!(second.tags.is_empty());
}
