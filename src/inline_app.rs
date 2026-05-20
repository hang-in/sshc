use crate::config::model::Host;
use crate::state::State;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum InlineAction {
    Quit,
    Connect(String),
    Reconnect,
}

/// Lean read-only host browser for v0.4 inline mode. No modal subsystem,
/// no probe column, no forms, no storage writes. Select-and-go semantics.
pub struct InlineApp {
    pub hosts: Vec<Host>,
    pub filtered: Vec<usize>,
    pub selected: usize,
    pub query: String,
    pub last_connected: Option<String>,
    pending_action: Option<InlineAction>,
    matcher: nucleo::Matcher,
}

impl InlineApp {
    pub fn new(hosts: Vec<Host>) -> Self {
        let len = hosts.len();
        let filtered = (0..len).collect();
        Self {
            hosts,
            filtered,
            selected: 0,
            query: String::new(),
            last_connected: None,
            pending_action: None,
            matcher: nucleo::Matcher::new(nucleo::Config::DEFAULT),
        }
    }

    pub fn new_with_state(hosts: Vec<Host>, state: &State) -> Self {
        let mut app = Self::new(hosts);
        // Prefer the v0.6 `recent` head; fall back to the legacy field
        // on first load from a pre-v0.6 state.toml.
        app.last_connected = state
            .memory
            .recent
            .first()
            .map(|e| e.alias.clone())
            .or_else(|| state.memory.last_connected_alias.clone());
        app
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            if let KeyCode::Char('c') = key.code {
                self.pending_action = Some(InlineAction::Quit);
            }
            return;
        }

