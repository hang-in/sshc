use crossterm::event::{KeyCode, KeyEvent};

use crate::config::model::Host;
use crate::exec::ssh::SshResult;
use crate::ui::status_bar::StatusMessage;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum AppAction {
    Quit,
    Connect(String),
    EditConfig,
}

/// Application state for the TUI.
pub struct App {
    pub hosts: Vec<Host>,
    pub filtered: Vec<usize>,
    pub selected: usize,
    pub filter_mode: bool,
    pub filter_query: String,
    pub scroll_offset: usize,
    pub should_quit: bool,
    pub should_connect: bool,
    pub should_edit: bool,
    pub last_connected: Option<String>,
    pub status_message: Option<StatusMessage>,
    pending_action: Option<AppAction>,
    matcher: nucleo::Matcher,
}

impl App {
    pub fn new(hosts: Vec<Host>) -> Self {
        let filtered: Vec<usize> = (0..hosts.len()).collect();
        Self {
            hosts,
            filtered,
            selected: 0,
            filter_mode: false,
            filter_query: String::new(),
            scroll_offset: 0,
            should_quit: false,
            should_connect: false,
            should_edit: false,
            last_connected: None,
            status_message: None,
            pending_action: None,
            matcher: nucleo::Matcher::new(nucleo::Config::DEFAULT),
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        if self.filter_mode {
            match key.code {
                KeyCode::Esc => {
                    self.filter_mode = false;
                    if self.filter_query.is_empty() {
                        self.should_quit = true;
                        self.pending_action = Some(AppAction::Quit);
                    } else {
                        self.filter_query.clear();
                        self.apply_filter();
                    }
                }
                KeyCode::Enter => {
                    self.filter_mode = false;
                    if !self.filtered.is_empty() {
                        self.should_connect = true;
                        if let Some(alias) = self.selected_host().map(|h| h.alias.clone()) {
                            self.pending_action = Some(AppAction::Connect(alias));
                        }
                    }
                }
                KeyCode::Char('k') => self.previous(),
                KeyCode::Char('j') => self.next(),
                KeyCode::Char(c) => {
                    self.filter_query.push(c);
                    self.apply_filter();
                }
                KeyCode::Backspace => {
                    self.filter_query.pop();
                    self.apply_filter();
                }
                KeyCode::Up => self.previous(),
                KeyCode::Down => self.next(),
                _ => {}
            }
        } else {
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => self.previous(),
                KeyCode::Down | KeyCode::Char('j') => self.next(),
                KeyCode::Char('/') => {
                    self.filter_mode = true;
                }
                KeyCode::Enter => {
                    if !self.filtered.is_empty() {
                        self.should_connect = true;
                        if let Some(alias) = self.selected_host().map(|h| h.alias.clone()) {
                            self.pending_action = Some(AppAction::Connect(alias));
                        }
                    }
                }
                KeyCode::Char('e') => {
                    if !self.filtered.is_empty() {
                        self.should_edit = true;
                        self.pending_action = Some(AppAction::EditConfig);
                    }
                }
                KeyCode::Char('r') => {
                    self.try_reconnect();
                }
                KeyCode::Char('q') | KeyCode::Esc => {
                    self.should_quit = true;
                    self.pending_action = Some(AppAction::Quit);
                }
                _ => {}
            }
        }
    }

    /// Drain the pending action. Also clears legacy should_* flags.
    pub fn take_action(&mut self) -> Option<AppAction> {
        let action = self.pending_action.take();
        self.should_quit = false;
        self.should_connect = false;
        self.should_edit = false;
        action
    }

    /// Update status_message based on ssh exit. Silent for Success/Interrupted.
    pub fn on_ssh_finished(&mut self, host_alias: &str, result: SshResult) {
        match result {
            SshResult::Success | SshResult::Interrupted => {
                self.status_message = None;
            }
            SshResult::ConnectFailed(code) => {
                self.status_message = Some(StatusMessage::new(format!(
                    "Connection failed ({}): {}",
                    code, host_alias
                )));
            }
            SshResult::Failed(code) => {
                self.status_message = Some(StatusMessage::new(format!(
                    "ssh exit {}: {}",
                    code, host_alias
                )));
            }
            SshResult::Crashed(sig) => {
                self.status_message = Some(StatusMessage::new(format!(
                    "ssh killed by signal {}: {}",
                    sig, host_alias
                )));
            }
            SshResult::UnknownTermination => {
                self.status_message = Some(StatusMessage::new(format!(
                    "ssh terminated abnormally: {}",
                    host_alias
                )));
            }
        }
    }

