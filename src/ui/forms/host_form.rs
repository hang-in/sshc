use crate::config::tags::normalize_tag;
use crate::ui::forms::list_edit::{ForwardingKind, ListEditModal, ListKind, ListOutcome};
use crate::ui::modal::{FormOutcome, FormPayload, FormState};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use std::path::PathBuf;

const FIELD_COUNT: usize = 10;
/// Index of the IdentityFile field in `fields` / `labels`.
const IDENTITY_INDEX: usize = 4;
/// v0.10 G1: forwarding field indices in `fields[]`. These rows are
/// summary cells; typing has no effect, Enter opens
/// `ForwardingListModal` against the matching kind.
const LOCAL_FORWARD_INDEX: usize = 6;
const REMOTE_FORWARD_INDEX: usize = 7;
const DYNAMIC_FORWARD_INDEX: usize = 8;
/// v0.9 G5: section headers rendered between input rows. Each entry is
/// `(field_index_before_which_header_appears, label)`. Headers are
/// dimmed, non-focusable, and don't change Tab routing.
const SECTION_HEADERS: &[(usize, &str)] = &[(6, "─── Forwarding ───"), (9, "─── Advanced ───")];

pub struct HostForm {
    fields: [String; FIELD_COUNT],
    active_index: usize,
    error: Option<String>,
    /// Private-key file candidates discovered under `~/.ssh/`. The
    /// IdentityFile field uses ↑/↓ to cycle through these so the user
    /// doesn't have to type the full path.
    identity_candidates: Vec<PathBuf>,
    /// v0.10 G1: typed Forwarding entries, kept out of `fields` so
    /// multiple entries per kind can be modeled. fields[6/7/8] hold
    /// the *display summary* synced from these vectors.
    local_forward_entries: Vec<String>,
    remote_forward_entries: Vec<String>,
    dynamic_forward_entries: Vec<String>,
    /// v0.12 G1 R3: IdentityFile is now a Vec too. Like the
    /// forwarding triple, fields[IDENTITY_INDEX] holds the display
    /// summary; the real entries live here.
    identity_file_entries: Vec<String>,
    /// Active child modal when the user is editing one of the
    /// list-shaped rows (forwarding OR IdentityFile). Rendering +
    /// key dispatch route through this when set.
    child_modal: Option<ListEditModal>,
    /// Which row the open modal is editing — used to route the
    /// result back into the right Vec on close.
    child_target: Option<ChildTarget>,
}

/// v0.12 G1 R3: which HostForm row a child `ListEditModal` is
/// currently editing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChildTarget {
    Forwarding(ForwardingKind),
    IdentityFile,
}

impl HostForm {
    /// Construct an empty form. `identity_candidates` is the list of
    /// private-key paths the IdentityFile field will cycle through with
    /// ↑/↓ — discovered by the caller (typically via
    /// `app::forms::discover_identity_files`), kept out of this widget
    /// to keep `ui/forms/*` free of filesystem access (R-G8).
    pub fn new(identity_candidates: Vec<PathBuf>) -> Self {
        Self {
            fields: Default::default(),
            active_index: 0,
            error: None,
            identity_candidates,
            local_forward_entries: Vec::new(),
            remote_forward_entries: Vec::new(),
            dynamic_forward_entries: Vec::new(),
            identity_file_entries: Vec::new(),
            child_modal: None,
            child_target: None,
        }
    }

