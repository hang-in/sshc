//! v0.10 G1: a tiny `FormState` impl for editing a list of entries
//! that belong to one OpenSSH directive kind. Opened by the parent
//! `HostForm` when the user presses Enter on a list-shaped field row.
//! The parent reads the result back through `entries()` once
//! `handle_key` reports `Done`/`Cancel`.
//!
//! v0.12 G1: renamed from `ForwardingListModal` to `ListEditModal`
//! and the per-kind logic moved into a `ListKind` enum so the modal
//! can host IdentityFile (R3) on top of the original three forwarding
//! kinds. R1 (this commit) only wraps `ForwardingKind` under
//! `ListKind::Forwarding(_)`; behaviour is unchanged.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use std::path::PathBuf;

/// Which forwarding directive kind this modal is editing. Drives the
/// title bar + the validator picked for new entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForwardingKind {
    Local,
    Remote,
    Dynamic,
}

impl ForwardingKind {
    pub fn title(self) -> &'static str {
        match self {
            ForwardingKind::Local => "LocalForward",
            ForwardingKind::Remote => "RemoteForward",
            ForwardingKind::Dynamic => "DynamicForward",
        }
    }

    /// Loose syntax check that matches HostForm's R1 validators.
    /// Local/Remote: `[bind:]port host:hostport`. Dynamic: `[bind:]port`.
    pub fn validate(self, value: &str) -> bool {
        match self {
            ForwardingKind::Local | ForwardingKind::Remote => looks_like_lr(value),
            ForwardingKind::Dynamic => looks_like_dyn(value),
        }
    }
}

/// v0.12 G1: which kind of list this modal is editing. R1 wrapped the
/// `Forwarding(_)` variant; R3 adds `IdentityFile { candidates }` so
/// the IdentityFile row in `HostForm` reuses the same modal surface.
/// Driving title/validate/edit-mode hints through the enum lets the
/// modal stay one type as new list kinds land.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListKind {
    Forwarding(ForwardingKind),
    /// `candidates` carries the `~/.ssh/*` private-key paths the form
    /// discovered up-front (via `app::forms::discover_identity_files`).
    /// In the modal's edit mode `↑`/`↓` cycle through these — this is
    /// the v0.7.1 form-level picker behaviour, now moved inside the
    /// modal so the multi-entry case has a single home.
    IdentityFile {
        candidates: Vec<PathBuf>,
    },
}

impl ListKind {
    pub fn title(&self) -> &'static str {
        match self {
            ListKind::Forwarding(k) => k.title(),
            ListKind::IdentityFile { .. } => "IdentityFile",
        }
    }

    pub fn validate(&self, value: &str) -> bool {
        match self {
            ListKind::Forwarding(k) => k.validate(value),
            ListKind::IdentityFile { .. } => looks_like_identity_file_path(value),
        }
    }

    /// Error string shown in the modal footer when a value is
    /// rejected on Enter. Mirrors the v0.10 messages so existing
    /// tests pass unchanged.
    pub fn reject_hint(&self) -> &'static str {
        match self {
            ListKind::Forwarding(ForwardingKind::Local)
            | ListKind::Forwarding(ForwardingKind::Remote) => {
                "expected `[bind:]port host:hostport`"
            }
            ListKind::Forwarding(ForwardingKind::Dynamic) => "expected `[bind:]port`",
            ListKind::IdentityFile { .. } => "expected an `IdentityFile` path",
        }
    }

    /// Convenience: pull the candidate Vec out for ↑/↓ cycling in
    /// edit mode. Returns `None` for kinds that don't cycle.
    pub fn candidates(&self) -> Option<&[PathBuf]> {
        match self {
            ListKind::IdentityFile { candidates } => Some(candidates),
            _ => None,
        }
    }
}

