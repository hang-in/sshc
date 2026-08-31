//! Inline-mode event loop and ssh round-trip handler.
//!
//! Inline mode renders into a `Viewport::Inline(N)` ratatui terminal
//! created by `main.rs`. Unlike the manage-mode runtime, this layer
//! never re-enters the UI after a successful ssh round-trip — process
//! exits immediately, returning the `SshResult` so the caller can pick
//! an exit code.
//!
//! Layout: no border, left-aligned, width matches the data (clamped to
//! the viewport width). Leaves the rest of the shell context visible to
//! the right of the table.

use std::io::Stdout;
use std::time::Duration;

use crossterm::event::{poll, read, Event, KeyEventKind};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Paragraph, Row, Table, TableState};
use ratatui::{Frame, Terminal};

use crate::error::AppError;
use crate::exec::ssh::{ssh_run, SshResult};
use crate::inline_app::InlineApp;
use crate::tui::TerminalGuard;

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

pub fn handle_connect_inline(
    guard: &mut TerminalGuard,
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut InlineApp,
    alias: &str,
) -> Result<SshResult, AppError> {
    terminal.clear()?;
    terminal.draw(|_| {})?;

    app.last_connected = Some(alias.to_string());
    guard.suspend()?;

    let (program, args) = crate::exec::ssh::resolve_ssh_command();
    let result = ssh_run(alias, &program, &args)?;
    Ok(result)
}

/// Per-column max widths used to size the table to the data.
struct InlineWidths {
    alias: u16,
    account: u16,
    host: u16,
    port: u16,
}

fn compute_widths(app: &InlineApp, fallback_user: &str) -> InlineWidths {
    // Header text gives the floor.
    let mut w = InlineWidths {
        alias: "Alias".len() as u16,
        account: "Account".len() as u16,
        host: "Host".len() as u16,
        port: "Port".len() as u16,
    };
    for &idx in &app.filtered {
        let host = &app.hosts[idx];
        w.alias = w.alias.max(host.alias.chars().count() as u16);
        let account_len = match host.user.as_deref() {
            Some(u) if !u.is_empty() => u.chars().count() as u16,
            _ => fallback_user.chars().count() as u16,
        };
        w.account = w.account.max(account_len);
        let host_len = host
            .hostname
            .as_deref()
            .map(|s| s.chars().count() as u16)
            .unwrap_or(1);
        w.host = w.host.max(host_len);
        let port_len = host.port.map(|p| p.to_string().len() as u16).unwrap_or(0);
        w.port = w.port.max(port_len);
    }
    w
}

fn render_inline(f: &mut Frame, app: &InlineApp) {
    let viewport = f.area();

    let fallback_user = std::env::var("USER").unwrap_or_else(|_| "?".to_string());
    let w = compute_widths(app, &fallback_user);

    let pad: u16 = 2;
    let status_w: u16 = 2;
    let column_spacing: u16 = 1; // ratatui Table default
                                 // 5 columns: Alias, Account, Host, Port, Status. 4 gaps between them.
    let row_width = (w.alias + pad)
        + (w.account + pad)
        + (w.host + pad)
        + (w.port + pad)
        + status_w
        + column_spacing * 4;
    let render_width = row_width.min(viewport.width);
    let area = Rect::new(viewport.x, viewport.y, render_width, viewport.height);

    // v0.6: 2-line status block under the host table:
    //   - line 1: host summary (nav mode) or `▸ <query>` (filter mode)
    //   - line 2: mode-appropriate key hints
    let chunks = Layout::vertical([
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(area);
    let table_area = chunks[0];
    let line1_area = chunks[1];
    let line2_area = chunks[2];

    let constraints = [
        Constraint::Length(w.alias + pad),
        Constraint::Length(w.account + pad),
        Constraint::Length(w.host + pad),
        Constraint::Length(w.port + pad),
        Constraint::Length(status_w),
    ];

    let header = Row::new(vec!["Alias", "Account", "Host", "Port", "St"])
        .style(Style::default().add_modifier(Modifier::BOLD));

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
            // ★ in yellow = favorite (sticky, user-pinned).
            // ★ in cyan   = last_connected (transient, recency-derived).
            // favorite wins when both apply.
            let is_fav = app.is_favorite(&host.alias);
            let is_last = app.last_connected.as_deref() == Some(host.alias.as_str());
            let status_cell = if is_fav {
                Cell::from(Line::from(vec![
                    Span::raw(" "),
                    Span::styled("★", Style::default().fg(Color::Yellow)),
                ]))
            } else if is_last {
                Cell::from(Line::from(vec![
                    Span::raw(" "),
                    Span::styled("★", Style::default().fg(Color::Cyan)),
                ]))
            } else {
                Cell::from("  ")
            };
            Row::new(vec![
                Cell::from(host.alias.clone()),
                Cell::from(account_text).style(account_style),
                Cell::from(hostname),
                Cell::from(port),
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

    // Line 1: filter-mode shows the active query; nav mode shows a one-
    // line user@host:port preview of the highlighted row.
    let line1 = if app.filter_mode {
        Paragraph::new(format!(" ▸ {}", app.query)).style(Style::default().fg(Color::Cyan))
    } else if let Some(host) = app
        .filtered
        .get(app.selected)
        .and_then(|&i| app.hosts.get(i))
    {
        let user = host
            .user
            .as_deref()
            .filter(|u| !u.is_empty())
            .unwrap_or(fallback_user.as_str());
        let hostname = host.hostname.as_deref().unwrap_or("-");
        let port = host.port.map(|p| format!(":{p}")).unwrap_or_default();
        Paragraph::new(format!(" → {user}@{hostname}{port}")).style(dim)
    } else {
        Paragraph::new("").style(dim)
    };

    // Line 2: mode-appropriate key hints.
    let line2_text = if app.filter_mode {
        " ↑/↓ nav  Esc cancel  Enter ssh"
    } else {
        " ↑/↓ or j/k nav  / search  Enter ssh  q quit"
    };
    let line2 = Paragraph::new(line2_text).style(dim);

    f.render_widget(line1, line1_area);
    f.render_widget(line2, line2_area);
}