    // v0.5 surface called for 7 args; v0.8 R0 hoisted identity_candidates
    // for R-G8 (8); v0.9 G5 added three Forwarding fields (11); v0.10 G1
    // promotes those to Vecs (still 11, just with different types). A
    // struct-of-args cleanup keeps drifting later — for now keep the
    // explicit signature so individual call sites stay legible.
    #[allow(clippy::too_many_arguments)]
    pub fn from_host(
        alias: &str,
        hostname: &str,
        user: &str,
        port: &str,
        identity_file: Vec<String>,
        tags_csv: &str,
        local_forward: Vec<String>,
        remote_forward: Vec<String>,
        dynamic_forward: Vec<String>,
        extra: &str,
        identity_candidates: Vec<PathBuf>,
    ) -> Self {
        let mut form = Self {
            fields: [
                alias.to_string(),
                hostname.to_string(),
                user.to_string(),
                port.to_string(),
                String::new(), // fields[4]: synced from identity_file_entries
                tags_csv.to_string(),
                String::new(),
                String::new(),
                String::new(),
                extra.to_string(),
            ],
            active_index: 0,
            error: None,
            identity_candidates,
            local_forward_entries: local_forward,
            remote_forward_entries: remote_forward,
            dynamic_forward_entries: dynamic_forward,
            identity_file_entries: identity_file,
            child_modal: None,
            child_target: None,
        };
        form.refresh_list_summaries();
        form
    }

    /// v0.10 G1 + v0.12 G1 R3: rebuild the summary text shown in
    /// `fields[IDENTITY_INDEX, 6, 7, 8]` from the underlying entry
    /// vectors. Called after `from_host` and after a list-edit modal
    /// closes with new entries.
    fn refresh_list_summaries(&mut self) {
        self.fields[IDENTITY_INDEX] = Self::summary_for(&self.identity_file_entries);
        self.fields[LOCAL_FORWARD_INDEX] = Self::summary_for(&self.local_forward_entries);
        self.fields[REMOTE_FORWARD_INDEX] = Self::summary_for(&self.remote_forward_entries);
        self.fields[DYNAMIC_FORWARD_INDEX] = Self::summary_for(&self.dynamic_forward_entries);
    }

    fn summary_for(entries: &[String]) -> String {
        match entries.len() {
            0 => String::new(),
            1 => entries[0].clone(),
            n => format!("{} +{} more", entries[0], n - 1),
        }
    }

    fn forwarding_kind_for(index: usize) -> Option<ForwardingKind> {
        match index {
            LOCAL_FORWARD_INDEX => Some(ForwardingKind::Local),
            REMOTE_FORWARD_INDEX => Some(ForwardingKind::Remote),
            DYNAMIC_FORWARD_INDEX => Some(ForwardingKind::Dynamic),
            _ => None,
        }
    }

    fn entries_for(&self, kind: ForwardingKind) -> &Vec<String> {
        match kind {
            ForwardingKind::Local => &self.local_forward_entries,
            ForwardingKind::Remote => &self.remote_forward_entries,
            ForwardingKind::Dynamic => &self.dynamic_forward_entries,
        }
    }

    fn set_entries_for(&mut self, kind: ForwardingKind, entries: Vec<String>) {
        match kind {
            ForwardingKind::Local => self.local_forward_entries = entries,
            ForwardingKind::Remote => self.remote_forward_entries = entries,
            ForwardingKind::Dynamic => self.dynamic_forward_entries = entries,
        }
    }

    fn open_forwarding_modal(&mut self, kind: ForwardingKind) {
        let entries = self.entries_for(kind).clone();
        self.child_modal = Some(ListEditModal::new(ListKind::Forwarding(kind), entries));
        self.child_target = Some(ChildTarget::Forwarding(kind));
    }

    /// v0.12 G1 R3: open the IdentityFile child modal. Seeds the
    /// modal with the current Vec + the candidate list so the
    /// v0.7.1 ↑/↓ picker survives inside edit mode.
    fn open_identity_modal(&mut self) {
        let entries = self.identity_file_entries.clone();
        let candidates = self.identity_candidates.clone();
        self.child_modal = Some(ListEditModal::new(
            ListKind::IdentityFile { candidates },
            entries,
        ));
        self.child_target = Some(ChildTarget::IdentityFile);
    }

