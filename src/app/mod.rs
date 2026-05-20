mod filter;

use crossterm::event::{KeyCode, KeyEvent};

use crate::config::model::Host;
use crate::config::tags::normalize_tag;
use crate::error::{AppError, SetupError};
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
pub(super) enum FormContext {
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
    /// Cached `~/.ssh/config.d/sshc.conf` path. `None` when the home
    /// directory can't be resolved — in that case every host is treated as
    /// "external" (read-only via the TUI), which is correct: we have
    /// nowhere to persist new entries. (Pre-cache version used
    /// `unwrap_or_default()` which produced an empty PathBuf, accidentally
    /// matching hosts whose source_file resolution had also failed.)
    pub(super) sshc_conf_path: Option<std::path::PathBuf>,
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
            sshc_conf_path: crate::storage::sshc_conf_path(),
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
                KeyCode::Enter => self.activate_selected(),
                KeyCode::Char('s') => {
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
                KeyCode::Char('d') => self.open_delete_confirm(),
                KeyCode::Char('t') => self.open_tag_form(),
                KeyCode::Char('i') => {
                    // Force-retry the Include injection. Useful when the user
                    // declined first-run setup and now wants to enable writes,
                    // or to repair a missing Include line. The runtime
                    // (handle_inject_include) flips include_check_done = true
                    // and declined_include_injection = false on success.
                    self.state.setup.declined_include_injection = false;
                    self.pending_action = Some(AppAction::InjectInclude);
                }
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

    pub(super) fn open_add_form(&mut self) {
        if self.is_read_only() {
            self.status_message = Some(StatusMessage::new(
                "read-only — press 'i' to add Include line and enable writes",
            ));
            return;
        }
        let form = crate::ui::forms::HostForm::new();
        self.active_form_context = Some(FormContext::AddHost);
        self.mode = AppMode::Modal(ModalKind::Form(Box::new(form)));
    }

    pub(super) fn open_modify_form(&mut self) {
        if self.is_read_only() {
            self.status_message = Some(StatusMessage::new(
                "read-only — press 'i' to add Include line",
            ));
            return;
        }
        let Some(host) = self.selected_host().cloned() else {
            return;
        };
        if host.source_file != self.sshc_conf_path_or_blank() {
            self.status_message = Some(StatusMessage::new(
                "this host lives outside sshc.conf; press 'e' to edit source",
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
        let extra_joined = host.extra.join("; ");
        let form = crate::ui::forms::HostForm::from_host(
            &host.alias,
            host.hostname.as_deref().unwrap_or(""),
            host.user.as_deref().unwrap_or(""),
            &port_str,
            &identity,
            &tags_csv,
            &extra_joined,
        );
        self.active_form_context = Some(FormContext::EditHost(host.alias.clone()));
        self.mode = AppMode::Modal(ModalKind::Form(Box::new(form)));
    }

    pub(super) fn open_tag_form(&mut self) {
        if self.is_read_only() {
            self.status_message = Some(StatusMessage::new(
                "read-only — press 'i' to add Include line",
            ));
            return;
        }
        let Some(host) = self.selected_host().cloned() else {
            return;
        };
        if host.source_file != self.sshc_conf_path_or_blank() {
            self.status_message = Some(StatusMessage::new(
                "tags can only be edited on sshc.conf hosts",
            ));
            return;
        }
        let initial = host.tags.join(", ");
        let form = crate::ui::forms::TagForm::new(initial);
        self.active_form_context = Some(FormContext::EditTags(host.alias.clone()));
        self.mode = AppMode::Modal(ModalKind::Form(Box::new(form)));
    }

    pub(super) fn open_delete_confirm(&mut self) {
        if self.is_read_only() {
            self.status_message = Some(StatusMessage::new(
                "read-only — press 'i' to add Include line",
            ));
            return;
        }
        let Some(host) = self.selected_host().cloned() else {
            return;
        };
        if host.source_file != self.sshc_conf_path_or_blank() {
            self.status_message = Some(StatusMessage::new("can only delete sshc.conf hosts"));
            return;
        }
        self.mode = AppMode::Modal(ModalKind::Confirmation {
            prompt: format!("Delete host '{}'?", host.alias),
            on_yes: ModalAction::Custom("delete_selected".to_string()),
            on_no: ModalAction::None,
        });
    }

    pub(super) fn open_help_modal(&mut self) {
        let msg = "j/k nav  / filter  Enter open  s ssh  r reconnect\n\
                   a add  d delete  t tags  e edit  i include  ? help  q quit"
            .to_string();
        self.mode = AppMode::Modal(ModalKind::Info {
            message: msg,
            dismiss: ModalAction::None,
        });
    }

    /// v0.4 Enter behaviour: open the edit form for sshc.conf-managed
    /// hosts (manage them); open `$EDITOR` at the host's line for
    /// external-source hosts (sshc.conf-external — cannot be edited via
    /// the form). For empty lists, no-op.
    fn activate_selected(&mut self) {
        if self.filtered.is_empty() {
            return;
        }
        let host_is_managed = self
            .selected_host()
            .map(|h| h.source_file == self.sshc_conf_path_or_blank())
            .unwrap_or(false);
        if host_is_managed {
            self.open_modify_form();
        } else {
            self.pending_action = Some(AppAction::EditConfig);
        }
    }

    pub(super) fn apply_form(&mut self, ctx: FormContext, payload: FormPayload) {
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

    pub(super) fn try_reconnect(&mut self) {
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

    // apply_filter lives in `filter.rs`.

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

    /// True when sshc cannot persist changes to sshc.conf (user declined the
    /// Include injection during first-run setup).
    pub fn is_read_only(&self) -> bool {
        self.state.setup.declined_include_injection
    }

    /// Cached sshc.conf path, or an empty `PathBuf` sentinel when the home
    /// directory couldn't be resolved at App construction. The sentinel
    /// never matches a parser-emitted `source_file`, so the comparison
    /// `host.source_file == self.sshc_conf_path_or_blank()` is correct
    /// (treats every host as external, which is the safe default when we
    /// have nowhere to persist new entries).
    pub(super) fn sshc_conf_path_or_blank(&self) -> std::path::PathBuf {
        self.sshc_conf_path.clone().unwrap_or_default()
    }

    /// Apply an add-host form submission: append to in-memory hosts and
    /// persist via storage::with_locked_write.
    fn apply_add(&mut self, payload: &FormPayload) -> Result<(), AppError> {
        // The caller (apply_form) already matched FormPayload::Host before
        // routing here, so host_from_payload always returns Some.
        let host = host_from_payload(payload, &self.sshc_conf_path_or_blank())
            .expect("apply_form routes Host payloads to apply_add");
        if self.hosts.iter().any(|h| h.alias == host.alias) {
            self.status_message = Some(StatusMessage::new(format!(
                "alias '{}' already exists",
                host.alias
            )));
            return Ok(());
        }
        self.hosts.push(host);
        self.probe_states.push(ProbeState::Unknown);
        self.persist_sshc_conf()?;
        self.apply_filter();
        Ok(())
    }

    fn apply_modify(&mut self, alias: &str, payload: &FormPayload) -> Result<(), AppError> {
        // Caller already matched FormPayload::Host (see apply_add note).
        let new_host = host_from_payload(payload, &self.sshc_conf_path_or_blank())
            .expect("apply_form routes Host payloads to apply_modify");
        if let Some(pos) = self.hosts.iter().position(|h| h.alias == alias) {
            self.hosts[pos] = new_host;
            self.persist_sshc_conf()?;
            self.apply_filter();
        }
        Ok(())
    }

    pub(super) fn apply_delete(&mut self, alias: &str) {
        if let Some(pos) = self.hosts.iter().position(|h| h.alias == alias) {
            self.hosts.remove(pos);
            if pos < self.probe_states.len() {
                self.probe_states.remove(pos);
            }
            if let Err(e) = self.persist_sshc_conf() {
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
            self.persist_sshc_conf()?;
            self.apply_filter();
        }
        Ok(())
    }

    /// Re-render the in-memory hosts that live in sshc.conf and atomically
    /// rewrite the file.
    fn persist_sshc_conf(&self) -> Result<(), AppError> {
        // sshc_conf_path() only returns None when the home directory cannot
        // be resolved. Report that explicitly rather than disguising it as
        // a lock-contention failure.
        let path =
            crate::storage::sshc_conf_path().ok_or(AppError::Setup(SetupError::HomeDirMissing))?;
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
        extra,
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
        let extra_lines: Vec<String> = extra
            .split(';')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();
        Some(Host {
            alias: alias.clone(),
            hostname: Some(hostname.clone()),
            user: user_field,
            port: port_parsed,
            identity_file: identity,
            line_start: 1,
            source_file: source.to_path_buf(),
            tags,
            extra: extra_lines,
        })
    } else {
        None
    }
}

#[cfg(test)]
mod tests;
