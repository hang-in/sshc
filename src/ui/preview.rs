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

    // v0.12 G1: identity_file is now a Vec. Show the first entry or
    // "<n>: <first> +N more" for the multi case; empty Vec → em-dash.
    let identity = match host.identity_file.as_slice() {
        [] => "—".to_string(),
        [single] => single.display().to_string(),
        [first, rest @ ..] => format!("{} (+{} more)", first.display(), rest.len()),
    };
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

    // v0.13 G2: summarise the typed Forwarding fields right under
    // Identity. Lifecycle parity with v0.6 Extra — if every kind is
    // empty, skip the section entirely (no row, no blank line).
    let total_forwarding =
        host.local_forward.len() + host.remote_forward.len() + host.dynamic_forward.len();
    let forwarding_summary = format!(
        "L:{} R:{} D:{}",
        host.local_forward.len(),
        host.remote_forward.len(),
        host.dynamic_forward.len()
    );
    let forwarding_first = host
        .local_forward
        .first()
        .or(host.remote_forward.first())
        .or(host.dynamic_forward.first())
        .cloned()
        .unwrap_or_default();

    let mut lines = vec![
        kv_line("Alias", &host.alias),
        kv_line("HostName", host.hostname.as_deref().unwrap_or("—")),
        kv_line("User", host.user.as_deref().unwrap_or("—")),
        kv_line("Port", &port),
        kv_line("Identity", &identity),
    ];
    if total_forwarding > 0 {
        lines.push(kv_line("Forward", &forwarding_summary));
        // Show the first entry across the three kinds so the
        // preview surfaces the *content*, not just the count. The
        // entry is shown without an extra label cell so it lines
        // up under the Forward summary; wrap is on for long
        // values.
        lines.push(Line::from(Span::raw(format!(
            "            {forwarding_first}"
        ))));
    }
    lines.push(Line::from(""));
    lines.push(kv_line("Tags", &tags));
    lines.push(Line::from(""));
    lines.push(kv_line("Extra", &extra));

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
            identity_file: vec![PathBuf::from("/home/u/.ssh/id_ed25519")],
            line_start: 1,
            source_file: PathBuf::from("/tmp/sshc.conf"),
            tags: vec!["prod".into(), "api".into()],
            extra: vec!["ForwardAgent yes".into()],
            local_forward: Vec::new(),
            remote_forward: Vec::new(),
            dynamic_forward: Vec::new(),
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
        h.identity_file.clear();
        h.tags.clear();
        h.extra.clear();
        let _ = render_with(&h, 36, 12);
    }

    /// Walk every cell of the rendered TestBackend buffer and join it
    /// into a single string per row. Useful for asserting that a
    /// substring landed somewhere in the preview without caring
    /// about its (x, y) coordinates.
    fn render_text(term: &Terminal<TestBackend>) -> String {
        let mut s = String::new();
        let buf = term.backend().buffer();
        for y in 0..buf.area().height {
            for x in 0..buf.area().width {
                s.push_str(buf[(x, y)].symbol());
            }
            s.push('\n');
        }
        s
    }

    #[test]
    fn renders_with_multi_forwarding_shows_counts_and_first_entry() {
        let mut h = make_host("api-fwd");
        h.local_forward = vec!["8080 localhost:80".into(), "9090 db.internal:5432".into()];
        h.remote_forward = vec!["9000 127.0.0.1:9000".into()];
        let term = render_with(&h, 60, 16);
        let txt = render_text(&term);
        assert!(txt.contains("Forward"), "Forward row missing: {txt}");
        assert!(txt.contains("L:2 R:1 D:0"), "counts wrong: {txt}");
        assert!(
            txt.contains("8080 localhost:80"),
            "first entry missing: {txt}"
        );
    }

    #[test]
    fn renders_without_forwarding_omits_section() {
        // No forwards → no `Forward:` row at all (no count of 0,
        // no blank line gap). Matches the Identity → Tags → Extra
        // flow that v0.12 shipped. (NB: substring "Forward" also
        // shows up inside `ForwardAgent` lines in `Extra`; we check
        // the label-form `Forward:` to disambiguate.)
        let mut h = make_host("plain");
        h.extra.clear(); // strip the ForwardAgent fixture entry
        let term = render_with(&h, 60, 12);
        let txt = render_text(&term);
        assert!(
            !txt.contains("Forward:"),
            "Forward row should be absent for empty forwards: {txt}"
        );
        // And the L:0 R:0 D:0 string never lands when the section
        // is omitted.
        assert!(
            !txt.contains("L:0 R:0 D:0"),
            "empty counts should not be rendered: {txt}"
        );
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
