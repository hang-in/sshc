//! Form-related methods on `impl super::App` — opening modal forms,
//! applying their submissions, and persisting changes back to `sshc.conf`.

use super::{App, AppAction, FormContext};
use crate::config::model::Host;
use crate::config::tags::normalize_tag;
use crate::error::{AppError, SetupError};
use crate::probe::ProbeState;
use crate::ui::modal::{FormPayload, ModalAction, ModalKind};
use crate::ui::status_bar::StatusMessage;
use std::path::PathBuf;

/// Scan `~/.ssh/` for plausible private-key files. Excludes:
///   - public-key counterparts (`*.pub`)
///   - well-known non-key files (`known_hosts*`, `authorized_keys`,
///     `config*`, `environment`)
///   - directories and hidden entries
///
/// Returns sorted by path. Empty Vec on any I/O failure. Lives in
/// `app/forms.rs` rather than `ui/forms/host_form.rs` so that the
/// `ui/forms/*` layer remains free of filesystem access (R-G8).
fn discover_identity_files() -> Vec<PathBuf> {
    let Some(home) = dirs::home_dir() else {
        return Vec::new();
    };
    let ssh_dir = home.join(".ssh");
    let entries = match std::fs::read_dir(&ssh_dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };
    const EXCLUDED_PREFIXES: &[&str] = &["known_hosts", "authorized_keys", "config", "environment"];
    let mut candidates: Vec<PathBuf> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if !path.is_file() {
                return None;
            }
            let name = path.file_name()?.to_str()?;
            if name.starts_with('.') || name.ends_with(".pub") {
                return None;
            }
            if EXCLUDED_PREFIXES.iter().any(|p| name.starts_with(p)) {
                return None;
            }
            Some(path)
        })
        .collect();
    candidates.sort();
    candidates
}

impl App {
    pub(super) fn open_add_form(&mut self) {
        if self.is_read_only() {
            self.status_message = Some(StatusMessage::new(
                "read-only — press 'i' to add Include line and enable writes",
            ));
            return;
        }
        let form = crate::ui::forms::HostForm::new(discover_identity_files());
        self.active_form_context = Some(FormContext::AddHost);
        self.mode = super::AppMode::Modal(ModalKind::Form(Box::new(form)));
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
        let local_forward = host.local_forward.clone();
        let remote_forward = host.remote_forward.clone();
        let dynamic_forward = host.dynamic_forward.clone();
        let form = crate::ui::forms::HostForm::from_host(
            &host.alias,
            host.hostname.as_deref().unwrap_or(""),
            host.user.as_deref().unwrap_or(""),
            &port_str,
            &identity,
            &tags_csv,
            local_forward,
            remote_forward,
            dynamic_forward,
            &extra_joined,
            discover_identity_files(),
        );
        self.active_form_context = Some(FormContext::EditHost(host.alias.clone()));
        self.mode = super::AppMode::Modal(ModalKind::Form(Box::new(form)));
    }

