use std::time::{Duration, Instant};

pub const STATUS_BAR_TIMEOUT_MS: u64 = 3_000;

#[derive(Debug, Clone)]
pub struct StatusMessage {
    text: String,
    expires_at: Instant,
}

impl StatusMessage {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            expires_at: Instant::now() + Duration::from_millis(STATUS_BAR_TIMEOUT_MS),
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn is_visible(&self) -> bool {
        self.is_visible_at(Instant::now())
    }

    pub(crate) fn is_visible_at(&self, now: Instant) -> bool {
        now < self.expires_at
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
        let future = msg.expires_at + Duration::from_millis(1);
        assert!(!msg.is_visible_at(future));
    }

    #[test]
    fn test_visible_at_exact_deadline_false() {
        let msg = StatusMessage::new("test");
        assert!(!msg.is_visible_at(msg.expires_at));
    }

    #[test]
    fn test_text_accessor() {
        let msg = StatusMessage::new("hello");
        assert_eq!(msg.text(), "hello");
    }
}
