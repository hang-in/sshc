use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use sshc::config::model::Host;
use sshc::inline_app::{InlineAction, InlineApp};
use sshc::state::{MemorySection, State, CURRENT_VERSION};

fn host(alias: &str, hostname: &str) -> Host {
    Host {
        alias: alias.to_string(),
        hostname: Some(hostname.to_string()),
        user: Some("deploy".to_string()),
        port: Some(22),
        identity_file: None,
        line_start: 1,
        source_file: PathBuf::from("/tmp/fixture-config"),
        tags: Vec::new(),
        extra: Vec::new(),
    }
}

fn ke(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::empty())
}

fn ke_ctrl(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::CONTROL)
}

fn send_chars(app: &mut InlineApp, s: &str) {
    for c in s.chars() {
        app.handle_key(ke(KeyCode::Char(c)));
    }
}

#[test]
fn test_filter_then_enter_connects_to_top_match() {
    let hosts = vec![
        host("web-prod", "1.1.1.1"),
        host("web-staging", "1.1.1.2"),
        host("db-prod", "1.1.1.3"),
        host("api-gateway", "1.1.1.4"),
    ];
    let mut app = InlineApp::new(hosts);

    send_chars(&mut app, "web");
    assert_eq!(app.host_count(), 2);

    app.handle_key(ke(KeyCode::Enter));
    match app.take_action() {
        Some(InlineAction::Connect(alias)) => {
            assert!(
                alias.starts_with("web"),
                "expected a web-prefixed alias, got {alias}"
            );
        }
        other => panic!("expected Connect, got {other:?}"),
    }
}

#[test]
fn test_navigate_down_then_enter_picks_third_host() {
    let hosts = vec![
        host("alpha", "1.1.1.1"),
        host("bravo", "1.1.1.2"),
        host("charlie", "1.1.1.3"),
        host("delta", "1.1.1.4"),
    ];
    let mut app = InlineApp::new(hosts);

    app.handle_key(ke(KeyCode::Down));
    app.handle_key(ke(KeyCode::Down));
    assert_eq!(app.selected, 2);

    app.handle_key(ke(KeyCode::Enter));
    assert_eq!(
        app.take_action(),
        Some(InlineAction::Connect("charlie".to_string()))
    );
}

#[test]
fn test_state_seeded_reconnect() {
    let hosts = vec![host("host-a", "1.1.1.1"), host("host-b", "1.1.1.2")];
    let state = State {
        version: CURRENT_VERSION,
        setup: Default::default(),
        memory: MemorySection {
            last_connected_alias: Some("host-b".to_string()),
            ..Default::default()
        },
    };

    let mut app = InlineApp::new_with_state(hosts, &state);
    assert_eq!(app.last_connected, Some("host-b".to_string()));

    app.handle_key(ke(KeyCode::Char('r')));
    assert_eq!(app.take_action(), Some(InlineAction::Reconnect));
}

#[test]
fn test_filter_then_clear_then_quit() {
    let hosts = vec![host("x", "1.1.1.1"), host("y", "1.1.1.2")];
    let mut app = InlineApp::new(hosts);

    send_chars(&mut app, "x");
    assert_eq!(app.query, "x");

    app.handle_key(ke(KeyCode::Esc));
    assert!(app.query.is_empty());
    assert!(!app.has_pending_action());

    app.handle_key(ke(KeyCode::Esc));
    assert_eq!(app.take_action(), Some(InlineAction::Quit));
}

#[test]
fn test_ctrl_c_quits_mid_typing() {
    let hosts = vec![host("x", "1.1.1.1")];
    let mut app = InlineApp::new(hosts);

    send_chars(&mut app, "a");
    app.handle_key(ke_ctrl(KeyCode::Char('c')));
    assert_eq!(app.take_action(), Some(InlineAction::Quit));
}
