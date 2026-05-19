use crossterm::event::{KeyCode, KeyEvent};

use crate::config::model::Host;
use crate::config::tags::normalize_tag;
use crate::error::{AppError, StorageError};
use crate::exec::ssh::SshResult;
use crate::probe::{ProbeState, ProbeUpdate};
use crate::state::State as AppState;
use crate::ui::modal::{FormOutcome, FormPayload, ModalAction, ModalKind};
use crate::ui::status_bar::StatusMessage;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum AppAction {
    Quit,
    Connect(String),
    EditConfig,
    SaveState,
    InjectInclude,
    DeclineInclude,
}

/// Foreground UI mode. `List` is the host browser; `Modal` defers all key
/// events to the active modal (form, confirmation, or info dialog).
pub enum AppMode {
    List,
    Modal(ModalKind),
}

/// Tracks why a form was opened, so the submitted payload can be routed.
#[derive(Debug, Clone)]
enum FormContext {
    AddHost,
    EditHost(String),
    EditTags(String),
}

/// Application state for the TUI.
pub struct App {
    pub hosts: Vec<Host>,
    pub filtered: Vec<usize>,
    pub selected: usize,
    pub filter_mode: bool,
    pub filter_query: String,
    pub scroll_offset: usize,
    pub last_connected: Option<String>,
    pub status_message: Option<StatusMessage>,
    pub mode: AppMode,
    pub probe_states: Vec<ProbeState>,
    pub state: AppState,
    pub probe_generation: u64,
    pending_action: Option<AppAction>,
    active_form_context: Option<FormContext>,
    matcher: nucleo::Matcher,
}

impl App {
    pub fn new(hosts: Vec<Host>) -> Self {
        Self::new_with_state(hosts, AppState::default())
    }