    fn validate(&self) -> Result<FormPayload, String> {
        let alias = self.fields[0].trim();
        if alias.is_empty()
            || !alias
                .chars()
                .all(|c| c.is_alphanumeric() || c == '.' || c == '_' || c == '-')
        {
            return Err("Invalid Alias: alphanumeric, '.', '_', '-' only".to_string());
        }

        let hostname = self.fields[1].trim();
        if hostname.is_empty() {
            return Err("HostName is required".to_string());
        }

        let port_str = self.fields[3].trim();
        if !port_str.is_empty() {
            let parsed: u16 = port_str
                .parse()
                .map_err(|_| "Invalid Port number".to_string())?;
            if parsed == 0 {
                return Err("Port must be 1-65535".to_string());
            }
        }

        // v0.12 G1 R3: IdentityFile entries are now managed by
        // `ListEditModal` (kind = IdentityFile). The modal's per-entry
        // validator handles path-shape checks (cfg(windows) keeps the
        // backslash exception that v0.7.2 introduced). The form just
        // hands the Vec through.

        let tags_csv = self.fields[5].trim();
        let distinct: std::collections::HashSet<_> =
            tags_csv.split(',').filter_map(normalize_tag).collect();
        if distinct.len() > 16 {
            return Err("Maximum 16 distinct tags allowed".to_string());
        }

        // v0.10 G1: forwarding entries are now managed by the
        // ForwardingListModal — every push through the modal is
        // already validated. The form just hands the Vecs through.

        let extra = self.fields[9].trim();

        Ok(FormPayload::Host {
            alias: alias.to_string(),
            hostname: hostname.to_string(),
            user: self.fields[2].trim().to_string(),
            port: port_str.to_string(),
            identity_file: self.identity_file_entries.clone(),
            tags_csv: tags_csv.to_string(),
            local_forward: self.local_forward_entries.clone(),
            remote_forward: self.remote_forward_entries.clone(),
            dynamic_forward: self.dynamic_forward_entries.clone(),
            extra: extra.to_string(),
        })
    }
}

// v0.9 G5 had `looks_like_local_remote_forward` /
// `looks_like_dynamic_forward` helpers used by HostForm::validate.
// v0.10 G1 moved per-entry validation into `ForwardingListModal` (one
// path per kind, applied at the moment the user presses Enter inside
// the list modal). The helpers themselves now live next to the modal
// in `ui::forms::forwarding_list`.