        match key.code {
            KeyCode::Up => self.previous(),
            KeyCode::Down => self.next(),
            KeyCode::Esc => {
                if !self.query.is_empty() {
                    self.query.clear();
                    self.apply_filter();
                } else {
                    self.pending_action = Some(InlineAction::Quit);
                }
            }
            KeyCode::Enter => {
                if let Some(host) = self.selected_host() {
                    self.pending_action = Some(InlineAction::Connect(host.alias.clone()));
                }
            }
            KeyCode::Backspace => {
                self.query.pop();
                self.apply_filter();
            }
            KeyCode::Char(c) => {
                if self.query.is_empty() {
                    match c {
                        'j' => {
                            self.next();
                            return;
                        }
                        'k' => {
                            self.previous();
                            return;
                        }
                        'r' if self.last_connected.is_some() => {
                            self.pending_action = Some(InlineAction::Reconnect);
                            return;
                        }
                        _ => {}
                    }
                }
                self.query.push(c);
                self.apply_filter();
            }
            _ => {}
        }
    }

    pub fn take_action(&mut self) -> Option<InlineAction> {
        self.pending_action.take()
    }

    pub fn has_pending_action(&self) -> bool {
        self.pending_action.is_some()
    }

    pub fn selected_host(&self) -> Option<&Host> {
        self.filtered
            .get(self.selected)
            .and_then(|&i| self.hosts.get(i))
    }

    pub fn host_count(&self) -> usize {
        self.filtered.len()
    }

    pub fn total_host_count(&self) -> usize {
        self.hosts.len()
    }

    fn next(&mut self) {
        if !self.filtered.is_empty() {
            self.selected = (self.selected + 1) % self.filtered.len();
        }
    }

    fn previous(&mut self) {
        if !self.filtered.is_empty() {
            self.selected = if self.selected == 0 {
                self.filtered.len() - 1
            } else {
                self.selected - 1
            };
        }
    }

    fn apply_filter(&mut self) {
        let q = self.query.clone();
        let mut scored: Vec<(usize, u32)> = self
            .hosts
            .iter()
            .enumerate()
            .filter_map(|(i, h)| {
                let s = h.fuzzy_score(&q, &mut self.matcher);
                if s > 0 {
                    Some((i, s))
                } else {
                    None
                }
            })
            .collect();

        scored.sort_by(|a, b| b.1.cmp(&a.1));
        self.filtered = scored.into_iter().map(|(i, _)| i).collect();

        if self.filtered.is_empty() {
            self.selected = 0;
        } else if self.selected >= self.filtered.len() {
            self.selected = self.filtered.len() - 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_host(alias: &str) -> Host {
        Host {
            alias: alias.to_string(),
            hostname: Some(format!("{}.example.com", alias)),
            user: Some("deploy".to_string()),
            port: Some(22),
            identity_file: None,
            line_start: 1,
            source_file: std::path::PathBuf::from("/test/config"),
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

    #[test]
    fn test_new_initial_state() {
        let hosts = vec![make_host("a"), make_host("b")];
        let app = InlineApp::new(hosts.clone());
        assert_eq!(app.query, "");
        assert_eq!(app.selected, 0);
        assert_eq!(app.filtered.len(), hosts.len());
        assert!(!app.has_pending_action());
        assert!(app.last_connected.is_none());
    }

    #[test]
    fn test_new_with_state_seeds_last_connected() {
        let state = State {
            version: crate::state::CURRENT_VERSION,
            setup: Default::default(),
            memory: crate::state::MemorySection {
                last_connected_alias: Some("foo".to_string()),
                ..Default::default()
            },
        };
        let app = InlineApp::new_with_state(vec![], &state);
        assert_eq!(app.last_connected, Some("foo".to_string()));
    }

    #[test]
    fn test_immediate_filter_appends_to_query() {
        let mut app = InlineApp::new(vec![make_host("apple")]);
        app.handle_key(ke(KeyCode::Char('a')));
        assert_eq!(app.query, "a");
        assert!(!app.filtered.is_empty());
    }

    #[test]
    fn test_backspace_pops_query() {
        let mut app = InlineApp::new(vec![make_host("apple")]);
        app.handle_key(ke(KeyCode::Char('a')));
        app.handle_key(ke(KeyCode::Char('b')));
        app.handle_key(ke(KeyCode::Backspace));
        assert_eq!(app.query, "a");
    }

    #[test]
    fn test_esc_clears_query_when_nonempty() {
        let mut app = InlineApp::new(vec![make_host("apple")]);
        app.handle_key(ke(KeyCode::Char('a')));
        app.handle_key(ke(KeyCode::Esc));
        assert_eq!(app.query, "");
        assert!(!app.has_pending_action());
    }

    #[test]
    fn test_esc_quits_when_empty() {
        let mut app = InlineApp::new(vec![]);
        app.handle_key(ke(KeyCode::Esc));
        assert_eq!(app.take_action(), Some(InlineAction::Quit));
    }

    #[test]
    fn test_ctrl_c_always_quits() {
        let mut app = InlineApp::new(vec![]);
        app.query = "something".to_string();
        app.handle_key(ke_ctrl(KeyCode::Char('c')));
        assert_eq!(app.take_action(), Some(InlineAction::Quit));
    }

    #[test]
    fn test_enter_emits_connect_with_alias() {
        let mut app = InlineApp::new(vec![make_host("host1"), make_host("host2")]);
        app.handle_key(ke(KeyCode::Enter));
        assert_eq!(
            app.take_action(),
            Some(InlineAction::Connect("host1".to_string()))
        );
    }

    #[test]
    fn test_enter_noop_on_empty_filter() {
        let mut app = InlineApp::new(vec![make_host("a")]);
        app.query = "zzzzz".to_string();
        app.apply_filter();
        app.handle_key(ke(KeyCode::Enter));
        assert!(!app.has_pending_action());
    }

    #[test]
    fn test_navigation_wraps() {
        let mut app = InlineApp::new(vec![make_host("1"), make_host("2"), make_host("3")]);
        app.handle_key(ke(KeyCode::Down));
        app.handle_key(ke(KeyCode::Down));
        app.handle_key(ke(KeyCode::Down));
        assert_eq!(app.selected, 0);
        app.handle_key(ke(KeyCode::Up));
        assert_eq!(app.selected, 2);
    }

    #[test]
    fn test_jk_navigate_only_when_query_empty() {
        let mut app = InlineApp::new(vec![make_host("1"), make_host("2")]);
        app.handle_key(ke(KeyCode::Char('j')));
        assert_eq!(app.selected, 1);

        app.query = "a".to_string();
        app.apply_filter();
        app.handle_key(ke(KeyCode::Char('j')));
        assert_eq!(app.query, "aj");
    }

    #[test]
    fn test_r_reconnect_only_when_query_empty_and_last_connected_set() {
        // Case 1: last_connected None, query empty -> 'r' appends to query
        // (per spec: any other Char appends when reconnect precondition fails).
        let mut app = InlineApp::new(vec![]);
        app.handle_key(ke(KeyCode::Char('r')));
        assert!(!app.has_pending_action());
        assert_eq!(app.query, "r");

        // Case 2: last_connected Some, query empty -> Reconnect.
        let mut app = InlineApp::new(vec![]);
        app.last_connected = Some("foo".to_string());
        app.handle_key(ke(KeyCode::Char('r')));
        assert_eq!(app.take_action(), Some(InlineAction::Reconnect));

        // Case 3: last_connected Some, query non-empty -> 'r' appends.
        let mut app = InlineApp::new(vec![]);
        app.last_connected = Some("foo".to_string());
        app.query = "x".to_string();
        app.handle_key(ke(KeyCode::Char('r')));
        assert_eq!(app.query, "xr");
        assert!(!app.has_pending_action());
    }

    #[test]
    fn test_take_action_clears_pending() {
        let mut app = InlineApp::new(vec![]);
        app.handle_key(ke(KeyCode::Esc));
        assert_eq!(app.take_action(), Some(InlineAction::Quit));
        assert_eq!(app.take_action(), None);
    }
}
