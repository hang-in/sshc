use super::*;
use crate::ui::modal::{ModalAction, ModalKind};
use crossterm::event::{KeyCode, KeyEvent};
use std::path::PathBuf;

fn make_host(alias: &str) -> Host {
    Host {
        alias: alias.to_string(),
        hostname: Some(format!("{}.example.com", alias)),
        user: Some("deploy".to_string()),
        port: Some(22),
        identity_file: None,
        line_start: 1,
        source_file: PathBuf::from("/test/config"),
        tags: Vec::new(),
        extra: Vec::new(),
        local_forward: None,
        remote_forward: None,
        dynamic_forward: None,
    }
}

fn make_host_with_tags(alias: &str, tags: &[&str]) -> Host {
    let mut h = make_host(alias);
    h.tags = tags.iter().map(|s| s.to_string()).collect();
    h
}

#[test]
fn test_app_navigation() {
    let hosts = vec![make_host("a"), make_host("b"), make_host("c")];
    let mut app = App::new(hosts);
    assert_eq!(app.selected, 0);

    app.next();
    assert_eq!(app.selected, 1);

    app.next();
    assert_eq!(app.selected, 2);

    app.next();
    assert_eq!(app.selected, 0);

    app.previous();
    assert_eq!(app.selected, 2);
}

#[test]
fn test_app_filter() {
    let hosts = vec![make_host("web"), make_host("db"), make_host("web-prod")];
    let mut app = App::new(hosts);

    app.filter_mode = true;
    app.filter_query = "web".to_string();
    app.apply_filter();

    assert_eq!(app.filtered.len(), 2);
}

#[test]
fn test_app_quit_on_esc_without_filter() {
    let hosts = vec![make_host("a")];
    let mut app = App::new(hosts);
    app.handle_key(KeyEvent::from(KeyCode::Esc));
    assert_eq!(app.take_action(), Some(AppAction::Quit));
}

#[test]
fn test_app_s_connects() {
    let hosts = vec![make_host("a")];
    let mut app = App::new(hosts);
    app.handle_key(KeyEvent::from(KeyCode::Char('s')));
    assert_eq!(app.take_action(), Some(AppAction::Connect("a".to_string())));
}

#[test]
fn test_app_enter_on_external_host_opens_editor() {
    // make_host() seeds source_file = "/test/config" (not sshc.conf).
    let hosts = vec![make_host("a")];
    let mut app = App::new(hosts);
    app.handle_key(KeyEvent::from(KeyCode::Enter));
    assert_eq!(app.take_action(), Some(AppAction::EditConfig));
}

#[test]
fn test_app_enter_on_managed_host_opens_form() {
    let mut h = make_host("managed");
    h.source_file = crate::storage::sshc_conf_path().unwrap_or_default();
    let mut app = App::new(vec![h]);
    app.handle_key(KeyEvent::from(KeyCode::Enter));
    // Enter should open the modify form (Modal::Form). No pending action.
    assert!(matches!(app.mode, AppMode::Modal(ModalKind::Form(_))));
    assert!(app.take_action().is_none());
}

#[test]
fn test_app_m_key_unbound() {
    // 'm' is no longer a manage-mode shortcut (merged into Enter).
    let mut h = make_host("managed");
    h.source_file = crate::storage::sshc_conf_path().unwrap_or_default();
    let mut app = App::new(vec![h]);
    app.handle_key(KeyEvent::from(KeyCode::Char('m')));
    assert!(matches!(app.mode, AppMode::List));
    assert!(app.take_action().is_none());
}

#[test]
fn test_app_promote_m_on_managed_host_emits_status_no_action() {
    let mut h = make_host("already-mine");
    h.source_file = crate::storage::sshc_conf_path().unwrap_or_default();
    let mut app = App::new(vec![h]);
    app.handle_key(KeyEvent::from(KeyCode::Char('M')));
    // Mode stays in List (no form opened), no pending action, but the
    // status bar carries an "already managed" hint.
    assert!(matches!(app.mode, AppMode::List));
    assert!(app.take_action().is_none());
    let msg = app
        .status_message
        .as_ref()
        .expect("expected status hint")
        .text();
    assert!(
        msg.contains("already managed"),
        "expected 'already managed' hint, got {msg:?}"
    );
}