impl FormState for HostForm {
    fn render(&self, area: Rect, f: &mut Frame) {
        // v0.10 G1: when a forwarding list modal is open, it owns the
        // whole modal area. The parent HostForm waits underneath
        // until the child closes.
        if let Some(ref child) = self.child_modal {
            child.render(area, f);
            return;
        }
        let block = Block::default().title(" Host ").borders(Borders::ALL);
        let inner = block.inner(area);
        f.render_widget(block, area);

        let labels = [
            "Alias",
            "HostName",
            "User",
            "Port",
            "IdentityFile",
            "Tags",
            "LocalForward",
            "RemoteForward",
            "DynamicForward",
            "Options",
        ];
        // Right-padded so the column of `[` brackets lines up across rows.
        // "DynamicForward" is now the longest label at 14 chars; +2 for ": ".
        const LABEL_WIDTH: u16 = 16;

        // Each row is either an input field or a dim section header.
        // Build the row list so the layout can size itself off the total.
        enum Row {
            Field(usize),
            Header(&'static str),
        }
        let mut rows: Vec<Row> = Vec::with_capacity(FIELD_COUNT + SECTION_HEADERS.len());
        for i in 0..FIELD_COUNT {
            if let Some((_, header)) = SECTION_HEADERS.iter().find(|(at, _)| *at == i) {
                rows.push(Row::Header(header));
            }
            rows.push(Row::Field(i));
        }
        let total_rows = rows.len();

        let outer = Layout::vertical([
            Constraint::Length(total_rows as u16),
            Constraint::Length(1),
            Constraint::Min(1),
        ])
        .split(inner);
        let row_chunks = Layout::vertical(vec![Constraint::Length(1); total_rows]).split(outer[0]);

        for (row_idx, row) in rows.iter().enumerate() {
            match row {
                Row::Header(text) => {
                    let header_line = Line::from(Span::styled(
                        format!(" {text}"),
                        Style::default()
                            .fg(Color::DarkGray)
                            .add_modifier(Modifier::DIM),
                    ));
                    f.render_widget(Paragraph::new(header_line), row_chunks[row_idx]);
                }
                Row::Field(i) => {
                    let is_active = self.active_index == *i;
                    let row_cells =
                        Layout::horizontal([Constraint::Length(LABEL_WIDTH), Constraint::Min(3)])
                            .split(row_chunks[row_idx]);

                    let label_style = if is_active {
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().add_modifier(Modifier::BOLD)
                    };
                    let label_text = format!(
                        "{:<width$}",
                        format!("{}:", labels[*i]),
                        width = LABEL_WIDTH as usize
                    );
                    f.render_widget(
                        Paragraph::new(Line::from(Span::styled(label_text, label_style))),
                        row_cells[0],
                    );

                    let cursor = if is_active { "█" } else { "" };
                    let value_text = format!("[{}{}]", self.fields[*i], cursor);
                    let value_style = if is_active {
                        Style::default().fg(Color::Yellow)
                    } else {
                        Style::default()
                    };
                    f.render_widget(
                        Paragraph::new(Line::from(Span::styled(value_text, value_style))),
                        row_cells[1],
                    );
                }
            }
        }

        // Hint about the Options field syntax — sits between the fields
        // and the footer so it doesn't crowd the label column.
        let hint = Line::from(Span::styled(
            " (Options: semicolon-separated SSH directives, e.g. \"ProxyJump h; ForwardAgent yes\")",
            Style::default().fg(Color::DarkGray),
        ));
        f.render_widget(Paragraph::new(hint), outer[1]);

        let footer_line = if let Some(ref err) = self.error {
            Line::from(Span::styled(err.clone(), Style::default().fg(Color::Red)))
        } else if self.active_index == IDENTITY_INDEX {
            // v0.12 G1 R3: the form-level ↑/↓ key picker is gone —
            // it moved inside the list modal. Hint surfaces Enter
            // as the gateway and mentions the candidate count.
            Line::from(Span::styled(
                format!(
                    " Enter to open the IdentityFile list ({} key(s) discoverable) • Tab move • Esc cancel",
                    self.identity_candidates.len()
                ),
                Style::default().fg(Color::Gray),
            ))
        } else {
            Line::from(Span::styled(
                " Tab/Shift-Tab move • Enter submit • Esc cancel • Ctrl-U clear field",
                Style::default().fg(Color::Gray),
            ))
        };
        f.render_widget(Paragraph::new(footer_line), outer[2]);
    }

    fn handle_key(&mut self, key: KeyEvent) -> FormOutcome {
        // v0.10 G1 + v0.12 G1 R3: route every keystroke into the
        // active child modal until it reports Done/Cancel. The modal
        // handles forwarding triples AND IdentityFile.
        if self.child_modal.is_some() {
            let outcome = self
                .child_modal
                .as_mut()
                .map(|m| m.handle_key(key))
                .unwrap_or(ListOutcome::Stay);
            match outcome {
                ListOutcome::Stay => {}
                ListOutcome::Done => {
                    let modal = self.child_modal.take().unwrap();
                    let target = self.child_target.take().unwrap();
                    let new_entries = modal.entries().to_vec();
                    match target {
                        ChildTarget::Forwarding(kind) => self.set_entries_for(kind, new_entries),
                        ChildTarget::IdentityFile => self.identity_file_entries = new_entries,
                    }
                    self.refresh_list_summaries();
                }
                ListOutcome::Cancel => {
                    self.child_modal = None;
                    self.child_target = None;
                }
            }
            return FormOutcome::Stay;
        }
        // List-shaped rows are summary-only: char / backspace silently
        // ignored, Enter opens the child modal. Covers the three
        // forwarding kinds + IdentityFile (v0.12 G1 R3).
        let on_list_row = matches!(
            self.active_index,
            IDENTITY_INDEX | LOCAL_FORWARD_INDEX | REMOTE_FORWARD_INDEX | DYNAMIC_FORWARD_INDEX
        );
        match key.code {
            KeyCode::Tab => {
                self.active_index = (self.active_index + 1) % FIELD_COUNT;
                FormOutcome::Stay
            }
            KeyCode::BackTab => {
                self.active_index = (self.active_index + FIELD_COUNT - 1) % FIELD_COUNT;
                FormOutcome::Stay
            }
            KeyCode::Backspace if !on_list_row => {
                self.fields[self.active_index].pop();
                FormOutcome::Stay
            }
            KeyCode::Esc => FormOutcome::Cancel,
            KeyCode::Enter => {
                if on_list_row {
                    if self.active_index == IDENTITY_INDEX {
                        self.open_identity_modal();
                    } else if let Some(kind) = Self::forwarding_kind_for(self.active_index) {
                        self.open_forwarding_modal(kind);
                    }
                    return FormOutcome::Stay;
                }
                if self.active_index + 1 < FIELD_COUNT {
                    self.active_index += 1;
                    FormOutcome::Stay
                } else {
                    match self.validate() {
                        Ok(payload) => FormOutcome::Submit(payload),
                        Err(e) => {
                            self.error = Some(e);
                            FormOutcome::Stay
                        }
                    }
                }
            }
            KeyCode::Char(c) => {
                if on_list_row {
                    return FormOutcome::Stay;
                }
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    if c == 'u' || c == 'U' {
                        self.fields[self.active_index].clear();
                    }
                } else {
                    self.fields[self.active_index].push(c);
                }
                FormOutcome::Stay
            }
            _ => FormOutcome::Stay,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::modal::{FormOutcome, FormPayload};
    use crossterm::event::{KeyEvent, KeyModifiers};

    fn ke(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::empty())
    }

    fn ke_ctrl(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::CONTROL)
    }

