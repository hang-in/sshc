mod filter;
mod forms;
mod input;

use crate::config::model::Host;
use crate::exec::ssh::SshResult;
use crate::probe::{ProbeState, ProbeUpdate};
use crate::state::State as AppState;
use crate::ui::modal::ModalKind;
use crate::ui::status_bar::StatusMessage;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum AppAction {
    Quit,
    Connect(String),
    EditConfig,
    SaveState,
    InjectInclude,
    DeclineInclude,
    /// User pressed `M` on an external host: open the add/modify form
    /// pre-filled with that host's fields so the saved entry lands in
    /// `sshc.conf`. The original `~/.ssh/config` entry is left intact —
    /// anti-feature 1 forbids sshc from rewriting user-authored config.
    /// Carries the alias so the runtime can look the host up after the
    /// list state has potentially shifted.
    OpenPromoteForm(String),
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
    /// Promote an external host (originally from `~/.ssh/config` or one
    /// of its Includes) into a fresh sshc.conf entry. Save semantics
    /// match `AddHost`; only the status message differs so the user
    /// understands the original entry was left intact.
    PromoteHost(String),
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
    /// v0.6: session-only cache of `ssh -G <alias>` output, keyed by
    /// alias. Cleared on any apply_form / apply_delete / apply_tags
    /// success so we never show stale resolution after edits.
    pub(super) validation_cache: std::collections::HashMap<String, String>,
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
        // Prefer the v0.6 `recent` head; fall back to the legacy field on
        // first load from a pre-v0.6 state.toml.
        let last_connected = state
            .memory
            .recent
            .first()
            .map(|e| e.alias.clone())
            .or_else(|| state.memory.last_connected_alias.clone());
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
            validation_cache: std::collections::HashMap::new(),
            pending_action: None,
            active_form_context: None,
            matcher: nucleo::Matcher::new(nucleo::Config::DEFAULT),
        }
    }

    pub fn exit_modal(&mut self) {
        self.mode = AppMode::List;
        self.active_form_context = None;
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

    /// True when the currently-selected host lives outside `sshc.conf`
    /// (i.e. in `~/.ssh/config` or one of its `Include`d files). UI
    /// surfaces use this to gate hints / actions that only apply to
    /// external sources — notably the `M promote` hint in the status
    /// bar and the keystroke wired in `app::input::promote_selected`.
    /// Returns false when the list is empty or no host is selected.
    pub fn selected_is_external(&self) -> bool {
        self.selected_host()
            .map(|h| h.source_file != self.sshc_conf_path_or_blank())
            .unwrap_or(false)
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

    /// Returns true when `alias` is in `state.memory.favorites`.
    pub fn is_favorite(&self, alias: &str) -> bool {
        self.state.memory.favorites.iter().any(|a| a == alias)
    }

    /// Run `ssh -G <selected_alias>` (cached per session) and open the
    /// resolved config in an Info modal. No network I/O — `ssh -G` only
    /// parses local config. Errors land in the status bar; nothing
    /// blocks the user from continuing.
    pub(super) fn validate_selected(&mut self) {
        let Some(alias) = self.selected_host().map(|h| h.alias.clone()) else {
            return;
        };
        let output = if let Some(cached) = self.validation_cache.get(&alias) {
            cached.clone()
        } else {
            self.status_message = Some(StatusMessage::new(format!("Validating {alias}…")));
            match crate::exec::ssh_config::validate_alias(&alias) {
                Ok(out) => {
                    self.validation_cache.insert(alias.clone(), out.clone());
                    out
                }
                Err(e) => {
                    self.status_message = Some(StatusMessage::new(format!("ssh -G failed: {e}")));
                    return;
                }
            }
        };
        self.status_message = None;
        self.mode = AppMode::Modal(ModalKind::Info {
            message: format!("ssh -G {alias}\n\n{output}"),
            dismiss: crate::ui::modal::ModalAction::None,
        });
    }

    /// Toggle a host's pinned status. Returns `true` if the host is now
    /// pinned, `false` if it was removed. Caller is responsible for
    /// queuing `AppAction::SaveState` and re-sorting via `apply_filter`.
    pub(super) fn toggle_favorite(&mut self, alias: &str) -> bool {
        if let Some(pos) = self.state.memory.favorites.iter().position(|a| a == alias) {
            self.state.memory.favorites.remove(pos);
            false
        } else {
            self.state.memory.favorites.push(alias.to_string());
            true
        }
    }
}

#[cfg(test)]
mod tests;
