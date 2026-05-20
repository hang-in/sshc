use crate::config::tags::normalize_tag;
use crate::ui::modal::{FormOutcome, FormPayload, FormState};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

const FIELD_COUNT: usize = 7;

pub struct HostForm {
    fields: [String; FIELD_COUNT],
    active_index: usize,
    error: Option<String>,
}

impl HostForm {
    pub fn new() -> Self {
        Self {
            fields: Default::default(),
            active_index: 0,
            error: None,
        }
    }

    pub fn from_host(
        alias: &str,
        hostname: &str,
        user: &str,
        port: &str,
        identity_file: &str,
        tags_csv: &str,
        extra: &str,
    ) -> Self {
        Self {
            fields: [
                alias.to_string(),
                hostname.to_string(),
                user.to_string(),
                port.to_string(),
                identity_file.to_string(),
                tags_csv.to_string(),
                extra.to_string(),
            ],
            active_index: 0,
            error: None,
        }
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

        let id_file = self.fields[4].trim();
        let forbidden = [
            ';', '|', '&', '$', '`', '<', '>', '(', ')', '{', '}', '*', '?', '[', ']', '"', '\'',
            '\\',
        ];
        if id_file.chars().any(|c| forbidden.contains(&c)) {
            return Err("IdentityFile contains forbidden shell characters".to_string());
        }

        let tags_csv = self.fields[5].trim();
        let distinct: std::collections::HashSet<_> =
            tags_csv.split(',').filter_map(normalize_tag).collect();
        if distinct.len() > 16 {
            return Err("Maximum 16 distinct tags allowed".to_string());
        }

        let extra = self.fields[6].trim();

        Ok(FormPayload::Host {
            alias: alias.to_string(),
            hostname: hostname.to_string(),
            user: self.fields[2].trim().to_string(),
            port: port_str.to_string(),
            identity_file: id_file.to_string(),
            tags_csv: tags_csv.to_string(),
            extra: extra.to_string(),
        })
    }
}

impl Default for HostForm {
    fn default() -> Self {
        Self::new()
    }
}

impl FormState for HostForm {
    fn render(&self, area: Rect, f: &mut Frame) {
        let block = Block::default().title("Host").borders(Borders::ALL);
        let inner = block.inner(area);
        f.render_widget(block, area);

        let labels = [
            "Alias",
            "HostName",
            "User",
            "Port",
            "IdentityFile",
            "Tags",
            "Options (a; b)",
        ];

        let outer = Layout::vertical([
            Constraint::Length((FIELD_COUNT as u16) * 2),
            Constraint::Min(1),
        ])
        .split(inner);
        let field_chunks = Layout::vertical([Constraint::Length(2); FIELD_COUNT]).split(outer[0]);

        for i in 0..FIELD_COUNT {
            let is_active = self.active_index == i;
            let label = Line::from(format!("{}:", labels[i]));
            let value_style = if is_active {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            let cursor = if is_active { "█" } else { " " };
            let value_text = format!("[ {}{} ]", self.fields[i], cursor);
            let value_line = Line::from(Span::styled(value_text, value_style));

            let field_area = field_chunks[i];
            f.render_widget(
                Paragraph::new(label),
                Rect::new(field_area.x, field_area.y, field_area.width, 1),
            );
            f.render_widget(
                Paragraph::new(value_line),
                Rect::new(field_area.x, field_area.y + 1, field_area.width, 1),
            );
        }

        let footer_line = if let Some(ref err) = self.error {
            Line::from(Span::styled(err.clone(), Style::default().fg(Color::Red)))
        } else {
            Line::from(Span::styled(
                "Tab/Shift-Tab move • Enter submit • Esc cancel • Ctrl-U clear",
                Style::default().fg(Color::Gray),
            ))
        };
        f.render_widget(Paragraph::new(footer_line), outer[1]);
    }

    fn handle_key(&mut self, key: KeyEvent) -> FormOutcome {
        match key.code {
            KeyCode::Tab => {
                self.active_index = (self.active_index + 1) % FIELD_COUNT;
                FormOutcome::Stay
            }
            KeyCode::BackTab => {
                self.active_index = (self.active_index + FIELD_COUNT - 1) % FIELD_COUNT;
                FormOutcome::Stay
            }
            KeyCode::Backspace => {
                self.fields[self.active_index].pop();
                FormOutcome::Stay
            }
            KeyCode::Esc => FormOutcome::Cancel,
            KeyCode::Enter => {
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
        let form = HostForm::new();
        assert_eq!(form.active_index, 0);
        assert!(form.error.is_none());
        assert!(form.fields.iter().all(|s| s.is_empty()));
    }

    #[test]
    fn test_tab_wraparound() {
        let mut form = HostForm::new();
        for expected in &[1usize, 2, 3, 4, 5, 6, 0] {
            form.handle_key(ke(KeyCode::Tab));
            assert_eq!(form.active_index, *expected);
        }
    }

    #[test]
    fn test_backtab_wraparound() {
        let mut form = HostForm::new();
        for expected in &[6usize, 5, 4, 3, 2, 1, 0] {
            form.handle_key(ke(KeyCode::BackTab));
            assert_eq!(form.active_index, *expected);
        }
    }

    #[test]
    fn test_char_backspace_ctrl_u() {
        let mut form = HostForm::new();
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
        let mut form = HostForm::new();
        assert!(matches!(
            form.handle_key(ke(KeyCode::Esc)),
            FormOutcome::Cancel
        ));
    }

    #[test]
    fn test_enter_advances_non_last_field() {
        let mut form = HostForm::new();
        form.handle_key(ke(KeyCode::Enter));
        assert_eq!(form.active_index, 1);
    }

    #[test]
    fn test_enter_on_last_field_submits_when_valid() {
        let mut form = HostForm::new();
        form.fields[0] = "dev1".to_string();
        form.fields[1] = "10.0.0.1".to_string();
        form.active_index = 6;
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
        let mut form = HostForm::new();
        form.fields[1] = "host".to_string();
        form.active_index = 6;
        assert!(matches!(
            form.handle_key(ke(KeyCode::Enter)),
            FormOutcome::Stay
        ));
        assert!(form.error.is_some());
    }

    #[test]
    fn test_validation_invalid_port() {
        let mut form = HostForm::new();
        form.fields[0] = "dev".to_string();
        form.fields[1] = "h".to_string();
        form.fields[3] = "70000".to_string();
        form.active_index = 6;
        assert!(matches!(
            form.handle_key(ke(KeyCode::Enter)),
            FormOutcome::Stay
        ));
        assert!(form.error.is_some());
    }

    #[test]
    fn test_validation_shell_metachar() {
        let mut form = HostForm::new();
        form.fields[0] = "dev".to_string();
        form.fields[1] = "h".to_string();
        form.fields[4] = "/etc/key;rm".to_string();
        form.active_index = 6;
        assert!(matches!(
            form.handle_key(ke(KeyCode::Enter)),
            FormOutcome::Stay
        ));
        assert!(form.error.is_some());
    }

    #[test]
    fn test_from_host_populates_fields() {
        let form = HostForm::from_host("a", "h", "u", "22", "/k", "x,y", "ProxyJump bastion");
        assert_eq!(form.fields[0], "a");
        assert_eq!(form.fields[1], "h");
        assert_eq!(form.fields[2], "u");
        assert_eq!(form.fields[3], "22");
        assert_eq!(form.fields[4], "/k");
        assert_eq!(form.fields[5], "x,y");
        assert_eq!(form.fields[6], "ProxyJump bastion");
    }
}
