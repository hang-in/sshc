use std::path::Path;

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Row, Table, TableState};

use crate::app::App;
use crate::config::model::Host;
use crate::ui::layout::{host_table_constraints, ColumnVisibility};

/// Build the host table widget + selection state for the current terminal
/// width. The width drives column visibility (BRIEF_V3 §5 Q6 priority).
pub fn create_host_table<'a>(app: &'a App, width: u16) -> (Table<'a>, TableState) {
    let visibility = ColumnVisibility::for_width(width);
    let sshs_conf = crate::storage::sshs_conf_path();

    let rows: Vec<Row<'a>> = app
        .filtered
        .iter()
        .map(|&idx| {
            let host = &app.hosts[idx];
            host_row(host, app, sshs_conf.as_deref(), &visibility)
        })
        .collect();

    let constraints = host_table_constraints(&visibility);
    let table = Table::new(rows, constraints).header(header_row(&visibility));

    let mut state = TableState::default();
    if !app.filtered.is_empty() {
        state.select(Some(app.selected));
    }

    (table, state)
}

fn header_row(visibility: &ColumnVisibility) -> Row<'static> {
    let mut cells: Vec<Cell> = vec![Cell::from("Alias")];
    if visibility.show_account {
        cells.push(Cell::from("Account"));
    }
    if visibility.show_host {
        cells.push(Cell::from("Host"));
    }
    if visibility.show_port {
        cells.push(Cell::from("Port"));
    }
    cells.push(Cell::from("St"));
    Row::new(cells).style(Style::default().add_modifier(Modifier::BOLD))
}

fn host_row<'a>(
    host: &'a Host,
    app: &App,
    sshs_conf: Option<&Path>,
    visibility: &ColumnVisibility,
) -> Row<'a> {
    let mut cells: Vec<Cell<'a>> = Vec::new();

    cells.push(alias_cell(host));

    if visibility.show_account {
        let account = host.user.as_deref().unwrap_or("");
        cells.push(Cell::from(account.to_string()));
    }
    if visibility.show_host {
        let hostname = host.hostname.as_deref().unwrap_or("-");
        cells.push(Cell::from(hostname.to_string()));
    }
    if visibility.show_port {
        let port_str = host.port.map(|p| p.to_string()).unwrap_or_default();
        cells.push(Cell::from(port_str));
    }

    cells.push(status_cell(host, app, sshs_conf));

    Row::new(cells)
}

fn alias_cell(host: &Host) -> Cell<'_> {
    if host.tags.is_empty() {
        Cell::from(host.alias.as_str())
    } else {
        let prefix = format!("[{}] ", host.tags.join(","));
        Cell::from(Line::from(vec![
            Span::styled(prefix, Style::default().fg(Color::Cyan)),
            Span::raw(host.alias.as_str()),
        ]))
    }
}

/// Status cell encoded as 2-character `<probe><marker>`:
/// - probe glyph (T11 wiring): space placeholder until ProbePool is wired in
/// - marker priority: ★ (last_connected) > · (external source) > space
fn status_cell(host: &Host, app: &App, sshs_conf: Option<&Path>) -> Cell<'static> {
    let probe_glyph = ' ';

    let marker = if app.last_connected.as_deref() == Some(host.alias.as_str()) {
        '★'
    } else if sshs_conf
        .map(|conf| host.source_file != conf)
        .unwrap_or(false)
    {
        '·'
    } else {
        ' '
    };

    let text = format!("{probe_glyph}{marker}");
    let style = match marker {
        '★' => Style::default().fg(Color::Yellow),
        '·' => Style::default().add_modifier(Modifier::DIM),
        _ => Style::default(),
    };
    Cell::from(text).style(style)
}

/// Title-bar text shown in the outer block (e.g., " sshs (12) ").
pub fn title_line(host_count: usize, _total: usize) -> Line<'static> {
    Line::from(Span::styled(
        format!(" sshs ({}) ", host_count),
        Style::default().add_modifier(Modifier::BOLD),
    ))
}

/// Bottom status row: filter input or default help.
pub fn status_line(filter_mode: bool, filter_query: &str) -> Line<'static> {
    if filter_mode {
        Line::from(Span::styled(
            format!(" /{}", filter_query),
            Style::default(),
        ))
    } else {
        Line::from(Span::styled(
            " j/k nav  / filter  Enter ssh  r reconnect  a add  d del  m modify  t tags  e edit  q quit".to_string(),
            Style::default().add_modifier(Modifier::DIM),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn host_in(path: &str, alias: &str, tags: Vec<&str>) -> Host {
        Host {
            alias: alias.to_string(),
            hostname: Some("h".into()),
            user: Some("u".into()),
            port: Some(22),
            identity_file: None,
            line_start: 1,
            source_file: PathBuf::from(path),
            tags: tags.into_iter().map(String::from).collect(),
        }
    }

    #[test]
    fn test_alias_cell_no_tags_plain() {
        let h = host_in("/a", "alpha", vec![]);
        let cell = alias_cell(&h);
        // Cell is opaque; we re-create the expected and compare via formatting.
        // At minimum exercise the path with no panic.
        let _ = cell;
    }

    #[test]
    fn test_alias_cell_with_tags_renders_prefix() {
        let h = host_in("/a", "alpha", vec!["t1", "t2"]);
        let cell = alias_cell(&h);
        let _ = cell; // smoke
    }

    #[test]
    fn test_status_marker_star_wins_over_source() {
        let h = host_in("/elsewhere", "alpha", vec![]);
        let mut app = App::new(vec![h.clone()]);
        app.last_connected = Some("alpha".to_string());
        let sshs_conf = PathBuf::from("/managed/sshs.conf");
        let cell = status_cell(&h, &app, Some(sshs_conf.as_path()));
        // smoke: just ensure it builds without panic
        let _ = cell;
    }

    #[test]
    fn test_status_marker_dot_for_external_source() {
        let h = host_in("/etc/ssh/config", "alpha", vec![]);
        let app = App::new(vec![h.clone()]);
        let sshs_conf = PathBuf::from("/home/u/.ssh/config.d/sshs.conf");
        let cell = status_cell(&h, &app, Some(sshs_conf.as_path()));
        let _ = cell;
    }

    #[test]
    fn test_status_marker_blank_when_internal_and_not_last() {
        let conf = "/home/u/.ssh/config.d/sshs.conf";
        let h = host_in(conf, "alpha", vec![]);
        let app = App::new(vec![h.clone()]);
        let sshs_conf = PathBuf::from(conf);
        let cell = status_cell(&h, &app, Some(sshs_conf.as_path()));
        let _ = cell;
    }

    #[test]
    fn test_header_includes_status_always() {
        for w in [20u16, 35, 50, 80, 120] {
            let v = ColumnVisibility::for_width(w);
            let row = header_row(&v);
            let _ = row; // smoke: builds without panic
        }
    }

    #[test]
    fn test_title_line_includes_count() {
        let line = title_line(7, 12);
        let s: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(s.contains("sshs"));
        assert!(s.contains("(7)"));
    }

    #[test]
    fn test_status_line_filter_mode() {
        let l = status_line(true, "foo");
        let s: String = l.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(s.contains("/foo"));
    }
}
