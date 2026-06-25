//! Key-input dispatcher methods on `impl super::App`.
//!
//! Routes `handle_key` between list-mode and modal-mode handlers, and owns
//! the Enter-key behaviour (`activate_selected`) plus the modal action
//! dispatch table (`dispatch_modal_action`).

use crossterm::event::{KeyCode, KeyEvent};

use super::{App, AppAction, AppMode};
use crate::ui::modal::{FormOutcome, ModalAction, ModalKind};
use crate::ui::status_bar::StatusMessage;

impl App {
    pub fn handle_key(&mut self, key: KeyEvent) {
        // v0.9 G3: Error-kind status messages are sticky across
        // redraws; the user's next keystroke is what dismisses them.
        // Doing this *before* dispatch means a fresh status set by
        // the current keystroke (Info or Error) survives — only
        // the *previous* sticky Error is cleared.
        self.clear_sticky_error_status();
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
                KeyCode::Char('a') => self.open_add_form(),
                KeyCode::Char('d') => self.open_delete_confirm(),
                KeyCode::Char('f') => self.toggle_selected_favorite(),
                KeyCode::Char('t') => self.open_tag_form(),
                KeyCode::Char('v') => self.validate_selected(),
                KeyCode::Char('c') => self.copy_ssh_command_for_selected(),
                KeyCode::Char('g') => self.reach_check_for_selected(),
                KeyCode::Char('M') => self.promote_selected(),
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

    /// Toggle pinned status on the selected host. Queues a state save
    /// and re-runs the filter so the pinned glyph + sort order update
    /// immediately. Manage-mode only; the inline picker handles `f`
    /// itself with a status message instead.
    fn toggle_selected_favorite(&mut self) {
        let Some(alias) = self.selected_host().map(|h| h.alias.clone()) else {
            return;
        };
        let now_pinned = self.toggle_favorite(&alias);
        self.status_message = Some(StatusMessage::new(if now_pinned {
            format!("★ pinned: {alias}")
        } else {
            format!("pin removed: {alias}")
        }));
        self.pending_action = Some(AppAction::SaveState);
        self.apply_filter();
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

    /// v0.8 G2: pressing `M` on an external host requests promotion into
    /// `sshc.conf` — the runtime opens an add/modify form pre-filled
    /// with that host's fields. Pressing `M` on a host that's already
    /// managed by sshc surfaces a status hint and changes nothing else.
    /// Empty lists and the read-only state are silent no-ops.
    ///
    /// This method only routes the intent; the form prefill + write
    /// path lands in R4.
    fn promote_selected(&mut self) {
        if self.filtered.is_empty() {
            return;
        }
        let Some(host) = self.selected_host() else {
            return;
        };
        if host.source_file == self.sshc_conf_path_or_blank() {
            let alias = host.alias.clone();
            self.status_message = Some(StatusMessage::new(format!(
                "'{alias}' already managed by sshc.conf"
            )));
            return;
        }
        if self.is_read_only() {
            self.status_message = Some(StatusMessage::new(
                "read-only — press 'i' to add Include line and enable promote",
            ));
            return;
        }
        let alias = host.alias.clone();
        self.pending_action = Some(AppAction::OpenPromoteForm(alias));
    }
}
