//! v0.6 manage-mode preview panel.
//!
//! Renders a key/value detail view of one host to the right of the host
//! table. The caller decides whether there's room — `render_preview`
//! just paints into the rect it's given.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::config::model::Host;

/// Render the host detail into `area`. The widget paints a left-border
/// separator + a title so it visually attaches to the host table on its
/// left. Long values (Tags, Extra, IdentityFile) wrap.
pub fn render_preview(host: &Host, area: Rect, f: &mut Frame) {
    let block = Block::default()
        .borders(Borders::LEFT)
        .title(Line::from(Span::styled(
            " Detail ",
            Style::default().add_modifier(Modifier::BOLD),
        )));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let identity = host
        .identity_file
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "—".into());
    let port = host
        .port
        .map(|p| p.to_string())
        .unwrap_or_else(|| "—".into());
    let tags = if host.tags.is_empty() {
        "—".to_string()
    } else {
        host.tags.join(", ")
    };
    let extra = if host.extra.is_empty() {
        "—".to_string()
    } else {
        host.extra.join("; ")
    };

    let lines = vec![
        kv_line("Alias", &host.alias),
        kv_line("HostName", host.hostname.as_deref().unwrap_or("—")),
        kv_line("User", host.user.as_deref().unwrap_or("—")),
        kv_line("Port", &port),
        kv_line("Identity", &identity),
        Line::from(""),
        kv_line("Tags", &tags),
        Line::from(""),
        kv_line("Extra", &extra),
    ];

    let para = Paragraph::new(lines).wrap(Wrap { trim: false });
    f.render_widget(para, inner);
}

fn kv_line<'a>(label: &'a str, value: &'a str) -> Line<'a> {
    Line::from(vec![
        Span::styled(
            format!(" {label:>9}: "),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(value.to_string()),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use std::path::PathBuf;

    fn make_host(alias: &str) -> Host {
        Host {
            alias: alias.to_string(),
            hostname: Some(format!("{alias}.example.com")),
            user: Some("deploy".into()),
            port: Some(22),
            identity_file: Some(PathBuf::from("/home/u/.ssh/id_ed25519")),
            line_start: 1,
            source_file: PathBuf::from("/tmp/sshc.conf"),
            tags: vec!["prod".into(), "api".into()],
            extra: vec!["ForwardAgent yes".into()],
            local_forward: None,
            remote_forward: None,
            dynamic_forward: None,
        }
    }

    fn render_with(host: &Host, width: u16, height: u16) -> Terminal<TestBackend> {
        let backend = TestBackend::new(width, height);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| {
            let area = Rect::new(0, 0, width, height);
            render_preview(host, area, f);
        })
        .unwrap();
        term
    }

    #[test]
    fn renders_without_panic_for_full_host() {
        let _ = render_with(&make_host("api-1"), 36, 12);
    }

    #[test]
    fn renders_without_panic_for_minimal_host() {
        let mut h = make_host("a");
        h.hostname = None;
        h.user = None;
        h.port = None;
        h.identity_file = None;
        h.tags.clear();
        h.extra.clear();
        let _ = render_with(&h, 36, 12);
    }

    #[test]
    fn renders_with_long_extra() {
        let mut h = make_host("a");
        h.extra = vec![
            "ProxyJump bastion.example.com".into(),
            "ServerAliveInterval 30".into(),
            "ForwardAgent yes".into(),
        ];
        let _ = render_with(&h, 40, 16);
    }
}
