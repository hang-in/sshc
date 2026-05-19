use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListState};

use crate::app::App;
use crate::config::model::Host;

/// Creates the host list widget for rendering.
pub fn create_host_list<'a>(app: &'a App) -> (List<'a>, ListState) {
    let items: Vec<Line<'a>> = app
        .filtered
        .iter()
        .map(|&idx| {
            let host = &app.hosts[idx];
            let hostname = host.hostname.as_deref().unwrap_or("<no hostname>");
            let user = host
                .user
                .as_deref()
                .map(|u| format!("{}@", u))
                .unwrap_or_default();
            let port = host.port.map(|p| format!(":{}", p)).unwrap_or_default();

            Line::from(Span::styled(
                format!("{:<20} {}{}{}", host.alias, user, hostname, port),
                Style::default(),
            ))
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::NONE)
            .style(Style::default()),
    );

    let mut state = ListState::default();
    if !app.filtered.is_empty() {
        state.select(Some(app.selected));
    }

    (list, state)
}

/// Formats the title bar line.
pub fn title_line(host_count: usize, _total: usize) -> Line<'static> {
    Line::from(Span::styled(
        format!(" sshs ({} hosts)", host_count),
        Style::default().add_modifier(Modifier::BOLD),
    ))
}

/// Formats the status bar line.
pub fn status_line(filter_mode: bool, filter_query: &str) -> Line<'static> {
    if filter_mode {
        Line::from(Span::styled(
            format!(" /{}", filter_query),
            Style::default(),
        ))
    } else {
        Line::from(Span::styled(
            " ↑↓/jk navigate  / filter  Enter connect  e edit  q quit".to_string(),
            Style::default(),
        ))
    }
}

/// Formats a single host for display.
pub fn format_host(host: &Host) -> String {
    host.to_string()
}
