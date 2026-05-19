use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListState};

use crate::app::App;

/// Column widths for aligned display.
pub const ALIAS_WIDTH: usize = 18;
pub const PORT_WIDTH: usize = 6;

/// Creates the host list widget for rendering with table-like column alignment.
pub fn create_host_list<'a>(app: &'a App) -> (List<'a>, ListState) {
    let items: Vec<Line<'a>> = app
        .filtered
        .iter()
        .map(|&idx| {
            let host = &app.hosts[idx];
            let alias = host.alias.as_str();
            let hostname = host.hostname.as_deref().unwrap_or("-");
            let user = host.user.as_deref().unwrap_or("");
            let port_str = host.port.map(|p| p.to_string()).unwrap_or_default();

            let host_display = if user.is_empty() {
                hostname.to_string()
            } else {
                format!("{}@{}", user, hostname)
            };

            // Truncate alias if too long, pad if too short
            let alias_display = if alias.len() > ALIAS_WIDTH {
                format!("{}…", &alias[..ALIAS_WIDTH - 1])
            } else {
                format!("{:<width$}", alias, width = ALIAS_WIDTH)
            };

            // Right-align port
            let port_display = if port_str.is_empty() {
                format!("{:>width$}", "", width = PORT_WIDTH)
            } else {
                format!("{:>width$}", port_str, width = PORT_WIDTH)
            };

            Line::from(vec![
                Span::styled(format!("{}  ", alias_display), Style::default()),
                Span::styled(host_display, Style::default()),
                Span::styled(format!("  {}", port_display), Style::default()),
            ])
        })
        .collect();

    let list = List::new(items);

    let mut state = ListState::default();
    if !app.filtered.is_empty() {
        state.select(Some(app.selected));
    }

    (list, state)
}

/// Formats the title bar line.
pub fn title_line(host_count: usize, _total: usize) -> Line<'static> {
    Line::from(Span::styled(
        format!(" sshs ({})", host_count),
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
            " j/k nav  / filter  Enter ssh  e edit  q quit".to_string(),
            Style::default().add_modifier(Modifier::DIM),
        ))
    }
}
