//! Inline-mode event loop and ssh round-trip handler.
//!
//! Inline mode renders into a `Viewport::Inline(N)` ratatui terminal
//! created by `main.rs`. Unlike the manage-mode runtime, this layer
//! never re-enters the UI after a successful ssh round-trip — process
//! exits immediately, returning the `SshResult` so the caller can pick
//! an exit code.

use std::io::Stdout;
use std::time::Duration;

use crossterm::event::{poll, read, Event, KeyEventKind};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState};
use ratatui::{Frame, Terminal};

use crate::error::AppError;
use crate::exec::ssh::{ssh_run, SshResult};
use crate::inline_app::InlineApp;
use crate::tui::TerminalGuard;

/// Drive the inline-mode TUI. Returns when `app` has set a pending action.
pub fn run_event_loop_inline(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut InlineApp,
) -> Result<(), AppError> {
    loop {
        terminal.draw(|f| render_inline(f, app))?;

        if poll(Duration::from_millis(250))? {
            if let Event::Key(key) = read()? {
                if key.kind == KeyEventKind::Press {
                    app.handle_key(key);
                }
            }
        }

        if app.has_pending_action() {
            return Ok(());
        }
    }
}

/// Suspend the inline viewport, spawn ssh, and return the result.
/// Inline mode does NOT resume the UI afterwards — the caller exits the
/// process based on the returned `SshResult`.
pub fn handle_connect_inline(
    guard: &mut TerminalGuard,
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut InlineApp,
    alias: &str,
) -> Result<SshResult, AppError> {
    // Clear the inline viewport so the shell context isn't polluted with
    // a frozen frame. Empty draw collapses the area before suspend.
    terminal.clear()?;
    terminal.draw(|_| {})?;

    app.last_connected = Some(alias.to_string());
    guard.suspend()?;

    let result = ssh_run(alias, "ssh")?;
    Ok(result)
}

fn render_inline(f: &mut Frame, app: &InlineApp) {
    let size = f.area();

    let outer = Block::default()
        .borders(Borders::ALL)
        .title(format!(" sshc ({}) ", app.host_count()));
    let inner = outer.inner(size);
    f.render_widget(outer, size);

    let chunks = Layout::vertical([Constraint::Min(1), Constraint::Length(2)]).split(inner);
    let table_area = chunks[0];
    let status_area = chunks[1];

    // Columns: Alias / Account / Host / Port / spacer / Status (★ only).
    let constraints = [
        Constraint::Min(10),
        Constraint::Length(10),
        Constraint::Min(12),
        Constraint::Length(5),
        Constraint::Min(2),
        Constraint::Length(2),
    ];

    let header = Row::new(vec!["Alias", "Account", "Host", "Port", "", "St"])
        .style(Style::default().add_modifier(Modifier::BOLD));

    let fallback_user = std::env::var("USER").unwrap_or_else(|_| "?".to_string());
    let dim = Style::default().add_modifier(Modifier::DIM);

    let rows: Vec<Row> = app
        .filtered
        .iter()
        .map(|&idx| {
            let host = &app.hosts[idx];

            let (account_text, account_style) = match host.user.as_deref() {
                Some(u) if !u.is_empty() => (u.to_string(), Style::default()),
                _ => (fallback_user.clone(), dim),
            };
            let hostname = host.hostname.as_deref().unwrap_or("-").to_string();
            let port = host.port.map(|p| p.to_string()).unwrap_or_default();

            let is_last = app.last_connected.as_deref() == Some(host.alias.as_str());
            let status_cell = if is_last {
                Cell::from(Line::from(vec![
                    Span::raw(" "),
                    Span::styled("★", Style::default().fg(Color::Yellow)),
                ]))
            } else {
                Cell::from("  ")
            };

            Row::new(vec![
                Cell::from(host.alias.clone()),
                Cell::from(account_text).style(account_style),
                Cell::from(hostname),
                Cell::from(port),
                Cell::from(""),
                status_cell,
            ])
        })
        .collect();

    let mut state = TableState::default();
    if !app.filtered.is_empty() {
        state.select(Some(app.selected));
    }

    let table = Table::new(rows, constraints)
        .header(header)
        .row_highlight_style(
            Style::default()
                .add_modifier(Modifier::REVERSED)
                .add_modifier(Modifier::BOLD),
        );
    f.render_stateful_widget(table, table_area, &mut state);

    let status_chunks =
        Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).split(status_area);

    let line1 = if !app.query.is_empty() {
        Paragraph::new(format!(" /{}", app.query)).style(Style::default().fg(Color::Cyan))
    } else {
        Paragraph::new(" type to filter  ↑/↓ or j/k nav").style(dim)
    };
    let line2 = Paragraph::new(" Enter ssh  r reconnect  Esc cancel").style(dim);

    f.render_widget(line1, status_chunks[0]);
    f.render_widget(line2, status_chunks[1]);
}