    /// v0.8 G2: open the add/modify form pre-filled with the fields of
    /// an external host so that its saved form lands in `sshc.conf` as a
    /// brand-new entry. The original `~/.ssh/config` line is **never**
    /// touched — anti-feature 1 stands. Three early-exit branches:
    ///
    /// - the alias is already present in `sshc.conf` → status hint and
    ///   the form does not open (would be an immediate `apply_add`
    ///   collision anyway).
    /// - the alias contains an SSH wildcard (`*`, `?`) → status hint
    ///   and the form does not open. Wildcard hosts can't be promoted
    ///   into sshc.conf because sshc only manages explicit aliases
    ///   (anti-feature 5: no full `config(5)` parser).
    /// - read-only or alias not found → silent no-op (matches the
    ///   other `open_*_form` methods).
    ///
    /// On success the form opens with `FormContext::PromoteHost(alias)`
    /// and submission flows through the same `apply_add` write path as
    /// any other new sshc.conf entry.
    pub fn open_promote_form(&mut self, alias: &str) {
        if self.is_read_only() {
            self.status_message = Some(StatusMessage::new(
                "read-only — press 'i' to add Include line and enable promote",
            ));
            return;
        }
        // Wildcards bypass the lookup-by-alias check below (a literal
        // `*` rarely matches a parsed host), so test the string up
        // front.
        if alias.contains('*') || alias.contains('?') {
            self.status_message = Some(StatusMessage::new(format!(
                "wildcard alias '{alias}' cannot be promoted — sshc only manages explicit aliases"
            )));
            return;
        }
        // Look the host up rather than trusting the selection — the
        // list may have shifted between the `M` keystroke and runtime
        // dispatch (e.g. user typed into the filter mid-flight).
        let sshc_conf = self.sshc_conf_path_or_blank();
        let Some(host) = self.hosts.iter().find(|h| h.alias == alias).cloned() else {
            return;
        };
        if host.source_file == sshc_conf {
            self.status_message = Some(StatusMessage::new(format!(
                "'{alias}' already managed by sshc.conf"
            )));
            return;
        }
        if self
            .hosts
            .iter()
            .any(|h| h.alias == alias && h.source_file == sshc_conf)
        {
            // Defensive: the find() above returned an external row, but
            // there's *also* an sshc.conf-side entry with the same
            // alias. That's a duplicate-alias situation OpenSSH itself
            // wouldn't disambiguate cleanly — bail out so we don't
            // silently shadow it.
            self.status_message = Some(StatusMessage::new(format!(
                "'{alias}' already exists in sshc.conf — promote aborted"
            )));
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
        let local_forward = host.local_forward.clone();
        let remote_forward = host.remote_forward.clone();
        let dynamic_forward = host.dynamic_forward.clone();
        let form = crate::ui::forms::HostForm::from_host(
            &host.alias,
            host.hostname.as_deref().unwrap_or(""),
            host.user.as_deref().unwrap_or(""),
            &port_str,
            &identity,
            &tags_csv,
            local_forward,
            remote_forward,
            dynamic_forward,
            &extra_joined,
            discover_identity_files(),
        );
        self.active_form_context = Some(FormContext::PromoteHost(host.alias.clone()));
        self.mode = super::AppMode::Modal(ModalKind::Form(Box::new(form)));
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
        self.mode = super::AppMode::Modal(ModalKind::Form(Box::new(form)));
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
        self.mode = super::AppMode::Modal(ModalKind::Confirmation {
            prompt: format!("Delete host '{}'?", host.alias),
            on_yes: ModalAction::Custom("delete_selected".to_string()),
            on_no: ModalAction::None,
        });
    }

    pub(super) fn open_help_modal(&mut self) {
        let msg = "j/k nav  / filter  Enter open  s ssh  f pin  v validate\n\
                   a add  d delete  t tags  e edit  i include  ? help  q quit"
            .to_string();
        self.mode = super::AppMode::Modal(ModalKind::Info {
            message: msg,
            dismiss: ModalAction::None,
        });
    }

    pub(super) fn apply_form(&mut self, ctx: FormContext, payload: FormPayload) {
        let result = match (ctx, payload) {
            (
                FormContext::AddHost,
                FormPayload::Host {
                    alias,
                    hostname,
                    user,
                    port,
                    identity_file,
                    tags_csv,
                    extra,
                    local_forward,
                    remote_forward,
                    dynamic_forward,
                },
            ) => {
                let host = self.build_host(
                    alias,
                    hostname,
                    user,
                    port,
                    identity_file,
                    tags_csv,
                    extra,
                    local_forward,
                    remote_forward,
                    dynamic_forward,
                );
                self.apply_add(host)
            }
            (
                FormContext::EditHost(target_alias),
                FormPayload::Host {
                    alias,
                    hostname,
                    user,
                    port,
                    identity_file,
                    tags_csv,
                    extra,
                    local_forward,
                    remote_forward,
                    dynamic_forward,
                },
            ) => {
                let new_host = self.build_host(
                    alias,
                    hostname,
                    user,
                    port,
                    identity_file,
                    tags_csv,
                    extra,
                    local_forward,
                    remote_forward,
                    dynamic_forward,
                );
                self.apply_modify(&target_alias, new_host)
            }
            (FormContext::EditTags(alias), FormPayload::Tags { tags_csv }) => {
                self.apply_tags(&alias, &tags_csv)
            }
            (
                FormContext::PromoteHost(original_alias),
                FormPayload::Host {
                    alias,
                    hostname,
                    user,
                    port,
                    identity_file,
                    tags_csv,
                    extra,
                    local_forward,
                    remote_forward,
                    dynamic_forward,
                },
            ) => {
                let host = self.build_host(
                    alias,
                    hostname,
                    user,
                    port,
                    identity_file,
                    tags_csv,
                    extra,
                    local_forward,
                    remote_forward,
                    dynamic_forward,
                );
                self.apply_promote(&original_alias, host)
            }
            _ => Ok(()),
        };
        match result {
            Ok(()) => {
                self.pending_action = Some(AppAction::SaveState);
            }
            Err(e) => {
                self.status_message = Some(StatusMessage::error(format!("form apply failed: {e}")));
            }
        }
    }

    /// Build a `Host` from already-destructured form fields. Source file is
    /// the cached `sshc.conf` path (or the empty sentinel — see
    /// `sshc_conf_path_or_blank`).
    #[allow(clippy::too_many_arguments)]
    fn build_host(
        &self,
        alias: String,
        hostname: String,
        user: String,
        port: String,
        identity_file: String,
        tags_csv: String,
        extra: String,
        local_forward: Vec<String>,
        remote_forward: Vec<String>,
        dynamic_forward: Vec<String>,
    ) -> Host {
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
        let user_field = if user.is_empty() { None } else { Some(user) };
        let extra_lines: Vec<String> = extra
            .split(';')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();
        Host {
            alias,
            hostname: Some(hostname),
            user: user_field,
            port: port_parsed,
            identity_file: identity,
            line_start: 1,
            source_file: self.sshc_conf_path_or_blank(),
            tags: normalized_tags(&tags_csv),
            extra: extra_lines,
            local_forward,
            remote_forward,
            dynamic_forward,
        }
    }

    /// Apply an add-host form submission: append to in-memory hosts and
    /// persist via storage::with_locked_write.
    fn apply_add(&mut self, host: Host) -> Result<(), AppError> {
        if self.hosts.iter().any(|h| h.alias == host.alias) {
            self.status_message = Some(StatusMessage::new(format!(
                "alias '{}' already exists",
                host.alias
            )));
            return Ok(());
        }
        let alias_for_msg = host.alias.clone();
        let identity_missing = host.identity_file.is_none();
        self.hosts.push(host);
        self.probe_states.push(ProbeState::Unknown);
        self.persist_sshc_conf()?;
        self.apply_filter();
        self.validation_cache.clear();
        if identity_missing {
            self.status_message = Some(StatusMessage::new(format!(
                "'{alias_for_msg}' saved without IdentityFile — ssh will use agent or password prompt"
            )));
        }
        Ok(())
    }

    /// Promotion save: the user submitted a form opened via
    /// `open_promote_form`. The write path is exactly `apply_add`
    /// (append + persist sshc.conf), but the status message reminds
    /// them that the original `~/.ssh/config` entry is still there and
    /// will produce duplicate `ssh -G` lines until they delete it
    /// themselves. `original_alias` is the alias under which the host
    /// lived in the external source; the form can rename it during
    /// promote (no constraint).
    fn apply_promote(&mut self, original_alias: &str, host: Host) -> Result<(), AppError> {
        if self
            .hosts
            .iter()
            .any(|h| h.alias == host.alias && h.source_file == self.sshc_conf_path_or_blank())
        {
            self.status_message = Some(StatusMessage::new(format!(
                "'{}' already exists in sshc.conf — promote aborted",
                host.alias
            )));
            return Ok(());
        }
        let new_alias = host.alias.clone();
        self.hosts.push(host);
        self.probe_states.push(ProbeState::Unknown);
        self.persist_sshc_conf()?;
        self.apply_filter();
        self.validation_cache.clear();
        let rename_note = if new_alias == original_alias {
            String::new()
        } else {
            format!(" (renamed from '{original_alias}')")
        };
        self.status_message = Some(StatusMessage::new(format!(
            "'{new_alias}' promoted to sshc.conf{rename_note} — original ~/.ssh/config entry left intact, delete it manually if duplicate ssh -G output bothers you"
        )));
        Ok(())
    }

    fn apply_modify(&mut self, alias: &str, new_host: Host) -> Result<(), AppError> {
        if let Some(pos) = self.hosts.iter().position(|h| h.alias == alias) {
            let identity_missing = new_host.identity_file.is_none();
            self.hosts[pos] = new_host;
            self.persist_sshc_conf()?;
            self.apply_filter();
            self.validation_cache.clear();
            if identity_missing {
                self.status_message = Some(StatusMessage::new(format!(
                    "'{alias}' saved without IdentityFile — ssh will use agent or password prompt"
                )));
            }
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
                self.status_message = Some(StatusMessage::error(format!("delete failed: {e}")));
            } else {
                self.apply_filter();
                self.validation_cache.clear();
                self.pending_action = Some(AppAction::SaveState);
            }
        }
    }

