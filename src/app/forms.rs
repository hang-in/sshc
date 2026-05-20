//! Form-related methods on `impl super::App` — opening modal forms,
//! applying their submissions, and persisting changes back to `sshc.conf`.

use super::{App, AppAction, FormContext};
use crate::config::model::Host;
use crate::config::tags::normalize_tag;
use crate::error::{AppError, SetupError};
use crate::probe::ProbeState;
use crate::ui::modal::{FormPayload, ModalAction, ModalKind};
use crate::ui::status_bar::StatusMessage;

impl App {
    pub(super) fn open_add_form(&mut self) {
        if self.is_read_only() {
            self.status_message = Some(StatusMessage::new(
                "read-only — press 'i' to add Include line and enable writes",
            ));
            return;
        }
        let form = crate::ui::forms::HostForm::new();
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
                },
            ) => {
                let host =
                    self.build_host(alias, hostname, user, port, identity_file, tags_csv, extra);
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
                },
            ) => {
                let new_host =
                    self.build_host(alias, hostname, user, port, identity_file, tags_csv, extra);
                self.apply_modify(&target_alias, new_host)
            }
            (FormContext::EditTags(alias), FormPayload::Tags { tags_csv }) => {
                self.apply_tags(&alias, &tags_csv)
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
        self.hosts.push(host);
        self.probe_states.push(ProbeState::Unknown);
        self.persist_sshc_conf()?;
        self.apply_filter();
        self.validation_cache.clear();
        Ok(())
    }

    fn apply_modify(&mut self, alias: &str, new_host: Host) -> Result<(), AppError> {
        if let Some(pos) = self.hosts.iter().position(|h| h.alias == alias) {
            self.hosts[pos] = new_host;
            self.persist_sshc_conf()?;
            self.apply_filter();
            self.validation_cache.clear();
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