    pub fn new_with_state(hosts: Vec<Host>, state: AppState) -> Self {
        let filtered: Vec<usize> = (0..hosts.len()).collect();
        let probe_states = vec![ProbeState::Unknown; hosts.len()];
        let last_connected = state.memory.last_connected_alias.clone();
        Self {
            hosts,
            filtered,
            selected: 0,
            filter_mode: false,
            filter_query: String::new(),
            scroll_offset: 0,
            last_connected,
            status_message: None,
            mode: AppMode::List,
            probe_states,
            state,
            probe_generation: 0,
            pending_action: None,
            active_form_context: None,
            matcher: nucleo::Matcher::new(nucleo::Config::DEFAULT),
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        match &self.mode {
            AppMode::List => self.handle_list_key(key),
            AppMode::Modal(_) => self.handle_modal_key(key),
        }
    }

    fn handle_list_key(&mut self, key: KeyEvent) {
        if self.filter_mode {
            match key.code {
                KeyCode::Esc => {
                    self.filter_mode = false;
                    if self.filter_query.is_empty() {
                        self.pending_action = Some(AppAction::Quit);
                    } else {
                        self.filter_query.clear();
                        self.apply_filter();
                    }
                }
                KeyCode::Enter => {
                    self.filter_mode = false;
                    if !self.filtered.is_empty() {
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
                        if let Some(alias) = self.selected_host().map(|h| h.alias.clone()) {
                            self.pending_action = Some(AppAction::Connect(alias));
                        }
                    }
                }
                KeyCode::Char('e') => {
                    if !self.filtered.is_empty() {
                        self.pending_action = Some(AppAction::EditConfig);
                    }
                }
                KeyCode::Char('r') => {
                    self.try_reconnect();
                }
                KeyCode::Char('a') => self.open_add_form(),
                KeyCode::Char('m') => self.open_modify_form(),
                KeyCode::Char('d') => self.open_delete_confirm(),
                KeyCode::Char('t') => self.open_tag_form(),
                KeyCode::Char('?') => self.open_help_modal(),
                KeyCode::Char('q') | KeyCode::Esc => {
                    self.pending_action = Some(AppAction::Quit);
                }
                _ => {}
            }
        }
    }

    fn handle_modal_key(&mut self, key: KeyEvent) {
        let mut mode = std::mem::replace(&mut self.mode, AppMode::List);
        match &mut mode {
            AppMode::Modal(ModalKind::Form(form)) => match form.handle_key(key) {
                FormOutcome::Stay => {
                    self.mode = mode;
                }
                FormOutcome::Cancel => {
                    self.active_form_context = None;
                }
                FormOutcome::Submit(payload) => {
                    let ctx = self.active_form_context.take();
                    if let Some(ctx) = ctx {
                        self.apply_form(ctx, payload);
                    }
                }
            },
            AppMode::Modal(ModalKind::Confirmation { on_yes, on_no, .. }) => {
                let action = match key.code {
                    KeyCode::Char('y') | KeyCode::Char('Y') => Some(on_yes.clone()),
                    KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => Some(on_no.clone()),
                    _ => None,
                };
                match action {
                    Some(a) => self.dispatch_modal_action(a),
                    None => self.mode = mode,
                }
            }
            AppMode::Modal(ModalKind::Info { dismiss, .. }) => {
                if matches!(key.code, KeyCode::Enter | KeyCode::Esc) {
                    self.dispatch_modal_action(dismiss.clone());
                } else {
                    self.mode = mode;
                }
            }
            AppMode::List => {
                self.mode = mode;
            }
        }
    }

    fn dispatch_modal_action(&mut self, action: ModalAction) {
        match action {
            ModalAction::None => {}
            ModalAction::Custom(s) => match s.as_str() {
                "inject_include" => {
                    self.pending_action = Some(AppAction::InjectInclude);
                }
                "decline_include" => {
                    self.state.setup.declined_include_injection = true;
                    self.state.setup.include_check_done = true;
                    self.pending_action = Some(AppAction::DeclineInclude);
                }
                "delete_selected" => {
                    if let Some(alias) = self.selected_host().map(|h| h.alias.clone()) {
                        self.apply_delete(&alias);
                    }
                }
                _ => {}
            },
        }
    }

    pub fn exit_modal(&mut self) {
        self.mode = AppMode::List;
        self.active_form_context = None;
    }

    fn open_add_form(&mut self) {
        if self.is_read_only() {
            self.status_message = Some(StatusMessage::new(
                "read-only: sshs.conf is not Included by main ssh_config",
            ));
            return;
        }
        let form = crate::ui::forms::HostForm::new();
        self.active_form_context = Some(FormContext::AddHost);
        self.mode = AppMode::Modal(ModalKind::Form(Box::new(form)));
    }

    fn open_modify_form(&mut self) {
        if self.is_read_only() {
            self.status_message = Some(StatusMessage::new("read-only"));
            return;
        }
        let Some(host) = self.selected_host().cloned() else {
            return;
        };
        if host.source_file != Self::sshs_conf_path_or_blank() {
            self.status_message = Some(StatusMessage::new(
                "this host lives outside sshs.conf; press 'e' to edit source",
            ));
            return;
        }
        let port_str = host.port.map(|p| p.to_string()).unwrap_or_default();
        let identity = host
            .identity_file
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        let tags_csv = host.tags.join(", ");
        let form = crate::ui::forms::HostForm::from_host(
            &host.alias,
            host.hostname.as_deref().unwrap_or(""),
            host.user.as_deref().unwrap_or(""),
            &port_str,
            &identity,
            &tags_csv,
        );
        self.active_form_context = Some(FormContext::EditHost(host.alias.clone()));
        self.mode = AppMode::Modal(ModalKind::Form(Box::new(form)));
    }

    fn open_tag_form(&mut self) {
        if self.is_read_only() {
            self.status_message = Some(StatusMessage::new("read-only"));
            return;
        }
        let Some(host) = self.selected_host().cloned() else {
            return;
        };
        if host.source_file != Self::sshs_conf_path_or_blank() {
            self.status_message = Some(StatusMessage::new(
                "tags can only be edited on sshs.conf hosts",
            ));
            return;
        }
        let initial = host.tags.join(", ");
        let form = crate::ui::forms::TagForm::new(initial);
        self.active_form_context = Some(FormContext::EditTags(host.alias.clone()));
        self.mode = AppMode::Modal(ModalKind::Form(Box::new(form)));
    }

    fn open_delete_confirm(&mut self) {
        if self.is_read_only() {
            self.status_message = Some(StatusMessage::new("read-only"));
            return;
        }
        let Some(host) = self.selected_host().cloned() else {
            return;
        };
        if host.source_file != Self::sshs_conf_path_or_blank() {
            self.status_message = Some(StatusMessage::new("can only delete sshs.conf hosts"));
            return;
        }
        self.mode = AppMode::Modal(ModalKind::Confirmation {
            prompt: format!("Delete host '{}'?", host.alias),
            on_yes: ModalAction::Custom("delete_selected".to_string()),
            on_no: ModalAction::None,
        });
    }

    fn open_help_modal(&mut self) {
        let msg = "j/k nav  / filter  Enter ssh  r reconnect\n\
                   a add  d delete  m modify  t tags  e edit  ? help  q quit"
            .to_string();
        self.mode = AppMode::Modal(ModalKind::Info {
            message: msg,
            dismiss: ModalAction::None,
        });
    }

    fn apply_form(&mut self, ctx: FormContext, payload: FormPayload) {
        let result = match (ctx, &payload) {
            (FormContext::AddHost, FormPayload::Host { .. }) => self.apply_add(&payload),
            (FormContext::EditHost(alias), FormPayload::Host { .. }) => {
                self.apply_modify(&alias, &payload)
            }
            (FormContext::EditTags(alias), FormPayload::Tags { tags_csv }) => {
                self.apply_tags(&alias, tags_csv)
            }
            _ => Ok(()),
        };
        match result {
            Ok(()) => {
                self.pending_action = Some(AppAction::SaveState);
            }
            Err(e) => {
                self.status_message = Some(StatusMessage::new(format!("form apply failed: {e}")));
            }
        }
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

    /// Drain the pending action.
    pub fn take_action(&mut self) -> Option<AppAction> {
        self.pending_action.take()
    }

    pub fn has_pending_action(&self) -> bool {
        self.pending_action.is_some()
    }

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

    pub fn replace_hosts(&mut self, new_hosts: Vec<Host>) {
        let prev_alias = self.selected_host().map(|h| h.alias.clone());
        let query = self.filter_query.clone();

        self.hosts = new_hosts;
        self.probe_states = vec![ProbeState::Unknown; self.hosts.len()];
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

    /// Apply a batch of probe updates. Updates with older generations are
    /// dropped; newer generations roll forward the current_generation marker.
    pub fn apply_probe_updates(&mut self, updates: Vec<ProbeUpdate>) {
        for u in updates {
            if u.generation < self.probe_generation {
                continue;
            }
            if u.generation > self.probe_generation {
                self.probe_generation = u.generation;
            }
            if let Some(slot) = self.probe_states.get_mut(u.host_idx) {
                *slot = u.state;
            }
        }
    }

    fn apply_filter(&mut self) {
        let query = self.filter_query.clone();

        if let Some(tag_query) = query.strip_prefix('@') {
            let needle = tag_query.trim().to_lowercase();
            self.filtered = self
                .hosts
                .iter()
                .enumerate()
                .filter(|(_, h)| {
                    if needle.is_empty() {
                        !h.tags.is_empty()
                    } else {
                        h.tags.iter().any(|t| t.contains(&needle))
                    }
                })
                .map(|(i, _)| i)
                .collect();
        } else {
            let needle = query.to_lowercase();
            let mut scored: Vec<(usize, u32)> = self
                .hosts
                .iter()
                .enumerate()
                .filter_map(|(i, host)| {
                    let score = host.fuzzy_score(&query, &mut self.matcher);
                    let tag_match =
                        !needle.is_empty() && host.tags.iter().any(|t| t.contains(&needle));
                    let best = if tag_match { score.max(1) } else { score };
                    if best > 0 {
                        Some((i, best))
                    } else {
                        None
                    }
                })
                .collect();
            scored.sort_by(|a, b| b.1.cmp(&a.1));
            self.filtered = scored.into_iter().map(|(i, _)| i).collect();
        }

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

    /// True when sshs cannot persist changes to sshs.conf (user declined the
    /// Include injection during first-run setup).
    pub fn is_read_only(&self) -> bool {
        self.state.setup.declined_include_injection
    }

    fn sshs_conf_path_or_blank() -> std::path::PathBuf {
        crate::storage::sshs_conf_path().unwrap_or_default()
    }

    /// Apply an add-host form submission: append to in-memory hosts and
    /// persist via storage::with_locked_write.
    fn apply_add(&mut self, payload: &FormPayload) -> Result<(), AppError> {
        let host = host_from_payload(payload, &Self::sshs_conf_path_or_blank())
            .ok_or(AppError::Storage(StorageError::LockHeldByOther))?;
        if self.hosts.iter().any(|h| h.alias == host.alias) {
            self.status_message = Some(StatusMessage::new(format!(
                "alias '{}' already exists",
                host.alias
            )));
            return Ok(());
        }
        self.hosts.push(host);
        self.probe_states.push(ProbeState::Unknown);
        self.persist_sshs_conf()?;
        self.apply_filter();
        Ok(())
    }

    fn apply_modify(&mut self, alias: &str, payload: &FormPayload) -> Result<(), AppError> {
        let new_host = host_from_payload(payload, &Self::sshs_conf_path_or_blank())
            .ok_or(AppError::Storage(StorageError::LockHeldByOther))?;
        if let Some(pos) = self.hosts.iter().position(|h| h.alias == alias) {
            self.hosts[pos] = new_host;
            self.persist_sshs_conf()?;
            self.apply_filter();
        }
        Ok(())
    }

    fn apply_delete(&mut self, alias: &str) {
        if let Some(pos) = self.hosts.iter().position(|h| h.alias == alias) {
            self.hosts.remove(pos);
            if pos < self.probe_states.len() {
                self.probe_states.remove(pos);
            }
            if let Err(e) = self.persist_sshs_conf() {
                self.status_message = Some(StatusMessage::new(format!("delete failed: {e}")));
            } else {
                self.apply_filter();
                self.pending_action = Some(AppAction::SaveState);
            }
        }
    }

    fn apply_tags(&mut self, alias: &str, tags_csv: &str) -> Result<(), AppError> {
        let normalized: Vec<String> =
            tags_csv
                .split(',')
                .filter_map(normalize_tag)
                .fold(Vec::new(), |mut acc, t| {
                    if !acc.contains(&t) {
                        acc.push(t);
                    }
                    acc
                });
        if let Some(host) = self.hosts.iter_mut().find(|h| h.alias == alias) {
            host.tags = normalized;
            self.persist_sshs_conf()?;
            self.apply_filter();
        }
        Ok(())
    }

    /// Re-render the in-memory hosts that live in sshs.conf and atomically
    /// rewrite the file.
    fn persist_sshs_conf(&self) -> Result<(), AppError> {
        let path = crate::storage::sshs_conf_path()
            .ok_or(AppError::Storage(StorageError::LockHeldByOther))?;
        let owned_hosts: Vec<Host> = self
            .hosts
            .iter()
            .filter(|h| h.source_file == path)
            .cloned()
            .collect();
        crate::storage::with_locked_write(&path, true, |_| {
            crate::storage::host_blocks_to_text(&owned_hosts)
        })?;
        Ok(())
    }
}

fn host_from_payload(payload: &FormPayload, source: &std::path::Path) -> Option<Host> {
    if let FormPayload::Host {
        alias,
        hostname,
        user,
        port,
        identity_file,
        tags_csv,
    } = payload
    {
        let port_parsed: Option<u16> = if port.is_empty() {
            None
        } else {
            port.parse().ok()
        };
        let identity = if identity_file.is_empty() {
            None
        } else {
            Some(std::path::PathBuf::from(identity_file))
        };
        let user_field = if user.is_empty() {
            None
        } else {
            Some(user.clone())
        };
        let tags: Vec<String> =
            tags_csv
                .split(',')
                .filter_map(normalize_tag)
                .fold(Vec::new(), |mut acc, t| {
                    if !acc.contains(&t) {
                        acc.push(t);
                    }
                    acc
                });
        Some(Host {
            alias: alias.clone(),
            hostname: Some(hostname.clone()),
            user: user_field,
            port: port_parsed,
            identity_file: identity,
            line_start: 1,
            source_file: source.to_path_buf(),
            tags,
        })
    } else {
        None
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
            tags: Vec::new(),
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
    fn test_app_enter_connect() {
        let hosts = vec![make_host("a")];
        let mut app = App::new(hosts);
        app.handle_key(KeyEvent::from(KeyCode::Enter));
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
}
