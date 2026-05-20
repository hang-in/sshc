use crate::config::model::Host;
use crate::state::State;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum InlineAction {
    Quit,
    Connect(String),
}

/// Lean read-only host browser for v0.4 inline mode. No modal subsystem,
/// no probe column, no forms, no storage writes. Select-and-go semantics.
pub struct InlineApp {
    pub hosts: Vec<Host>,
    pub filtered: Vec<usize>,
    pub selected: usize,
    pub query: String,
    /// v0.6: inline is now modal like manage. `j/k/↑/↓/r/Enter/q/Esc`
    /// are nav/action keys until the user explicitly enters search mode
    /// with `/`. Inside search mode, printable chars filter; Esc exits
    /// search mode (keeping the picker open); Enter still ssh-launches
    /// the highlighted host.
    pub filter_mode: bool,
    pub last_connected: Option<String>,
    /// Snapshot of `state.memory.favorites` at construction. Inline mode
    /// is read-only, so this is never mutated after `new_with_state`.
    pub favorites: Vec<String>,
    /// Snapshot of `(alias, ts)` from `state.memory.recent`. Used as the
    /// secondary sort key (recency descending). Captured at construction
    /// for the same reason as `favorites`.
    pub recent: Vec<(String, u64)>,
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
            filter_mode: false,
            last_connected: None,
            favorites: Vec::new(),
            recent: Vec::new(),
            pending_action: None,
            matcher: nucleo::Matcher::new(nucleo::Config::DEFAULT),
        }
    }

    pub fn new_with_state(hosts: Vec<Host>, state: &State) -> Self {
        let mut app = Self::new(hosts);
        app.last_connected = state
            .memory
            .recent
            .first()
            .map(|e| e.alias.clone())
            .or_else(|| state.memory.last_connected_alias.clone());
        app.favorites = state.memory.favorites.clone();
        app.recent = state
            .memory
            .recent
            .iter()
            .map(|e| (e.alias.clone(), e.ts))
            .collect();
        app.apply_filter();
        app
    }

    pub fn is_favorite(&self, alias: &str) -> bool {
        self.favorites.iter().any(|a| a == alias)
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        // Ctrl+C always quits, regardless of mode.
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            if let KeyCode::Char('c') = key.code {
                self.pending_action = Some(InlineAction::Quit);
            }
            return;
        }

        if self.filter_mode {
            self.handle_key_filter(key);
        } else {
            self.handle_key_nav(key);
        }
    }

    fn handle_key_nav(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => self.previous(),
            KeyCode::Down | KeyCode::Char('j') => self.next(),
            KeyCode::Char('/') => {
                self.filter_mode = true;
                // Re-apply so the empty-query "match all" branch reseats
                // the filtered list to every host before the user types.
                self.apply_filter();
            }
            // v0.6: `r` reconnect dropped from inline. Recent-history sort
            // puts last_connected at row 0 anyway, so Enter is the same
            // keystroke count.
            KeyCode::Enter => {
                if let Some(host) = self.selected_host() {
                    self.pending_action = Some(InlineAction::Connect(host.alias.clone()));
                }
            }
            KeyCode::Char('q') | KeyCode::Esc => {
                self.pending_action = Some(InlineAction::Quit);
            }
            _ => {}
        }
    }

    fn handle_key_filter(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up => self.previous(),
            KeyCode::Down => self.next(),
            KeyCode::Esc => {
                self.query.clear();
                self.filter_mode = false;
                self.apply_filter();
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
        let favorites: std::collections::HashSet<&str> =
            self.favorites.iter().map(|s| s.as_str()).collect();
        let recency: std::collections::HashMap<&str, u64> =
            self.recent.iter().map(|(a, t)| (a.as_str(), *t)).collect();
        // When the query is empty, fuzzy_score returns 0 for every host,
        // which drops them all. Treat empty query as "everything in".
        let mut scored: Vec<(usize, u32)> = if q.is_empty() {
            self.hosts.iter().enumerate().map(|(i, _)| (i, 1)).collect()
        } else {
            self.hosts
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
                .collect()
        };

        scored.sort_by(|a, b| {
            let fa = favorites.contains(self.hosts[a.0].alias.as_str());
            let fb = favorites.contains(self.hosts[b.0].alias.as_str());
            let ta = recency
                .get(self.hosts[a.0].alias.as_str())
                .copied()
                .unwrap_or(0);
            let tb = recency
                .get(self.hosts[b.0].alias.as_str())
                .copied()
                .unwrap_or(0);
            fb.cmp(&fa).then(tb.cmp(&ta)).then(b.1.cmp(&a.1))
        });
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
        assert!(!app.filter_mode);
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
    fn test_jk_always_navigate_in_nav_mode() {
        let mut app = InlineApp::new(vec![make_host("a"), make_host("b"), make_host("c")]);
        app.handle_key(ke(KeyCode::Char('j')));
        assert_eq!(app.selected, 1);
        app.handle_key(ke(KeyCode::Char('k')));
        assert_eq!(app.selected, 0);
        assert_eq!(app.query, "", "j/k must not append in nav mode");
    }

    #[test]
    fn test_slash_enters_filter_mode_and_typing_appends() {
        let mut app = InlineApp::new(vec![make_host("apple"), make_host("banana")]);
        app.handle_key(ke(KeyCode::Char('/')));
        assert!(app.filter_mode);
        app.handle_key(ke(KeyCode::Char('a')));
        assert_eq!(app.query, "a");
        // Pre-filter 'j' would have navigated; once filter mode is on
        // every printable char appends.
        app.handle_key(ke(KeyCode::Char('j')));
        assert_eq!(app.query, "aj");
    }

    #[test]
    fn test_esc_in_filter_mode_exits_to_nav_and_clears_query() {
        let mut app = InlineApp::new(vec![make_host("apple")]);
        app.handle_key(ke(KeyCode::Char('/')));
        app.handle_key(ke(KeyCode::Char('a')));
        app.handle_key(ke(KeyCode::Esc));
        assert!(!app.filter_mode);
        assert_eq!(app.query, "");
        assert!(!app.has_pending_action());
    }

    #[test]
    fn test_esc_in_nav_mode_quits() {
        let mut app = InlineApp::new(vec![]);
        app.handle_key(ke(KeyCode::Esc));
        assert_eq!(app.take_action(), Some(InlineAction::Quit));
    }

    #[test]
    fn test_q_in_nav_mode_quits() {
        let mut app = InlineApp::new(vec![make_host("a")]);
        app.handle_key(ke(KeyCode::Char('q')));
        assert_eq!(app.take_action(), Some(InlineAction::Quit));
    }

    #[test]
    fn test_q_in_filter_mode_appends() {
        let mut app = InlineApp::new(vec![make_host("apple")]);
        app.handle_key(ke(KeyCode::Char('/')));
        app.handle_key(ke(KeyCode::Char('q')));
        assert_eq!(app.query, "q");
        assert!(!app.has_pending_action());
    }

    #[test]
    fn test_ctrl_c_quits_in_either_mode() {
        let mut app = InlineApp::new(vec![]);
        app.handle_key(ke(KeyCode::Char('/')));
        app.query = "something".to_string();
        app.handle_key(ke_ctrl(KeyCode::Char('c')));
        assert_eq!(app.take_action(), Some(InlineAction::Quit));
    }

    #[test]
    fn test_enter_emits_connect_with_alias_in_nav_mode() {
        let mut app = InlineApp::new(vec![make_host("host1"), make_host("host2")]);
        app.handle_key(ke(KeyCode::Enter));
        assert_eq!(
            app.take_action(),
            Some(InlineAction::Connect("host1".to_string()))
        );
    }

    #[test]
    fn test_enter_in_filter_mode_also_connects() {
        let mut app = InlineApp::new(vec![make_host("apple")]);
        app.handle_key(ke(KeyCode::Char('/')));
        app.handle_key(ke(KeyCode::Char('a')));
        app.handle_key(ke(KeyCode::Enter));
        assert_eq!(
            app.take_action(),
            Some(InlineAction::Connect("apple".to_string()))
        );
    }

    #[test]
    fn test_enter_noop_when_filter_matches_nothing() {
        let mut app = InlineApp::new(vec![make_host("a")]);
        app.handle_key(ke(KeyCode::Char('/')));
        app.handle_key(ke(KeyCode::Char('z')));
        app.handle_key(ke(KeyCode::Char('z')));
        app.handle_key(ke(KeyCode::Enter));
        assert!(!app.has_pending_action());
    }

    #[test]
    fn test_arrow_keys_navigate_in_either_mode() {
        let mut app = InlineApp::new(vec![make_host("1"), make_host("2"), make_host("3")]);
        // Nav mode arrows.
        app.handle_key(ke(KeyCode::Down));
        assert_eq!(app.selected, 1);
        // Filter mode arrows.
        app.handle_key(ke(KeyCode::Char('/')));
        app.handle_key(ke(KeyCode::Down));
        assert_eq!(app.selected, 2);
    }

    #[test]
    fn test_backspace_pops_in_filter_mode() {
        let mut app = InlineApp::new(vec![make_host("apple")]);
        app.handle_key(ke(KeyCode::Char('/')));
        app.handle_key(ke(KeyCode::Char('a')));
        app.handle_key(ke(KeyCode::Char('b')));
        app.handle_key(ke(KeyCode::Backspace));
        assert_eq!(app.query, "a");
    }

    #[test]
    fn test_take_action_clears_pending() {
        let mut app = InlineApp::new(vec![]);
        app.handle_key(ke(KeyCode::Esc));
        assert_eq!(app.take_action(), Some(InlineAction::Quit));
        assert_eq!(app.take_action(), None);
    }
}