#[test]
fn test_app_promote_m_on_external_host_emits_open_promote_form() {
    // make_host() seeds source_file = "/test/config" — external.
    let hosts = vec![make_host("borrowed")];
    let mut app = App::new(hosts);
    app.handle_key(KeyEvent::from(KeyCode::Char('M')));
    assert_eq!(
        app.take_action(),
        Some(AppAction::OpenPromoteForm("borrowed".to_string()))
    );
    assert!(matches!(app.mode, AppMode::List));
}

#[test]
fn test_app_promote_m_in_read_only_emits_hint_no_action() {
    let hosts = vec![make_host("borrowed")];
    let mut app = App::new(hosts);
    app.state.setup.declined_include_injection = true;
    app.handle_key(KeyEvent::from(KeyCode::Char('M')));
    // Read-only blocks the promote, like it blocks 'a'/'t'/etc.
    assert!(app.take_action().is_none());
    let msg = app
        .status_message
        .as_ref()
        .expect("expected read-only hint")
        .text();
    assert!(msg.contains("read-only"), "got {msg:?}");
}

#[test]
fn test_open_promote_form_wildcard_alias_rejected() {
    // Even if a wildcard somehow made it into the host list, promote
    // refuses to open a form for it — sshc.conf can't express the
    // pattern the user thinks they're carrying over.
    let mut h = make_host("prod-*");
    // source_file stays at the make_host default (/test/config), which
    // is treated as external.
    h.alias = "prod-*".to_string();
    let mut app = App::new(vec![h]);
    app.open_promote_form("prod-*");
    // Mode stays in List, status carries the wildcard hint.
    assert!(matches!(app.mode, AppMode::List));
    let msg = app
        .status_message
        .as_ref()
        .expect("expected wildcard rejection hint")
        .text();
    assert!(
        msg.contains("wildcard"),
        "expected wildcard rejection, got {msg:?}"
    );
}

#[test]
fn test_open_promote_form_external_host_opens_with_promote_context() {
    // make_host() seeds source_file = "/test/config" — external.
    let mut app = App::new(vec![make_host("borrowed")]);
    app.open_promote_form("borrowed");
    assert!(matches!(app.mode, AppMode::Modal(ModalKind::Form(_))));
    assert!(matches!(
        app.active_form_context,
        Some(FormContext::PromoteHost(ref a)) if a == "borrowed"
    ));
}

#[test]
fn test_open_promote_form_on_managed_host_rejects() {
    let mut h = make_host("already-mine");
    h.source_file = crate::storage::sshc_conf_path().unwrap_or_default();
    let mut app = App::new(vec![h]);
    app.open_promote_form("already-mine");
    // No form opens — still in List mode with a clear hint.
    assert!(matches!(app.mode, AppMode::List));
    let msg = app
        .status_message
        .as_ref()
        .expect("expected 'already managed' hint")
        .text();
    assert!(msg.contains("already managed"), "got {msg:?}");
}

#[test]
fn test_open_promote_form_alias_not_found_silent_noop() {
    let app_before = App::new(vec![make_host("borrowed")]);
    let mut app = App::new(vec![make_host("borrowed")]);
    app.open_promote_form("does-not-exist");
    assert!(matches!(app.mode, AppMode::List));
    assert!(app.status_message.is_none());
    // sanity: hosts list unchanged.
    assert_eq!(app.hosts.len(), app_before.hosts.len());
}

#[test]
fn test_app_initial_last_connected_none() {
    let app = App::new(vec![]);
    assert!(app.last_connected.is_none());
}

#[test]
fn test_app_replace_hosts_preserves_alias_selection() {
    let mut app = App::new(vec![make_host("a"), make_host("b"), make_host("c")]);
    app.selected = 1;
    let prev_alias = app.selected_host().unwrap().alias.clone();
    assert_eq!(prev_alias, "b");

    app.replace_hosts(vec![make_host("c"), make_host("b"), make_host("a")]);
    assert_eq!(app.selected_host().unwrap().alias, "b");
}

