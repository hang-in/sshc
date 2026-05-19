use std::path::PathBuf;

use assert_fs::fixture::{FileWriteStr, PathChild};
use sshs::config::parser::parse_config;
use sshs::exec::editor::build_editor_command;

/// Integration test: parse a temp config file and verify host list.
#[test]
fn test_integration_parse_temp_config() {
    let dir = assert_fs::TempDir::new().unwrap();
    let config = dir.child("ssh_config");
    config
        .write_str(
            "Host web\n  HostName web.example.com\n  User deploy\n  Port 2222\n\n\
             Host db\n  HostName 192.0.2.1\n  User admin\n  Port 5432\n",
        )
        .unwrap();

    let hosts = parse_config(config.path());
    assert_eq!(hosts.len(), 2);

    assert_eq!(hosts[0].alias, "web");
    assert_eq!(hosts[0].hostname.as_deref(), Some("web.example.com"));
    assert_eq!(hosts[0].user.as_deref(), Some("deploy"));
    assert_eq!(hosts[0].port, Some(2222));

    assert_eq!(hosts[1].alias, "db");
    assert_eq!(hosts[1].hostname.as_deref(), Some("192.0.2.1"));
    assert_eq!(hosts[1].user.as_deref(), Some("admin"));
    assert_eq!(hosts[1].port, Some(5432));
}

/// Integration test: verify editor command construction without executing.
#[test]
fn test_integration_editor_command_construction() {
    let file = PathBuf::from("/home/user/.ssh/config");

    // With default (vi), should have +line
    std::env::remove_var("EDITOR");
    let cmd = build_editor_command(&file, 42);
    let program = cmd.get_program().to_string_lossy().to_string();
    assert_eq!(program, "vi");
    let args: Vec<String> = cmd
        .get_args()
        .map(|a| a.to_string_lossy().to_string())
        .collect();
    assert!(args.contains(&"+42".to_string()));
    assert!(args.iter().any(|a| a.contains("config")));
}

/// Integration test: verify ssh command construction without executing.
#[test]
fn test_integration_ssh_command_would_be_correct() {
    // We can't actually call ssh_connect() as it would replace the process,
    // but we can verify the Host model carries the right alias.
    let hosts = parse_config(&{
        let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push("tests/fixtures/basic.config");
        p
    });

    let web_host = hosts.iter().find(|h| h.alias == "web").unwrap();
    assert_eq!(web_host.alias, "web");
    // The alias is what would be passed to `ssh <alias>`
}

/// Integration test: wildcard hosts are excluded from the list.
#[test]
fn test_integration_wildcard_exclusion() {
    let hosts = parse_config(&{
        let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push("tests/fixtures/wildcard.config");
        p
    });

    // Only non-wildcard hosts should appear
    for host in &hosts {
        assert!(
            !host.alias.contains('*'),
            "Wildcard host should be excluded: {}",
            host.alias
        );
    }
    assert_eq!(hosts.len(), 1);
    assert_eq!(hosts[0].alias, "web");
}

/// Integration test: include directive merges hosts from sub-files.
#[test]
fn test_integration_include_merges_hosts() {
    let hosts = parse_config(&{
        let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push("tests/fixtures/with_include.config");
        p
    });

    let aliases: Vec<&str> = hosts.iter().map(|h| h.alias.as_str()).collect();
    assert!(aliases.contains(&"bastion"), "bastion from main file");
    assert!(aliases.contains(&"prod"), "prod from included file");
}

/// Integration test: missing config file returns empty list without panic.
#[test]
fn test_integration_missing_config_empty_state() {
    let hosts = parse_config(PathBuf::from("/nonexistent/path/config").as_path());
    assert!(hosts.is_empty(), "Missing config should return empty list");
}
