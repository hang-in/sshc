use std::time::{Duration, Instant};

pub const STATUS_BAR_TIMEOUT_MS: u64 = 3_000;

/// How a `StatusMessage` should expire.
///
/// - `Info` — v0.6 transient behavior: auto-fades after
///   `STATUS_BAR_TIMEOUT_MS` ms. Used for routine confirmations
///   ("`<alias>` pinned", "validating…", "`<alias>` promoted to
///   sshc.conf").
/// - `Error` — v0.9 G3 sticky behavior: stays visible until the
///   user's next keystroke. Used for failures the user must see
///   before form-close redraw would otherwise overwrite the bar
///   (apply_form Err, persist_sshc_conf Err, include injection Err,
///   etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusKind {
    Info,
    Error,
}

#[derive(Debug, Clone)]
pub struct StatusMessage {
    text: String,
    kind: StatusKind,
    created_at: Instant,
}

impl StatusMessage {
    /// Build a transient (Info) status message — back-compat shape
    /// used by the bulk of call sites (pin toggle, "saved without
    /// IdentityFile" hints, modal-rejection notices). Behaves like
    /// the v0.6 status bar: auto-hides after `STATUS_BAR_TIMEOUT_MS`.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind: StatusKind::Info,
            created_at: Instant::now(),
        }
    }

    /// Build a sticky (Error) status message. The bar will keep
    /// showing it across redraws until the user's next keystroke,
    /// at which point `App::clear_sticky_error_status` will drop it.
    pub fn error(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind: StatusKind::Error,
            created_at: Instant::now(),
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn kind(&self) -> StatusKind {
        self.kind
    }

    pub fn is_visible(&self) -> bool {
        self.is_visible_at(Instant::now())
    }

    pub(crate) fn is_visible_at(&self, now: Instant) -> bool {
        match self.kind {
            // Error messages stay visible until explicit dismissal —
            // App::clear_sticky_error_status handles the user-action
            // path.
            StatusKind::Error => true,
            StatusKind::Info => {
                now < self.created_at + Duration::from_millis(STATUS_BAR_TIMEOUT_MS)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_visible_immediately() {
        let msg = StatusMessage::new("test");
        assert!(msg.is_visible());
    }

    #[test]
    fn test_hidden_after_timeout() {
        let msg = StatusMessage::new("test");
        let future = msg.created_at + Duration::from_millis(STATUS_BAR_TIMEOUT_MS + 1);
        assert!(!msg.is_visible_at(future));
    }

    #[test]
    fn test_visible_at_exact_deadline_false() {
        let msg = StatusMessage::new("test");
        let deadline = msg.created_at + Duration::from_millis(STATUS_BAR_TIMEOUT_MS);
        assert!(!msg.is_visible_at(deadline));
    }

    #[test]
    fn test_text_accessor() {
        let msg = StatusMessage::new("hello");
        assert_eq!(msg.text(), "hello");
    }

    // ----- v0.9 G3: sticky error semantics -----

    #[test]
    fn test_info_expires_by_timeout() {
        let msg = StatusMessage::new("info");
        assert_eq!(msg.kind(), StatusKind::Info);
        let future = msg.created_at + Duration::from_millis(STATUS_BAR_TIMEOUT_MS + 1);
        assert!(!msg.is_visible_at(future));
    }

    #[test]
    fn test_error_stays_visible_past_timeout() {
        // The whole point of G3 — sticky on error means the user
        // sees the failure even if the form-close redraw or any
        // periodic tick would normally have rolled the bar.
        let msg = StatusMessage::error("save failed");
        assert_eq!(msg.kind(), StatusKind::Error);
        let way_later = msg.created_at + Duration::from_secs(60);
        assert!(msg.is_visible_at(way_later));
    }

    #[test]
    fn test_error_visible_at_creation() {
        let msg = StatusMessage::error("immediate fail");
        assert!(msg.is_visible());
    }
}