#[test]
fn test_app_replace_hosts_fallback_when_alias_gone() {
    let mut app = App::new(vec![make_host("a"), make_host("b")]);
    app.selected = 1;
    app.replace_hosts(vec![make_host("x"), make_host("y")]);
    assert_eq!(app.selected, 0);
}

#[test]
fn test_app_take_action_quit_via_q() {
    let mut app = App::new(vec![]);
    app.handle_key(KeyEvent::from(KeyCode::Char('q')));
    assert_eq!(app.take_action(), Some(AppAction::Quit));
}

#[test]
fn test_app_take_action_clears_pending() {
    let mut app = App::new(vec![]);
    app.pending_action = Some(AppAction::Quit);
    assert_eq!(app.take_action(), Some(AppAction::Quit));
    assert!(app.take_action().is_none());
}

#[test]
fn test_app_on_ssh_finished_success_silent() {
    let mut app = App::new(vec![]);
    app.status_message = Some(StatusMessage::new("old"));
    app.on_ssh_finished("web", SshResult::Success);
    assert!(app.status_message.is_none());
}

#[test]
fn test_app_on_ssh_finished_interrupted_silent() {
    let mut app = App::new(vec![]);
    app.status_message = Some(StatusMessage::new("old"));
    app.on_ssh_finished("web", SshResult::Interrupted);
    assert!(app.status_message.is_none());
}

#[test]
fn test_app_on_ssh_finished_connect_failed_sets_message() {
    let mut app = App::new(vec![]);
    app.on_ssh_finished("web", SshResult::ConnectFailed(255));
    let msg = app.status_message.as_ref().unwrap();
    assert!(msg.text().contains("255"));
    assert!(msg.text().contains("web"));
}

#[test]
fn test_app_on_ssh_finished_failed_sets_message() {
    let mut app = App::new(vec![]);
    app.on_ssh_finished("web", SshResult::Failed(1));
    let msg = app.status_message.as_ref().unwrap();
    assert!(msg.text().contains("exit 1"));
}

#[test]
fn test_app_on_ssh_finished_crashed_sets_message() {
    let mut app = App::new(vec![]);
    app.on_ssh_finished("web", SshResult::Crashed(11));
    let msg = app.status_message.as_ref().unwrap();
    assert!(msg.text().contains("signal 11"));
}

#[test]
fn test_app_filter_mode_r_does_not_reconnect() {
    let mut app = App::new(vec![make_host("a"), make_host("b")]);
    app.filter_mode = true;
    app.last_connected = Some("b".to_string());
    app.handle_key(KeyEvent::from(KeyCode::Char('r')));
    assert!(app.pending_action.is_none());
    assert!(app.filter_query.contains('r'));
}

#[test]
fn test_probe_states_sized_with_hosts() {
    let app = App::new(vec![make_host("a"), make_host("b"), make_host("c")]);
    assert_eq!(app.probe_states.len(), 3);
    assert!(app
        .probe_states
        .iter()
        .all(|s| matches!(s, ProbeState::Unknown)));
}

#[test]
fn test_apply_probe_updates_respects_generation() {
    let mut app = App::new(vec![make_host("a"), make_host("b")]);
    app.apply_probe_updates(vec![ProbeUpdate {
        host_idx: 0,
        state: ProbeState::Open,
        generation: 2,
    }]);
    assert_eq!(app.probe_states[0], ProbeState::Open);
    assert_eq!(app.probe_generation, 2);

    // Stale update from generation 1 must NOT overwrite.
    app.apply_probe_updates(vec![ProbeUpdate {
        host_idx: 0,
        state: ProbeState::Failed,
        generation: 1,
    }]);
    assert_eq!(app.probe_states[0], ProbeState::Open);
}

