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

/// v0.10 G5: which secondary key sorts the host list when the user
/// hasn't typed a fuzzy filter. Favorites always sort first regardless.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortAxis {
    /// Alphabetical by alias (default — matches the v0.6 behavior
    /// before the sort key was a thing).
    #[default]
    AliasAlpha,
    /// Most recently connected first, then everything else after.
    RecentDesc,
    /// `ProbeState::Open` first, then `InFlight`, `Unknown`, `Failed`.
    /// Lets the user see the hosts that are reachable right now.
    ProbeStateOpenFirst,
}

impl SortAxis {
    /// Display label used in the status bar after `S` cycles the axis.
    pub fn label(self) -> &'static str {
        match self {
            SortAxis::AliasAlpha => "alias",
            SortAxis::RecentDesc => "recent",
            SortAxis::ProbeStateOpenFirst => "reachability",
        }
    }

    /// Move to the next axis in the cycle.
    pub fn next(self) -> Self {
        match self {
            SortAxis::AliasAlpha => SortAxis::RecentDesc,
            SortAxis::RecentDesc => SortAxis::ProbeStateOpenFirst,
            SortAxis::ProbeStateOpenFirst => SortAxis::AliasAlpha,
        }
    }

    /// v0.12 G3: convert from the state.toml-side enum. Pure mapping;
    /// state crate doesn't know about SortAxis (R-G6).
    pub fn from_persisted(p: crate::state::schema::SortAxisPersisted) -> Self {
        use crate::state::schema::SortAxisPersisted as P;
        match p {
            P::Alias => SortAxis::AliasAlpha,
            P::Recent => SortAxis::RecentDesc,
            P::Reachability => SortAxis::ProbeStateOpenFirst,
        }
    }

    /// v0.12 G3: convert into the state.toml-side enum for persistence.
    pub fn to_persisted(self) -> crate::state::schema::SortAxisPersisted {
        use crate::state::schema::SortAxisPersisted as P;
        match self {
            SortAxis::AliasAlpha => P::Alias,
            SortAxis::RecentDesc => P::Recent,
            SortAxis::ProbeStateOpenFirst => P::Reachability,
        }
    }
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
    /// v0.10 G5: secondary sort axis for the host list when the
    /// fuzzy filter query is empty. Cycles on `S` in manage mode.
    /// Session-only — not persisted across sshc invocations (the
    /// v0.11 decision after user feedback).
    pub(super) sort_axis: SortAxis,
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
        // v0.12 G3: pull the persisted sort axis out before `state`
        // gets moved into the struct. Pre-v0.12 state.toml has no
        // `sort_axis` key → `#[serde(default)]` returns `Alias`.
        let sort_axis = SortAxis::from_persisted(state.memory.sort_axis);
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
            sort_axis,
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

    /// v0.10 G5: advance the secondary sort axis to the next value in
    /// the cycle and re-run the filter so the host list re-orders
    /// immediately. Emits an Info status with the new label.
    pub(super) fn cycle_sort_axis(&mut self) {
        self.sort_axis = self.sort_axis.next();
        // v0.12 G3: persist the new axis so a fresh sshc session
        // resumes with the user's preference. save() is best-effort
        // — if the write fails, the in-memory cycle still applies
        // and the next successful save catches up. We don't surface
        // a state-write error here because the Info hint below has
        // already taken the status bar slot.
        self.state.memory.sort_axis = self.sort_axis.to_persisted();
        let _ = crate::state::save(&self.state);
        self.apply_filter();
        self.status_message = Some(StatusMessage::new(format!(
            "sorted by {}",
            self.sort_axis.label()
        )));
    }

    /// v0.9 G3: drop a sticky-Error status the moment the user takes
    /// any next keystroke. Called from the top of the keystroke
    /// dispatcher (`app::input::handle_key`). Info messages keep
    /// their own time-based expiry; Error messages exist only until
    /// the user acknowledges them by typing something.
    pub fn clear_sticky_error_status(&mut self) {
        if let Some(msg) = &self.status_message {
            if msg.kind() == crate::ui::status_bar::StatusKind::Error {
                self.status_message = None;
            }
        }
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

    /// v0.9 G6: pressing `g` on a selected host runs `ssh -G <alias>` to
    /// learn the effective hostname/port, then attempts a raw TCP
    /// connect (no SSH handshake). Distinguishes "host is down" from
    /// "ssh config is wrong" without spawning ssh itself.
    ///
    /// Reuses the same `ssh -G` cache that `v`/`c` populate, so
    /// repeated presses don't re-spawn.
    pub(super) fn reach_check_for_selected(&mut self) {
        let Some(alias) = self.selected_host().map(|h| h.alias.clone()) else {
            return;
        };
        let resolved = if let Some(cached) = self.validation_cache.get(&alias) {
            cached.clone()
        } else {
            match crate::exec::ssh_config::validate_alias(&alias) {
                Ok(out) => {
                    self.validation_cache.insert(alias.clone(), out.clone());
                    out
                }
                Err(e) => {
                    self.status_message = Some(StatusMessage::error(format!("ssh -G failed: {e}")));
                    return;
                }
            }
        };
        let (Some(hostname), Some(port)) = (
            resolved
                .lines()
                .find_map(|l| l.strip_prefix("hostname ").map(|s| s.trim().to_string())),
            resolved.lines().find_map(|l| {
                l.strip_prefix("port ")
                    .and_then(|s| s.trim().parse::<u16>().ok())
            }),
        ) else {
            self.status_message = Some(StatusMessage::error(format!(
                "could not parse hostname/port from ssh -G for '{alias}'"
            )));
            return;
        };
        let result = crate::exec::tcp_reach::check_tcp_reach(
            &hostname,
            port,
            crate::exec::tcp_reach::DEFAULT_BUDGET,
        );
        self.status_message = Some(match result {
            crate::exec::tcp_reach::ReachResult::Reachable { ms } => {
                StatusMessage::new(format!("✓ TCP reach: {hostname}:{port} ({ms} ms)"))
            }
            crate::exec::tcp_reach::ReachResult::Unreachable { error } => {
                StatusMessage::error(format!("✗ TCP unreachable: {hostname}:{port} — {error}"))
            }
        });
    }

    /// v0.9 G4: pressing `c` on a selected host runs `ssh -G <alias>` to
    /// learn the effective hostname/port/user/identityfile, builds an
    /// `ssh user@host -p port -i key` one-liner, and pushes it onto the
    /// system clipboard. Useful when sharing a connection string with
    /// someone who doesn't have the user's `~/.ssh/config`.
    ///
    /// Clipboard failures surface as sticky Error status messages
    /// (G3) so a Wayland / SSH-without-DISPLAY environment doesn't
    /// silently swallow the copy.
    pub(super) fn copy_ssh_command_for_selected(&mut self) {
        let Some(alias) = self.selected_host().map(|h| h.alias.clone()) else {
            return;
        };
        let cmd = match crate::exec::ssh_config::ssh_command_for_alias(&alias) {
            Ok(c) => c,
            Err(e) => {
                self.status_message = Some(StatusMessage::error(format!(
                    "could not resolve '{alias}' for copy: {e}"
                )));
                return;
            }
        };
        match crate::exec::clipboard::copy_to_clipboard(&cmd) {
            Ok(crate::exec::clipboard::ClipboardBackend::System) => {
                self.status_message = Some(StatusMessage::new(format!("copied: {cmd}")));
            }
            Ok(crate::exec::clipboard::ClipboardBackend::Osc52) => {
                self.status_message = Some(StatusMessage::new(format!("copied: {cmd} (osc52)")));
            }
            Err(e) => {
                self.status_message = Some(StatusMessage::error(format!(
                    "clipboard unavailable ({e}); copy '{cmd}' manually"
                )));
            }
        }
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
                    self.status_message = Some(StatusMessage::error(format!("ssh -G failed: {e}")));
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