    /// Replace host list, preserving selection by alias where possible.
    pub fn replace_hosts(&mut self, new_hosts: Vec<Host>) {
        let prev_alias = self.selected_host().map(|h| h.alias.clone());
        let query = self.filter_query.clone();

        self.hosts = new_hosts;
        self.apply_filter();

        if let Some(alias) = prev_alias {
            if let Some(pos_in_filtered) = self
                .filtered
                .iter()
                .position(|&i| self.hosts[i].alias == alias)
            {
                self.selected = pos_in_filtered;
            } else {
                self.selected = 0;
            }
        } else {
            self.selected = 0;
        }
        self.filter_query = query;
    }

    fn try_reconnect(&mut self) {
        if let Some(alias) = self.last_connected.clone() {
            if self.hosts.iter().any(|h| h.alias == alias) {
                self.pending_action = Some(AppAction::Connect(alias));
                return;
            }
        }
        self.status_message = Some(StatusMessage::new("no recent host to reconnect"));
    }

    fn apply_filter(&mut self) {
        let query = self.filter_query.clone();
        let mut scored: Vec<(usize, u32)> = self
            .hosts
            .iter()
            .enumerate()
            .filter_map(|(i, host)| {
                let score = host.fuzzy_score(&query, &mut self.matcher);
                if score > 0 {
                    Some((i, score))
                } else {
                    None
                }
            })
            .collect();

        scored.sort_by(|a, b| b.1.cmp(&a.1));
        self.filtered = scored.into_iter().map(|(i, _)| i).collect();

        if self.selected >= self.filtered.len() && !self.filtered.is_empty() {
            self.selected = self.filtered.len() - 1;
        }
    }

    pub fn next(&mut self) {
        if !self.filtered.is_empty() {
            self.selected = (self.selected + 1) % self.filtered.len();
            self.adjust_scroll();
        }
    }

    pub fn previous(&mut self) {
        if !self.filtered.is_empty() {
            if self.selected == 0 {
                self.selected = self.filtered.len() - 1;
            } else {
                self.selected -= 1;
            }
            self.adjust_scroll();
        }
    }

    fn adjust_scroll(&mut self) {
        if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        }
    }

    pub fn selected_host(&self) -> Option<&Host> {
        self.filtered
            .get(self.selected)
            .and_then(|&idx| self.hosts.get(idx))
    }

    pub fn host_count(&self) -> usize {
        self.filtered.len()
    }

    pub fn total_host_count(&self) -> usize {
        self.hosts.len()
    }

    pub fn reset_actions(&mut self) {
        self.should_quit = false;
        self.should_connect = false;
        self.should_edit = false;
        self.pending_action = None;
    }

    /// Legacy name kept for v0.1 main.rs compatibility — delegates to replace_hosts.
    /// Removed in T8 (R4) when main.rs is rewritten.
    pub fn refresh_hosts(&mut self, hosts: Vec<Host>) {
        self.replace_hosts(hosts);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        }
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
        assert!(app.should_quit);
        assert_eq!(app.take_action(), Some(AppAction::Quit));
    }

    #[test]
    fn test_app_enter_connect() {
        let hosts = vec![make_host("a")];
        let mut app = App::new(hosts);
        app.handle_key(KeyEvent::from(KeyCode::Enter));
        assert!(app.should_connect);
        assert_eq!(app.take_action(), Some(AppAction::Connect("a".to_string())));
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
    fn test_app_try_reconnect_none_sets_status() {
        let mut app = App::new(vec![]);
        app.last_connected = None;
        app.try_reconnect();
        let msg = app.status_message.as_ref().unwrap();
        assert!(msg.text().contains("no recent host"));
        assert!(app.pending_action.is_none());
    }

    #[test]
    fn test_app_try_reconnect_valid_alias() {
        let mut app = App::new(vec![make_host("a"), make_host("b")]);
        app.last_connected = Some("b".to_string());
        app.try_reconnect();
        assert_eq!(app.take_action(), Some(AppAction::Connect("b".to_string())));
    }

    #[test]
    fn test_app_try_reconnect_alias_gone_sets_status() {
        let mut app = App::new(vec![make_host("a")]);
        app.last_connected = Some("ghost".to_string());
        app.try_reconnect();
        let msg = app.status_message.as_ref().unwrap();
        assert!(msg.text().contains("no recent host"));
        assert!(app.pending_action.is_none());
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
}