#[test]
fn test_apply_probe_updates_ignores_oob_index() {
    let mut app = App::new(vec![make_host("a")]);
    app.apply_probe_updates(vec![ProbeUpdate {
        host_idx: 99,
        state: ProbeState::Open,
        generation: 1,
    }]);
    assert_eq!(app.probe_states[0], ProbeState::Unknown);
}

#[test]
fn test_tag_filter_at_prefix() {
    let hosts = vec![
        make_host_with_tags("alpha", &["prod"]),
        make_host_with_tags("beta", &["dev"]),
        make_host_with_tags("gamma", &["prod", "api"]),
    ];
    let mut app = App::new(hosts);
    app.filter_query = "@prod".to_string();
    app.apply_filter();
    assert_eq!(app.filtered.len(), 2);
}

#[test]
fn test_tag_filter_at_empty_lists_only_tagged() {
    let hosts = vec![
        make_host_with_tags("alpha", &["prod"]),
        make_host("untagged"),
        make_host_with_tags("gamma", &["dev"]),
    ];
    let mut app = App::new(hosts);
    app.filter_query = "@".to_string();
    app.apply_filter();
    assert_eq!(app.filtered.len(), 2);
}

#[test]
fn test_default_filter_also_matches_tags() {
    let hosts = vec![
        make_host_with_tags("alpha", &["production"]),
        make_host("beta"),
    ];
    let mut app = App::new(hosts);
    app.filter_query = "production".to_string();
    app.apply_filter();
    // alpha matches via tag even though alias is "alpha"
    assert!(!app.filtered.is_empty());
}

#[test]
fn test_is_read_only_reflects_state() {
    let mut app = App::new(vec![]);
    assert!(!app.is_read_only());
    app.state.setup.declined_include_injection = true;
    assert!(app.is_read_only());
}

#[test]
fn test_handle_key_routes_to_modal_when_modal_active() {
    let mut app = App::new(vec![make_host("a")]);
    app.mode = AppMode::Modal(ModalKind::Info {
        message: "hi".into(),
        dismiss: ModalAction::None,
    });
    // Pressing 'q' inside Info should NOT trigger Quit — it should be
    // swallowed by the modal.
    app.handle_key(KeyEvent::from(KeyCode::Char('q')));
    assert!(app.take_action().is_none());
    // Enter dismisses.
    app.handle_key(KeyEvent::from(KeyCode::Enter));
    assert!(matches!(app.mode, AppMode::List));
}

#[test]
fn test_confirmation_modal_yes_dispatches_custom() {
    let mut app = App::new(vec![]);
    app.mode = AppMode::Modal(ModalKind::Confirmation {
        prompt: "ok?".into(),
        on_yes: ModalAction::Custom("inject_include".into()),
        on_no: ModalAction::None,
    });
    app.handle_key(KeyEvent::from(KeyCode::Char('y')));
    assert_eq!(app.take_action(), Some(AppAction::InjectInclude));
    assert!(matches!(app.mode, AppMode::List));
}

#[test]
fn test_confirmation_modal_no_records_decline() {
    let mut app = App::new(vec![]);
    app.mode = AppMode::Modal(ModalKind::Confirmation {
        prompt: "ok?".into(),
        on_yes: ModalAction::Custom("inject_include".into()),
        on_no: ModalAction::Custom("decline_include".into()),
    });
    app.handle_key(KeyEvent::from(KeyCode::Char('n')));
    assert!(app.state.setup.declined_include_injection);
    assert!(app.state.setup.include_check_done);
}

#[test]
fn test_help_modal_open_on_question() {
    let mut app = App::new(vec![make_host("a")]);
    app.handle_key(KeyEvent::from(KeyCode::Char('?')));
    assert!(matches!(app.mode, AppMode::Modal(ModalKind::Info { .. })));
}

#[test]
fn test_toggle_favorite_round_trip() {
    let mut app = App::new(vec![make_host("alpha"), make_host("beta")]);
    assert!(!app.is_favorite("alpha"));
    let pinned = app.toggle_favorite("alpha");
    assert!(pinned);
    assert!(app.is_favorite("alpha"));
    let pinned_again = app.toggle_favorite("alpha");
    assert!(!pinned_again);
    assert!(!app.is_favorite("alpha"));
}

