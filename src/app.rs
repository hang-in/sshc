use crossterm::event::{KeyCode, KeyEvent};

use crate::config::model::Host;

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
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        if self.filter_mode {
            match key.code {
                KeyCode::Esc => {
                    self.filter_mode = false;
                    if self.filter_query.is_empty() {
                        self.should_quit = true;
                    } else {
                        self.filter_query.clear();
                        self.apply_filter();
                    }
                }
                KeyCode::Enter => {
                    self.filter_mode = false;
                    if !self.filtered.is_empty() {
                        self.should_connect = true;
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
                    }
                }
                KeyCode::Char('e') => {
                    if !self.filtered.is_empty() {
                        self.should_edit = true;
                    }
                }
                KeyCode::Char('q') | KeyCode::Esc => {
                    self.should_quit = true;
                }
                _ => {}
            }
        }
    }

    fn apply_filter(&mut self) {
        self.filtered = self
            .hosts
            .iter()
            .enumerate()
            .filter(|(_, host)| host.fuzzy_match(&self.filter_query))
            .map(|(i, _)| i)
            .collect();
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
        // Keep selection visible in the viewport (assume viewport height will be set during render)
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

    /// Resets action flags for re-entering the TUI after an editor session.
    pub fn reset_actions(&mut self) {
        self.should_quit = false;
        self.should_connect = false;
        self.should_edit = false;
    }

    /// Re-parses config and refreshes the host list.
    pub fn refresh_hosts(&mut self, hosts: Vec<Host>) {
        let query = self.filter_query.clone();
        self.hosts = hosts;
        self.filtered = (0..self.hosts.len()).collect();
        self.apply_filter_with_query(&query);
    }

    fn apply_filter_with_query(&mut self, query: &str) {
        self.filter_query = query.to_string();
        self.apply_filter();
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

        app.next(); // wraps around
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
        assert_eq!(app.hosts[app.filtered[0]].alias, "web");
        assert_eq!(app.hosts[app.filtered[1]].alias, "web-prod");
    }

    #[test]
    fn test_app_quit_on_esc_without_filter() {
        let hosts = vec![make_host("a")];
        let mut app = App::new(hosts);
        app.handle_key(KeyEvent::from(KeyCode::Esc));
        assert!(app.should_quit);
    }

    #[test]
    fn test_app_enter_connect() {
        let hosts = vec![make_host("a")];
        let mut app = App::new(hosts);
        app.handle_key(KeyEvent::from(KeyCode::Enter));
        assert!(app.should_connect);
    }
}