    fn apply_tags(&mut self, alias: &str, tags_csv: &str) -> Result<(), AppError> {
        let normalized = normalized_tags(tags_csv);
        if let Some(host) = self.hosts.iter_mut().find(|h| h.alias == alias) {
            host.tags = normalized;
            self.persist_sshc_conf()?;
            self.apply_filter();
            self.validation_cache.clear();
        }
        Ok(())
    }

    /// Re-render the in-memory hosts that live in sshc.conf and atomically
    /// rewrite the file. Uses the cached `self.sshc_conf_path` for both
    /// the filter predicate and the write target so the two comparisons
    /// sit on the same `PathBuf` instance — v0.8.1 unified this after
    /// v0.7.x recomputed the path inside the function. The path-cache
    /// unification was correct but didn't fix the Windows `a`-save
    /// failure: the real cause was `with_locked_write` opening a second
    /// `File::open(path)` after `LockFileEx`, which trips
    /// `ERROR_LOCK_VIOLATION` on Windows. Fixed in v0.8.2 by reading
    /// from the already-locked handle. See `src/storage/writer.rs`.
    fn persist_sshc_conf(&self) -> Result<(), AppError> {
        let path = self
            .sshc_conf_path
            .clone()
            .ok_or(AppError::Setup(SetupError::HomeDirMissing))?;
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

/// Split a comma-separated tag string, normalize each token via
/// `crate::config::tags::normalize_tag`, and dedupe (order-preserving).
fn normalized_tags(csv: &str) -> Vec<String> {
    csv.split(',')
        .filter_map(normalize_tag)
        .fold(Vec::new(), |mut acc, t| {
            if !acc.contains(&t) {
                acc.push(t);
            }
            acc
        })
}