    #[test]
    fn test_new_is_empty() {
        let form = HostForm::new(Vec::new());
        assert_eq!(form.active_index, 0);
        assert!(form.error.is_none());
        assert!(form.fields.iter().all(|s| s.is_empty()));
    }

    #[test]
    fn test_tab_wraparound() {
        let mut form = HostForm::new(Vec::new());
        for expected in &[1usize, 2, 3, 4, 5, 6, 7, 8, 9, 0] {
            form.handle_key(ke(KeyCode::Tab));
            assert_eq!(form.active_index, *expected);
        }
    }

    #[test]
    fn test_backtab_wraparound() {
        let mut form = HostForm::new(Vec::new());
        for expected in &[9usize, 8, 7, 6, 5, 4, 3, 2, 1, 0] {
            form.handle_key(ke(KeyCode::BackTab));
            assert_eq!(form.active_index, *expected);
        }
    }

    #[test]
    fn test_char_backspace_ctrl_u() {
        let mut form = HostForm::new(Vec::new());
        form.handle_key(ke(KeyCode::Char('a')));
        assert_eq!(form.fields[0], "a");
        form.handle_key(ke(KeyCode::Backspace));
        assert_eq!(form.fields[0], "");
        form.handle_key(ke(KeyCode::Char('b')));
        form.handle_key(ke(KeyCode::Char('c')));
        form.handle_key(ke_ctrl(KeyCode::Char('u')));
        assert_eq!(form.fields[0], "");
    }

    #[test]
    fn test_esc_cancels() {
        let mut form = HostForm::new(Vec::new());
        assert!(matches!(
            form.handle_key(ke(KeyCode::Esc)),
            FormOutcome::Cancel
        ));
    }

