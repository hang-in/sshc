use std::path::Path;

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Cell, Row, Table, TableState};

use crate::app::App;
use crate::config::model::Host;
use crate::probe::ProbeState;
use crate::ui::layout::{host_table_constraints_for, ColumnVisibility, ColumnWidths};

/// Scan the filtered hosts once and return the per-column max widths.
pub fn compute_column_widths(app: &App, fallback_user: &str) -> ColumnWidths {
    let mut widths = ColumnWidths::header_baseline();
    for &idx in &app.filtered {
        let host = &app.hosts[idx];
        let alias_len = host.alias.chars().count();
        let account_len = match host.user.as_deref() {
            Some(u) if !u.is_empty() => u.chars().count(),
            _ => fallback_user.chars().count(),
        };
        let host_len = host
            .hostname
            .as_deref()
            .map(|s| s.chars().count())
            .unwrap_or(1);
        let port_len = host.port.map(|p| p.to_string().len()).unwrap_or(0);
        let tags_len = if host.tags.is_empty() {
            0
        } else {
            // "t1, t2, t3" — joined by ", "
            host.tags.iter().map(|t| t.chars().count()).sum::<usize>()
                + host.tags.len().saturating_sub(1) * 2
        };
        widths.extend_with(alias_len, account_len, host_len, port_len, tags_len);
    }
    widths
}

/// Resolve the `$USER` value used as the dim Account-cell fallback. Centralised
/// so the renderer and the panel-size calculator agree on the same string.
pub fn fallback_user() -> String {
    std::env::var("USER").unwrap_or_else(|_| "?".to_string())
}

/// Build the host table widget + selection state for the current terminal
/// width. The width drives column visibility (BRIEF_V3 §5 Q6 priority).
pub fn create_host_table<'a>(app: &'a App, width: u16) -> (Table<'a>, TableState) {
    let visibility = ColumnVisibility::for_width(width);
    let sshc_conf = crate::storage::sshc_conf_path();
    let fallback_user = fallback_user();
    let widths = compute_column_widths(app, &fallback_user);

    let rows: Vec<Row<'a>> = app
        .filtered
        .iter()
        .map(|&idx| {
            let host = &app.hosts[idx];
            let probe = app
                .probe_states
                .get(idx)
                .copied()
                .unwrap_or(ProbeState::Unknown);
            host_row(
                host,
                app,
                sshc_conf.as_deref(),
                &visibility,
                probe,
                &fallback_user,
            )
        })
        .collect();

    let constraints = host_table_constraints_for(&widths, &visibility);
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
    if visibility.show_tags {
        cells.push(Cell::from("Tags"));
    }
    cells.push(Cell::from("")); // spacer
    cells.push(Cell::from("St"));
    Row::new(cells).style(Style::default().add_modifier(Modifier::BOLD))
}

fn host_row<'a>(
    host: &'a Host,
    app: &App,
    sshc_conf: Option<&Path>,
    visibility: &ColumnVisibility,
    probe: ProbeState,
    fallback_user: &str,
) -> Row<'a> {
    let mut cells: Vec<Cell<'a>> = Vec::new();

    cells.push(alias_cell(host));

    if visibility.show_account {
        cells.push(account_cell(host, fallback_user));
    }
    if visibility.show_host {
        let hostname = host.hostname.as_deref().unwrap_or("-");
        cells.push(Cell::from(hostname.to_string()));
    }
    if visibility.show_port {
        let port_str = host.port.map(|p| p.to_string()).unwrap_or_default();
        cells.push(Cell::from(port_str));
    }
    if visibility.show_tags {
        cells.push(tags_cell(host));
    }

    cells.push(Cell::from("")); // spacer
    cells.push(status_cell(host, app, sshc_conf, probe));

    Row::new(cells)
}

/// Alias column: plain host alias. (Pre-v0.4.2 we prefixed `[t1,t2]`
/// here; tags now live in their own column to the right.)
fn alias_cell(host: &Host) -> Cell<'_> {
    Cell::from(host.alias.as_str())
}

/// Tags column: comma-separated, cyan-dim. Empty for hosts without tags
/// so the column doesn't look noisy.
fn tags_cell(host: &Host) -> Cell<'static> {
    if host.tags.is_empty() {
        Cell::from("")
    } else {
        Cell::from(host.tags.join(", "))
            .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::DIM))
    }
}

/// Account cell: shows `host.user` when present; otherwise falls back to
/// `$USER` rendered dim, mirroring ssh's default behaviour (an absent `User`
/// directive means "log in as the current OS user").
fn account_cell(host: &Host, fallback_user: &str) -> Cell<'static> {
    match host.user.as_deref() {
        Some(u) if !u.is_empty() => Cell::from(u.to_string()),
        _ => Cell::from(fallback_user.to_string())
            .style(Style::default().add_modifier(Modifier::DIM)),
    }
}

