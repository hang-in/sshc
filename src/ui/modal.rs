use crossterm::event::KeyEvent;
use ratatui::layout::{Alignment, Margin, Rect};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

/// What kind of modal is currently displayed.
pub enum ModalKind {
    Confirmation {
        prompt: String,
        on_yes: ModalAction,
        on_no: ModalAction,
    },
    Info {
        message: String,
        dismiss: ModalAction,
    },
    Form(Box<dyn FormState>),
}

/// Action delivered back to the App when a modal closes / a button is clicked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModalAction {
    None,
    Custom(String),
}

/// Stateful form trait. Each form implementation owns its field buffers
/// and cursor, renders itself, and returns FormOutcome per keystroke.
pub trait FormState: Send {
    fn render(&self, area: Rect, f: &mut Frame);
    fn handle_key(&mut self, key: KeyEvent) -> FormOutcome;
}

/// Result of handling a key inside a form.
///
/// `Submit` carries the bulky `FormPayload` (see below); the same allow
/// rationale applies — outcomes are short-lived stack values.
#[allow(clippy::large_enum_variant)]
pub enum FormOutcome {
    Stay,
    Cancel,
    Submit(FormPayload),
}

/// Opaque form-result payload. Concrete form types put their data here.
///
/// `Host` is intentionally larger than its peers (10 typed String fields
/// once v0.9 G5 forwarding lands). It's constructed in exactly one place
/// per submission and lives on the stack only across that call — the
/// usual `Box` indirection rationale doesn't apply.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormPayload {
    Host {
        alias: String,
        hostname: String,
        user: String,
        port: String,
        identity_file: String,
        tags_csv: String,
        /// Freeform SSH config directives (one per line, e.g.
        /// `ProxyJump bastion`). Multi-line edit not implemented in v0.4;
        /// the form accepts semicolon-separated entries and splits on `;`.
        extra: String,
        /// v0.9 G5: typed Forwarding fields. Empty string ↔ unset.
        local_forward: String,
        remote_forward: String,
        dynamic_forward: String,
    },
    Tags {
        tags_csv: String,
    },
    Text(String),
}

/// Compute a centered Rect over `area` of approximately `width_pct` x `height_pct`.
pub fn centered_rect(area: Rect, width_pct: u16, height_pct: u16) -> Rect {
    let width = (area.width as u32 * width_pct as u32 / 100) as u16;
    let height = (area.height as u32 * height_pct as u32 / 100) as u16;
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect {
        x,
        y,
        width,
        height,
    }
}

/// Render the chrome (border + title) for any modal kind.
pub fn render_modal_chrome(area: Rect, f: &mut Frame, title: &str) {
    let block = Block::default()
        .title(title.to_string())
        .borders(Borders::ALL);
    f.render_widget(block, area);
}

/// Render a Confirmation modal body (prompt + Yes/No hint).
pub fn render_confirmation_body(area: Rect, f: &mut Frame, prompt: &str) {
    let inner = area.inner(Margin::new(1, 1));
    let text = format!("{}\n\n[Y] Yes  [N] No", prompt);
    let para = Paragraph::new(text).alignment(Alignment::Center);
    f.render_widget(para, inner);
}

/// Render an Info modal body (message + Enter-to-dismiss hint).
pub fn render_info_body(area: Rect, f: &mut Frame, message: &str) {
    let inner = area.inner(Margin::new(1, 1));
    let text = format!("{}\n\nPress Enter to dismiss", message);
    let para = Paragraph::new(text).alignment(Alignment::Center);
    f.render_widget(para, inner);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_centered_rect_centered() {
        let area = Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 50,
        };
        let r = centered_rect(area, 50, 50);
        assert_eq!(r.width, 50);
        assert_eq!(r.height, 25);
        assert_eq!(r.x, 25);
        assert!(r.y == 12 || r.y == 13); // integer truncation tolerance
    }

    #[test]
    fn test_modal_action_equality() {
        assert_eq!(
            ModalAction::Custom("a".into()),
            ModalAction::Custom("a".into())
        );
    }

    #[test]
    fn test_form_payload_clone() {
        let original = FormPayload::Tags {
            tags_csv: "a".into(),
        };
        let cloned = original.clone();
        assert_eq!(original, cloned);
    }
}