    #[test]
    fn test_enter_advances_non_last_field() {
        let mut form = HostForm::new(Vec::new());
        form.handle_key(ke(KeyCode::Enter));
        assert_eq!(form.active_index, 1);
    }

    #[test]
    fn test_enter_on_last_field_submits_when_valid() {
        let mut form = HostForm::new(Vec::new());
        form.fields[0] = "dev1".to_string();
        form.fields[1] = "10.0.0.1".to_string();
        form.active_index = 9;
        match form.handle_key(ke(KeyCode::Enter)) {
            FormOutcome::Submit(FormPayload::Host {
                alias, hostname, ..
            }) => {
                assert_eq!(alias, "dev1");
                assert_eq!(hostname, "10.0.0.1");
            }
            _ => panic!("expected Submit(Host)"),
        }
    }

    #[test]
    fn test_validation_missing_alias() {
        let mut form = HostForm::new(Vec::new());
        form.fields[1] = "host".to_string();
        form.active_index = 9;
        assert!(matches!(
            form.handle_key(ke(KeyCode::Enter)),
            FormOutcome::Stay
        ));
        assert!(form.error.is_some());
    }

    #[test]
    fn test_validation_invalid_port() {
        let mut form = HostForm::new(Vec::new());
        form.fields[0] = "dev".to_string();
        form.fields[1] = "h".to_string();
        form.fields[3] = "70000".to_string();
        form.active_index = 9;
        assert!(matches!(
            form.handle_key(ke(KeyCode::Enter)),
            FormOutcome::Stay
        ));
        assert!(form.error.is_some());
    }

    #[test]
    fn test_validation_shell_metachar() {
        // v0.7.2 → v0.12 R3 move: IdentityFile path validation
        // lives in ListEditModal now. Drive it through the modal
        // surface — open the IdentityFile child, type a value with
        // a shell metacharacter, press Enter, expect the modal to
        // stay open with an error and entries unchanged.
        let mut form = HostForm::new(Vec::new());
        form.fields[0] = "dev".to_string();
        form.fields[1] = "h".to_string();
        form.active_index = IDENTITY_INDEX;
        form.handle_key(ke(KeyCode::Enter)); // opens the IdentityFile modal
        assert!(form.child_modal.is_some());
        form.handle_key(ke(KeyCode::Enter)); // enters edit mode on the "+ add" row
        for c in "/etc/key;rm".chars() {
            form.handle_key(ke(KeyCode::Char(c)));
        }
        form.handle_key(ke(KeyCode::Enter)); // attempts to commit the bad value
        let modal = form.child_modal.as_ref().expect("modal still open");
        // Bad value did not land:
        assert!(modal.entries().is_empty());
    }

    #[test]
    fn test_from_host_populates_fields() {
        let form = HostForm::from_host(
            "a",
            "h",
            "u",
            "22",
            vec!["/k".to_string()],
            "x,y",
            vec!["8080 localhost:80".to_string()],
            vec!["9090 127.0.0.1:9090".to_string()],
            vec!["1080".to_string()],
            "ProxyJump bastion",
            Vec::new(),
        );
        assert_eq!(form.fields[0], "a");
        assert_eq!(form.fields[1], "h");
        assert_eq!(form.fields[2], "u");
        assert_eq!(form.fields[3], "22");
        assert_eq!(form.fields[4], "/k");
        assert_eq!(form.fields[5], "x,y");
        assert_eq!(form.fields[6], "8080 localhost:80");
        assert_eq!(form.fields[7], "9090 127.0.0.1:9090");
        assert_eq!(form.fields[8], "1080");
        assert_eq!(form.fields[9], "ProxyJump bastion");
    }

    // v0.9 G5 validator unit tests moved to
    // `ui::forms::forwarding_list::tests` along with the helpers they
    // exercised. See that module for canonical-shape coverage.
}