#[test]
fn test_favorite_floats_to_top_of_filter() {
    let mut app = App::new(vec![
        make_host("alpha"),
        make_host("beta"),
        make_host("gamma"),
    ]);
    app.filter_query = "a".to_string();
    app.apply_filter();
    assert!(!app.filtered.is_empty());
    app.toggle_favorite("gamma");
    app.apply_filter();
    let post = app
        .filtered
        .iter()
        .map(|&i| app.hosts[i].alias.clone())
        .collect::<Vec<_>>();
    assert_eq!(post[0], "gamma", "pinned host must float to top");
}

#[test]
fn test_three_tier_sort_favorite_recent_fuzzy() {
    // Hosts: a, b, c. Filter "x" matches none of the aliases, so the
    // bare-query branch returns an empty filtered list — instead use a
    // permissive query that matches all three.
    let mut app = App::new(vec![
        make_host("apple"),
        make_host("banana"),
        make_host("cherry"),
    ]);
    // Record recency in the order: cherry, then banana. So banana > cherry by ts.
    app.state.record_recent("cherry");
    app.state.record_recent("banana");
    // Pin "apple" — favorites always win.
    app.toggle_favorite("apple");
    app.filter_query = "a".to_string(); // matches all three
    app.apply_filter();
    let order = app
        .filtered
        .iter()
        .map(|&i| app.hosts[i].alias.clone())
        .collect::<Vec<_>>();
    assert_eq!(order[0], "apple", "favorite first");
    // The next two must be in recency order: banana (latest), then cherry.
    let rest: Vec<&str> = order[1..].iter().map(|s| s.as_str()).collect();
    assert!(
        rest.starts_with(&["banana", "cherry"]),
        "expected banana before cherry by recency, got {rest:?}"
    );
}

#[test]
fn test_f_key_in_manage_toggles_pin_and_queues_save() {
    let mut h = make_host("managed");
    h.source_file = crate::storage::sshc_conf_path().unwrap_or_default();
    let mut app = App::new(vec![h]);
    app.handle_key(KeyEvent::from(KeyCode::Char('f')));
    assert!(app.is_favorite("managed"));
    assert_eq!(app.take_action(), Some(AppAction::SaveState));
}

// Regression for v0.8.2: `a` (add host) on Windows used to silently leave
// sshc.conf empty because `with_locked_write` opened a second `File::open`
// after `LockFileEx` and tripped ERROR_LOCK_VIOLATION (mandatory locking on
// Windows). The fix reads from the locked handle directly; this test drives
// `apply_form` end-to-end against a temp path so the regression can't slip
// back in unnoticed on any platform.
#[test]
fn test_apply_form_add_host_writes_through_locked_writer() {
    use crate::ui::modal::FormPayload;
    use assert_fs::prelude::*;

    let temp = assert_fs::TempDir::new().unwrap();
    let sshc_conf = temp.child("sshc.conf");
    sshc_conf.touch().unwrap();
    let sshc_path = sshc_conf.path().to_path_buf();

    let mut app = App::new(vec![]);
    app.sshc_conf_path = Some(sshc_path.clone());

    let payload = FormPayload::Host {
        alias: "wintest".to_string(),
        hostname: "1.2.3.4".to_string(),
        user: String::new(),
        port: String::new(),
        identity_file: String::new(),
        tags_csv: String::new(),
        extra: String::new(),
        local_forward: String::new(),
        remote_forward: String::new(),
        dynamic_forward: String::new(),
    };
    app.apply_form(FormContext::AddHost, payload);

    let content = std::fs::read_to_string(&sshc_path).unwrap_or_default();
    assert!(
        content.contains("Host wintest"),
        "sshc.conf must contain 'Host wintest' after apply_form; got {} bytes: {:?}. \
         status_message={:?}",
        content.len(),
        content,
        app.status_message.as_ref().map(|m| m.text().to_string()),
    );
}