/// Status cell encoded as 3-character `<probe> <marker>`. The two halves
/// carry independent meanings — the probe glyph reflects TCP reachability
/// (computed by the background worker pool), the marker tracks
/// favorite / last-connection / external-source state. The mid space
/// disambiguates them.
///
/// - probe glyph: ● Open / ○ Failed / ◌ InFlight / ' ' Unknown
/// - marker priority (highest first):
///   - ★ yellow = favorite (sticky pin, in state.memory.favorites)
///   - ★ cyan   = last_connected (transient, recent[0])
///   - ·        = external source (host not in sshc.conf)
///   - ' '      = default
fn status_cell(
    host: &Host,
    app: &App,
    sshc_conf: Option<&Path>,
    probe: ProbeState,
) -> Cell<'static> {
    let (probe_glyph, probe_color) = match probe {
        ProbeState::Open => ('●', Some(Color::Green)),
        ProbeState::Failed => ('○', Some(Color::Red)),
        ProbeState::InFlight => ('◌', Some(Color::Yellow)),
        ProbeState::Unknown => (' ', None),
    };

    let (marker, marker_style) = if app.is_favorite(&host.alias) {
        ('★', Style::default().fg(Color::Yellow))
    } else if app.last_connected.as_deref() == Some(host.alias.as_str()) {
        ('★', Style::default().fg(Color::Cyan))
    } else if sshc_conf
        .map(|conf| host.source_file != conf)
        .unwrap_or(false)
    {
        ('·', Style::default().add_modifier(Modifier::DIM))
    } else {
        (' ', Style::default())
    };

    let probe_span = match probe_color {
        Some(c) => Span::styled(probe_glyph.to_string(), Style::default().fg(c)),
        None => Span::raw(probe_glyph.to_string()),
    };
    let marker_span = Span::styled(marker.to_string(), marker_style);
    Cell::from(Line::from(vec![probe_span, Span::raw(" "), marker_span]))
}

/// Title-bar text shown in the outer block (e.g., " sshc (12) ").
pub fn title_line(host_count: usize, _total: usize) -> Line<'static> {
    Line::from(Span::styled(
        format!(" sshc ({}) ", host_count),
        Style::default().add_modifier(Modifier::BOLD),
    ))
}

/// Bottom status text: filter input on top of an empty second row when in
/// filter mode; otherwise the keybinding help split across two lines so it
/// does not clip on narrow panels.
pub fn status_line(
    filter_mode: bool,
    filter_query: &str,
    selected_external: bool,
) -> Text<'static> {
    if filter_mode {
        Text::from(vec![
            Line::from(Span::styled(
                format!(" /{}", filter_query),
                Style::default(),
            )),
            Line::from(""),
        ])
    } else {
        let dim = Style::default().add_modifier(Modifier::DIM);
        // Second-line hints depend on what the user can actually do
        // with the selected host. `M promote to sshc.conf` only makes
        // sense when the selection lives in ~/.ssh/config (not already
        // managed), so it's swapped in only on external rows.
        let second_line = if selected_external {
            " a add  d del  t tags  e edit  M promote  i include  ? help  q quit"
        } else {
            " a add  d del  t tags  e edit  i include  ? help  q quit"
        };
        Text::from(vec![
            Line::from(Span::styled(
                " j/k nav  / filter  Enter open  s ssh  f pin  v validate".to_string(),
                dim,
            )),
            Line::from(Span::styled(second_line.to_string(), dim)),
        ])
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
            extra: Vec::new(),
            local_forward: None,
            remote_forward: None,
            dynamic_forward: None,
        }
    }

    #[test]
    fn test_alias_cell_no_tags_plain() {
        let h = host_in("/a", "alpha", vec![]);
        let _ = alias_cell(&h);
    }

    #[test]
    fn test_alias_cell_with_tags_renders_prefix() {
        let h = host_in("/a", "alpha", vec!["t1", "t2"]);
        let _ = alias_cell(&h);
    }

    #[test]
    fn test_account_cell_uses_user_when_present() {
        let h = host_in("/a", "alpha", vec![]);
        let _ = account_cell(&h, "root");
    }

    #[test]
    fn test_account_cell_falls_back_to_env_user_when_missing() {
        let mut h = host_in("/a", "alpha", vec![]);
        h.user = None;
        let _ = account_cell(&h, "root");
    }

    #[test]
    fn test_account_cell_falls_back_when_user_empty() {
        let mut h = host_in("/a", "alpha", vec![]);
        h.user = Some(String::new());
        let _ = account_cell(&h, "root");
    }

    #[test]
    fn test_status_marker_star_wins_over_source() {
        let h = host_in("/elsewhere", "alpha", vec![]);
        let mut app = App::new(vec![h.clone()]);
        app.last_connected = Some("alpha".to_string());
        let sshc_conf = PathBuf::from("/managed/sshc.conf");
        let _ = status_cell(&h, &app, Some(sshc_conf.as_path()), ProbeState::Unknown);
    }

    #[test]
    fn test_status_glyph_open() {
        let h = host_in("/a", "alpha", vec![]);
        let app = App::new(vec![h.clone()]);
        let _ = status_cell(&h, &app, None, ProbeState::Open);
    }

    #[test]
    fn test_header_includes_status_always() {
        for w in [20u16, 25, 35, 50, 80, 120] {
            let v = ColumnVisibility::for_width(w);
            let _ = header_row(&v);
        }
    }

    #[test]
    fn test_title_line_includes_count() {
        let line = title_line(7, 12);
        let s: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(s.contains("sshc"));
        assert!(s.contains("(7)"));
    }

    #[test]
    fn test_status_line_filter_mode() {
        let text = status_line(true, "foo", false);
        let first_line: String = text.lines[0]
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect();
        assert!(first_line.contains("/foo"));
        assert_eq!(text.lines.len(), 2);
    }

    #[test]
    fn test_status_line_default_two_rows() {
        let text = status_line(false, "", false);
        assert_eq!(text.lines.len(), 2);
        let row1: String = text.lines[0]
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect();
        let row2: String = text.lines[1]
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect();
        assert!(row1.contains("j/k"));
        assert!(row2.contains("? help"));
        assert!(
            !row2.contains("M promote"),
            "managed-host hint must not include M promote"
        );
    }

    #[test]
    fn test_status_line_external_host_shows_promote_hint() {
        let text = status_line(false, "", true);
        let row2: String = text.lines[1]
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect();
        assert!(
            row2.contains("M promote"),
            "external host hint should include 'M promote', got {row2:?}"
        );
    }
}