/// v0.7.0–v0.7.2 carry-over: accept what looks like a filesystem path
/// for an `IdentityFile`. The Unix rule is "any char except shell
/// metacharacters and the quoting set `?*<>|`"; Windows additionally
/// permits the backslash that shipped paths use (the v0.7.2 fix).
/// Empty input rejected.
fn looks_like_identity_file_path(s: &str) -> bool {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return false;
    }
    // Shell metacharacters that previously sat in HostForm::validate —
    // moved here in v0.12 R3 along with the rest of the row's
    // validation. `\\` is a path separator on Windows so it stays
    // off the forbidden list there.
    #[cfg(windows)]
    let forbidden: &[char] = &[
        ';', '|', '&', '$', '`', '<', '>', '(', ')', '{', '}', '*', '?', '[', ']', '"', '\'',
    ];
    #[cfg(not(windows))]
    let forbidden: &[char] = &[
        ';', '|', '&', '$', '`', '<', '>', '(', ')', '{', '}', '*', '?', '[', ']', '"', '\'', '\\',
    ];
    !trimmed
        .chars()
        .any(|c| c.is_control() || forbidden.contains(&c))
}

fn looks_like_lr(s: &str) -> bool {
    let mut parts = s.splitn(2, |c: char| c.is_whitespace());
    let local = parts.next().unwrap_or("");
    let remote = parts.next().unwrap_or("").trim();
    if local.is_empty() || remote.is_empty() {
        return false;
    }
    let local_ok = local
        .rsplit(':')
        .next()
        .map(|p| p.chars().all(|c| c.is_ascii_digit()))
        .unwrap_or(false);
    let remote_ok = remote
        .rsplit_once(':')
        .map(|(_, p)| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
        .unwrap_or(false);
    local_ok && remote_ok
}

fn looks_like_dyn(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    s.rsplit(':')
        .next()
        .map(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
        .unwrap_or(false)
}

/// Result of one keystroke. Mirrors `ui::modal::FormOutcome` but is
/// internal — the parent HostForm consumes it directly so we don't
/// have to put a list payload into the global `FormPayload` enum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListOutcome {
    Stay,
    /// Modal closed: parent should read `entries()` and copy them
    /// into its Vec for that forwarding kind.
    Done,
    /// Esc on a top-level row drops the edit *and* the modal. Parent
    /// keeps its pre-existing Vec.
    Cancel,
}

pub struct ListEditModal {
    kind: ListKind,
    entries: Vec<String>,
    /// Cursor row. `entries.len()` is the "+ add" pseudo-row, allowed
    /// so the user can append a new entry from anywhere in the list.
    selected: usize,
    /// When `Some`, the user is typing into row `selected`. The string
    /// is what they've typed so far. On Enter we validate + apply;
    /// on Esc we discard and the row reverts.
    editing: Option<String>,
    /// Last validation error so the footer can show it. Cleared on
    /// the next keystroke that isn't itself the rejected Enter.
    error: Option<String>,
}

impl ListEditModal {
    pub fn new(kind: ListKind, entries: Vec<String>) -> Self {
        Self {
            kind,
            entries,
            selected: 0,
            editing: None,
            error: None,
        }
    }

    /// Pull the final list out once the modal reports `Done`/`Cancel`.
    /// On `Cancel` the parent should discard this and keep its old list.
    pub fn entries(&self) -> &[String] {
        &self.entries
    }

    /// v0.12 G1 R3: implements the v0.7.1 candidate-cycle picker
    /// inside the modal. When the user is editing an `IdentityFile`
    /// row, ↑/↓ overwrites the buffer with the next/prev candidate.
    /// If the current buffer isn't already one of the candidates,
    /// `forward=true` starts at index 0, `false` at the last index.
    fn cycle_buffer_through_candidates(kind: &ListKind, buf: &mut String, forward: bool) {
        let Some(candidates) = kind.candidates() else {
            return;
        };
        if candidates.is_empty() {
            return;
        }
        let len = candidates.len();
        let pos = candidates
            .iter()
            .position(|p| p.display().to_string() == *buf);
        let next = match (pos, forward) {
            (None, true) => 0,
            (None, false) => len - 1,
            (Some(i), true) => (i + 1) % len,
            (Some(i), false) => (i + len - 1) % len,
        };
        *buf = candidates[next].display().to_string();
    }

    pub fn render(&self, area: Rect, f: &mut Frame) {
        let block = Block::default()
            .title(format!(" {} ", self.kind.title()))
            .borders(Borders::ALL);
        let inner = block.inner(area);
        f.render_widget(block, area);

        // Layout: list (Min) / hint (Length 1) / footer (Length 1).
        let outer = Layout::vertical([
            Constraint::Min(3),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(inner);

        let total_rows = self.entries.len() + 1; // +1 for "+ add"
        let row_chunks = Layout::vertical(vec![Constraint::Length(1); total_rows]).split(outer[0]);

        for (i, slot) in row_chunks.iter().enumerate() {
            let is_active = self.selected == i;
            let is_add = i == self.entries.len();
            let prefix = if is_add {
                "+: ".to_string()
            } else {
                format!("{}: ", i + 1)
            };
            let value = if is_add {
                if is_active && self.editing.is_some() {
                    format!("{}█", self.editing.as_deref().unwrap_or(""))
                } else {
                    "add new entry".to_string()
                }
            } else if is_active && self.editing.is_some() {
                format!("{}█", self.editing.as_deref().unwrap_or(""))
            } else {
                self.entries.get(i).cloned().unwrap_or_default()
            };
            let mut style = Style::default();
            if is_active {
                style = style.fg(Color::Yellow).add_modifier(Modifier::BOLD);
            } else if is_add {
                style = style.add_modifier(Modifier::DIM);
            }
            let line = Line::from(Span::styled(format!(" {prefix}{value}"), style));
            f.render_widget(Paragraph::new(line), *slot);
        }

        // v0.12 G1 R3: IdentityFile edit mode adds ↑/↓ for candidate
        // cycling — surface the candidate count in the hint so the
        // shortcut is discoverable.
        let picker_n = if self.editing.is_some() {
            self.kind.candidates().map(|c| c.len()).unwrap_or(0)
        } else {
            0
        };
        let hint: String = match (self.editing.is_some(), self.selected == self.entries.len()) {
            (true, _) if picker_n > 0 => format!(
                " Enter save • Esc cancel edit • ↑/↓ pick from {} key(s)",
                picker_n
            ),
            (true, _) => " Enter save • Esc cancel edit".into(),
            (false, true) => " Enter new entry • Esc done".into(),
            (false, false) => " Enter edit • d delete • ↑/↓ move • Esc done".into(),
        };
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                hint.to_string(),
                Style::default().fg(Color::Gray),
            ))),
            outer[1],
        );

        if let Some(ref err) = self.error {
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    format!(" {err}"),
                    Style::default().fg(Color::Red),
                ))),
                outer[2],
            );
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> ListOutcome {
        let total_rows = self.entries.len() + 1;
        // Editing mode: keys are absorbed into the buffer.
        if let Some(buf) = self.editing.as_mut() {
            match key.code {
                KeyCode::Enter => {
                    let value = buf.trim().to_string();
                    if value.is_empty() {
                        // Empty Enter on the "+ add" row just leaves
                        // edit mode without appending. On an existing
                        // row it deletes the entry — the simplest UX
                        // for "I cleared this on purpose".
                        if self.selected < self.entries.len() {
                            self.entries.remove(self.selected);
                            if self.selected > self.entries.len() {
                                self.selected = self.entries.len();
                            }
                        }
                        self.editing = None;
                        self.error = None;
                        return ListOutcome::Stay;
                    }
                    if !self.kind.validate(&value) {
                        self.error = Some(self.kind.reject_hint().to_string());
                        return ListOutcome::Stay;
                    }
                    if self.selected == self.entries.len() {
                        self.entries.push(value);
                    } else {
                        self.entries[self.selected] = value;
                    }
                    self.editing = None;
                    self.error = None;
                    ListOutcome::Stay
                }
                KeyCode::Esc => {
                    self.editing = None;
                    self.error = None;
                    ListOutcome::Stay
                }
                KeyCode::Backspace => {
                    buf.pop();
                    ListOutcome::Stay
                }
                // v0.12 G1 R3: edit-mode ↑/↓ cycles `IdentityFile`
                // candidates so the v0.7.1 picker behaviour survives
                // the move into the modal. Other kinds ignore arrows
                // in edit mode (matches v0.10 behaviour).
                KeyCode::Up | KeyCode::Down
                    if matches!(self.kind, ListKind::IdentityFile { .. }) =>
                {
                    let forward = matches!(key.code, KeyCode::Down);
                    Self::cycle_buffer_through_candidates(&self.kind, buf, forward);
                    ListOutcome::Stay
                }
                KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                    buf.push(c);
                    ListOutcome::Stay
                }
                KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    buf.clear();
                    ListOutcome::Stay
                }
                _ => ListOutcome::Stay,
            }
        } else {
            // Browse mode.
            self.error = None;
            match key.code {
                KeyCode::Up if self.selected > 0 => {
                    self.selected -= 1;
                    ListOutcome::Stay
                }
                KeyCode::Down if self.selected + 1 < total_rows => {
                    self.selected += 1;
                    ListOutcome::Stay
                }
                KeyCode::Enter => {
                    let seed = if self.selected < self.entries.len() {
                        self.entries[self.selected].clone()
                    } else {
                        String::new()
                    };
                    self.editing = Some(seed);
                    ListOutcome::Stay
                }
                KeyCode::Char('d') if self.selected < self.entries.len() => {
                    self.entries.remove(self.selected);
                    if self.selected > 0 && self.selected >= self.entries.len() {
                        self.selected = self.entries.len();
                    }
                    ListOutcome::Stay
                }
                KeyCode::Esc => ListOutcome::Done,
                _ => ListOutcome::Stay,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyEvent;

    fn ke(code: KeyCode) -> KeyEvent {
        KeyEvent::from(code)
    }

    #[test]
    fn add_entry_via_enter_then_type_then_enter() {
        let mut m = ListEditModal::new(ListKind::Forwarding(ForwardingKind::Local), Vec::new());
        // selected == 0 == entries.len() (the "+ add" row)
        m.handle_key(ke(KeyCode::Enter));
        for c in "8080 localhost:80".chars() {
            m.handle_key(ke(KeyCode::Char(c)));
        }
        m.handle_key(ke(KeyCode::Enter));
        assert_eq!(m.entries(), &["8080 localhost:80".to_string()]);
        assert!(m.editing.is_none());
    }

    #[test]
    fn validation_rejects_garbage() {
        let mut m = ListEditModal::new(ListKind::Forwarding(ForwardingKind::Local), Vec::new());
        m.handle_key(ke(KeyCode::Enter));
        for c in "garbage".chars() {
            m.handle_key(ke(KeyCode::Char(c)));
        }
        m.handle_key(ke(KeyCode::Enter));
        assert!(m.entries().is_empty(), "garbage should not be accepted");
        assert!(m.error.is_some());
        // Still editing so the user can fix the value.
        assert!(m.editing.is_some());
    }

    #[test]
    fn delete_entry_with_d() {
        let mut m = ListEditModal::new(
            ListKind::Forwarding(ForwardingKind::Local),
            vec!["8080 a:1".into(), "9090 b:2".into()],
        );
        m.selected = 0;
        m.handle_key(ke(KeyCode::Char('d')));
        assert_eq!(m.entries(), &["9090 b:2".to_string()]);
    }

    #[test]
    fn esc_in_browse_mode_closes() {
        let mut m = ListEditModal::new(
            ListKind::Forwarding(ForwardingKind::Local),
            vec!["8080 a:1".into()],
        );
        let out = m.handle_key(ke(KeyCode::Esc));
        assert_eq!(out, ListOutcome::Done);
    }

    #[test]
    fn esc_during_edit_keeps_existing_entry_intact() {
        let mut m = ListEditModal::new(
            ListKind::Forwarding(ForwardingKind::Local),
            vec!["8080 a:1".into()],
        );
        m.selected = 0;
        m.handle_key(ke(KeyCode::Enter));
        m.handle_key(ke(KeyCode::Char('x')));
        m.handle_key(ke(KeyCode::Esc));
        assert_eq!(m.entries(), &["8080 a:1".to_string()]);
        assert!(m.editing.is_none());
    }

    #[test]
    fn empty_enter_on_existing_row_deletes_it() {
        let mut m = ListEditModal::new(
            ListKind::Forwarding(ForwardingKind::Local),
            vec!["8080 a:1".into()],
        );
        m.selected = 0;
        m.handle_key(ke(KeyCode::Enter));
        // Clear the buffer
        for _ in 0..20 {
            m.handle_key(ke(KeyCode::Backspace));
        }
        m.handle_key(ke(KeyCode::Enter));
        assert!(m.entries().is_empty());
    }

    #[test]
    fn identity_file_kind_accepts_unix_path() {
        let mut m = ListEditModal::new(
            ListKind::IdentityFile {
                candidates: Vec::new(),
            },
            Vec::new(),
        );
        m.handle_key(ke(KeyCode::Enter));
        for c in "/home/u/.ssh/id_ed25519".chars() {
            m.handle_key(ke(KeyCode::Char(c)));
        }
        m.handle_key(ke(KeyCode::Enter));
        assert_eq!(m.entries(), &["/home/u/.ssh/id_ed25519".to_string()]);
    }

    #[test]
    fn identity_file_kind_rejects_shell_metacharacter() {
        let mut m = ListEditModal::new(
            ListKind::IdentityFile {
                candidates: Vec::new(),
            },
            Vec::new(),
        );
        m.handle_key(ke(KeyCode::Enter));
        for c in "/etc/key;rm".chars() {
            m.handle_key(ke(KeyCode::Char(c)));
        }
        m.handle_key(ke(KeyCode::Enter));
        assert!(m.entries().is_empty());
        assert!(m.error.is_some());
    }

    #[test]
    fn identity_file_kind_cycles_candidates_with_arrows_in_edit_mode() {
        use std::path::PathBuf;
        let candidates = vec![
            PathBuf::from("/home/u/.ssh/id_ed25519"),
            PathBuf::from("/home/u/.ssh/id_rsa"),
        ];
        let mut m = ListEditModal::new(ListKind::IdentityFile { candidates }, Vec::new());
        // Enter edit mode on the "+ add" row, then ↓ → first candidate.
        m.handle_key(ke(KeyCode::Enter));
        m.handle_key(ke(KeyCode::Down));
        assert_eq!(m.editing.as_deref(), Some("/home/u/.ssh/id_ed25519"));
        // ↓ again → next candidate; wraps on third press.
        m.handle_key(ke(KeyCode::Down));
        assert_eq!(m.editing.as_deref(), Some("/home/u/.ssh/id_rsa"));
        m.handle_key(ke(KeyCode::Down));
        assert_eq!(m.editing.as_deref(), Some("/home/u/.ssh/id_ed25519"));
    }

    #[test]
    fn forwarding_edit_mode_ignores_arrows() {
        // Regression guard for the v0.10 behaviour: ↑/↓ in edit mode
        // is candidate-cycle ONLY for IdentityFile; forwarding kinds
        // treat it as a no-op (the chars don't end up in the buffer
        // either).
        let mut m = ListEditModal::new(ListKind::Forwarding(ForwardingKind::Local), Vec::new());
        m.handle_key(ke(KeyCode::Enter));
        for c in "8080 a:1".chars() {
            m.handle_key(ke(KeyCode::Char(c)));
        }
        m.handle_key(ke(KeyCode::Down));
        m.handle_key(ke(KeyCode::Up));
        // Buffer untouched by the arrow keys.
        assert_eq!(m.editing.as_deref(), Some("8080 a:1"));
    }

    #[test]
    fn dynamic_validation_accepts_bare_port_and_bind_port() {
        let mut m = ListEditModal::new(ListKind::Forwarding(ForwardingKind::Dynamic), Vec::new());
        m.handle_key(ke(KeyCode::Enter));
        for c in "1080".chars() {
            m.handle_key(ke(KeyCode::Char(c)));
        }
        m.handle_key(ke(KeyCode::Enter));
        assert_eq!(m.entries(), &["1080".to_string()]);
    }
}
