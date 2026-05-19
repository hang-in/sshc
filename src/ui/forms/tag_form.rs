use crate::config::tags::normalize_tag;
use crate::ui::modal::{FormOutcome, FormPayload, FormState};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub struct TagForm {
    buffer: String,
    error: Option<String>,
}

impl TagForm {
    pub fn new(initial_tags_csv: String) -> Self {
        Self {
            buffer: initial_tags_csv,
            error: None,
        }
    }

    fn validate(&self) -> Result<FormPayload, String> {
        let trimmed = self.buffer.trim();
        let distinct: std::collections::HashSet<_> =
            trimmed.split(',').filter_map(normalize_tag).collect();
        if distinct.len() > 16 {
            return Err("Maximum 16 distinct tags allowed".to_string());
        }
        Ok(FormPayload::Tags {
            tags_csv: trimmed.to_string(),
        })
    }
}

impl FormState for TagForm {
    fn render(&self, area: Rect, f: &mut Frame) {
        let block = Block::default().title("Tags").borders(Borders::ALL);
        let inner = block.inner(area);
        f.render_widget(block, area);

        let chunks = Layout::vertical([Constraint::Length(3), Constraint::Min(1)]).split(inner);

        let value_text = format!("[ {}█ ]", self.buffer);
        f.render_widget(Paragraph::new(value_text), chunks[0]);

        let footer_line = if let Some(ref err) = self.error {
            Line::from(Span::styled(err.clone(), Style::default().fg(Color::Red)))
        } else {
            Line::from(Span::styled(
                "Enter submit • Esc cancel • Ctrl-U clear",
                Style::default().fg(Color::Gray),
            ))
        };
        f.render_widget(Paragraph::new(footer_line), chunks[1]);
    }

    fn handle_key(&mut self, key: KeyEvent) -> FormOutcome {
        match key.code {
            KeyCode::Esc => FormOutcome::Cancel,
            KeyCode::Enter => match self.validate() {
                Ok(payload) => FormOutcome::Submit(payload),
                Err(e) => {
                    self.error = Some(e);
                    FormOutcome::Stay
                }
            },
            KeyCode::Backspace => {
                self.buffer.pop();
                FormOutcome::Stay
            }
            KeyCode::Char(c) => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    if c == 'u' || c == 'U' {
                        self.buffer.clear();
                    }
                } else {
                    self.buffer.push(c);
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
    use crossterm::event::{KeyEvent, KeyModifiers};

    fn ke(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::empty())
    }

    #[test]
    fn test_new_and_edit() {
        let mut form = TagForm::new("a,b".to_string());
        assert_eq!(form.buffer, "a,b");
        form.handle_key(ke(KeyCode::Char('c')));
        assert_eq!(form.buffer, "a,bc");
        form.handle_key(ke(KeyCode::Backspace));
        assert_eq!(form.buffer, "a,b");
    }

    #[test]
    fn test_submit_returns_trimmed_csv() {
        let mut form = TagForm::new("tag1, tag2".to_string());
        match form.handle_key(ke(KeyCode::Enter)) {
            FormOutcome::Submit(FormPayload::Tags { tags_csv }) => {
                assert_eq!(tags_csv, "tag1, tag2");
            }
            _ => panic!("expected Submit(Tags)"),
        }
    }

    #[test]
    fn test_esc_cancels() {
        let mut form = TagForm::new(String::new());
        assert!(matches!(
            form.handle_key(ke(KeyCode::Esc)),
            FormOutcome::Cancel
        ));
    }

    #[test]
    fn test_too_many_tags_stays() {
        let csv = (0..20)
            .map(|i| format!("t{i}"))
            .collect::<Vec<_>>()
            .join(",");
        let mut form = TagForm::new(csv);
        assert!(matches!(
            form.handle_key(ke(KeyCode::Enter)),
            FormOutcome::Stay
        ));
        assert!(form.error.is_some());
    }
}
