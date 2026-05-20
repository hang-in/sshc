//! v0.4 inline-viewport behaviour prototype.
//!
//! Verifies that ratatui 0.29's `Viewport::Inline(N)` plays nicely with:
//!   - normal terminal mode (no alternate screen),
//!   - raw mode toggle for key input,
//!   - the suspend → "ssh"-simulation → exit round-trip,
//!   - panic hook restoration when `SSHS_PROTOTYPE_PANIC=1` is set.
//!
//! Run:
//!     cargo run --release --example inline_prototype
//!
//! Force a panic in the middle of the UI:
//!     SSHS_PROTOTYPE_PANIC=1 cargo run --release --example inline_prototype
//!
//! No real ssh is invoked. The Enter path prints "Connecting to <alias>..."
//! to stdout, sleeps 1s, and exits. Inline viewport content is intentionally
//! left in the shell scrollback so you can see what residue looks like.

use std::io::{self, Stdout};
use std::sync::OnceLock;
use std::thread;
use std::time::Duration;

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Row, Table, TableState},
    Terminal, TerminalOptions, Viewport,
};

static MODE: OnceLock<&'static str> = OnceLock::new();

/// Restore terminal state: cursor visible, raw mode off. Idempotent — safe
/// to call from Drop and from the panic hook even if neither was entered.
fn restore_terminal() {
    let _ = execute!(io::stdout(), cursor::Show);
    let _ = disable_raw_mode();
}

fn install_panic_hook() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal();
        default(info);
    }));
}

fn fake_hosts() -> Vec<(&'static str, &'static str, &'static str, u16)> {
    vec![
        ("web-1", "root", "web1.example.com", 22),
        ("db-1", "deploy", "db1.internal", 5432),
        ("staging", "deploy", "stg.example.com", 22),
        ("local", "d9ng", "127.0.0.1", 2222),
        ("legacy", "root", "10.0.0.99", 22),
        ("monitor", "ops", "mon.example.com", 22),
        ("bastion", "root", "bastion.example.com", 22),
    ]
}

fn filter_hosts<'a>(hosts: &'a [(&'a str, &'a str, &'a str, u16)], query: &str) -> Vec<usize> {
    let q = query.to_lowercase();
    hosts
        .iter()
        .enumerate()
        .filter(|(_, (alias, user, host, _))| {
            q.is_empty()
                || alias.to_lowercase().contains(&q)
                || user.to_lowercase().contains(&q)
                || host.to_lowercase().contains(&q)
        })
        .map(|(i, _)| i)
        .collect()
}

fn draw(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    hosts: &[(&str, &str, &str, u16)],
    filtered: &[usize],
    selected: usize,
    query: &str,
) -> io::Result<()> {
    terminal.draw(|f| {
        let area = f.area();
        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" sshs prototype ({}) ", filtered.len()));
        let inner = block.inner(area);
        f.render_widget(block, area);

        let rows_layout =
            Layout::vertical([Constraint::Min(1), Constraint::Length(2)]).split(inner);
        let table_area = rows_layout[0];
        let status_area = rows_layout[1];

        let table_rows: Vec<Row> = filtered
            .iter()
            .map(|&i| {
                let (alias, user, host, port) = hosts[i];
                Row::new(vec![
                    alias.to_string(),
                    user.to_string(),
                    host.to_string(),
                    port.to_string(),
                ])
            })
            .collect();

        let header = Row::new(vec!["Alias", "Account", "Host", "Port"])
            .style(Style::default().add_modifier(Modifier::BOLD));
        let table = Table::new(
            table_rows,
            [
                Constraint::Length(12),
                Constraint::Length(10),
                Constraint::Min(15),
                Constraint::Length(6),
            ],
        )
        .header(header)
        .row_highlight_style(
            Style::default()
                .add_modifier(Modifier::REVERSED)
                .add_modifier(Modifier::BOLD),
        );

        let mut state = TableState::default();
        if !filtered.is_empty() {
            state.select(Some(selected));
        }
        f.render_stateful_widget(table, table_area, &mut state);

        let dim = Style::default().add_modifier(Modifier::DIM);
        let status = if query.is_empty() {
            vec![
                Line::from(Span::styled(
                    " type to filter  ↑/↓ or j/k nav  Enter ssh  Esc quit",
                    dim,
                )),
                Line::from(Span::styled(
                    " (this is a prototype — no real ssh is invoked)",
                    dim.fg(Color::Yellow),
                )),
            ]
        } else {
            vec![
                Line::from(Span::styled(
                    format!(" /{query}"),
                    Style::default().fg(Color::Cyan),
                )),
                Line::from(Span::styled(" Backspace edit  Esc clear  Enter ssh", dim)),
            ]
        };
        let status_widget = Paragraph::new(ratatui::text::Text::from(status));
        f.render_widget(status_widget, status_area);
    })?;
    Ok(())
}

fn run_inline() -> io::Result<Option<String>> {
    MODE.set("inline").ok();
    let hosts = fake_hosts();
    let mut query = String::new();
    let mut filtered = filter_hosts(&hosts, &query);
    let mut selected = 0usize;

    enable_raw_mode()?;
    execute!(io::stdout(), cursor::Hide)?;

    let options = TerminalOptions {
        viewport: Viewport::Inline(15),
    };
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::with_options(backend, options)?;

    let mut chosen: Option<String> = None;

    'main: loop {
        // Optional panic injection for restoration verification.
        if std::env::var("SSHS_PROTOTYPE_PANIC").ok().as_deref() == Some("1") && !query.is_empty() {
            panic!("requested panic via SSHS_PROTOTYPE_PANIC=1");
        }

        draw(&mut terminal, &hosts, &filtered, selected, &query)?;

        if event::poll(Duration::from_millis(250))? {
            if let Event::Key(KeyEvent {
                code,
                modifiers,
                kind: KeyEventKind::Press,
                ..
            }) = event::read()?
            {
                match (code, modifiers) {
                    (KeyCode::Esc, _) => {
                        if query.is_empty() {
                            break 'main;
                        } else {
                            query.clear();
                            filtered = filter_hosts(&hosts, &query);
                            selected = 0;
                        }
                    }
                    (KeyCode::Char('c'), KeyModifiers::CONTROL) => break 'main,
                    (KeyCode::Up, _) | (KeyCode::Char('k'), KeyModifiers::NONE) => {
                        if !filtered.is_empty() {
                            selected = if selected == 0 {
                                filtered.len() - 1
                            } else {
                                selected - 1
                            };
                        }
                    }
                    (KeyCode::Down, _) | (KeyCode::Char('j'), KeyModifiers::NONE) => {
                        if !filtered.is_empty() {
                            selected = (selected + 1) % filtered.len();
                        }
                    }
                    (KeyCode::Enter, _) => {
                        if let Some(&i) = filtered.get(selected) {
                            chosen = Some(hosts[i].0.to_string());
                            break 'main;
                        }
                    }
                    (KeyCode::Backspace, _) => {
                        if query.pop().is_some() {
                            filtered = filter_hosts(&hosts, &query);
                            selected = 0;
                        }
                    }
                    (KeyCode::Char(c), m) if !m.contains(KeyModifiers::CONTROL) => {
                        query.push(c);
                        filtered = filter_hosts(&hosts, &query);
                        selected = 0;
                    }
                    _ => {}
                }
            }
        }
    }

    // Cleanly tear down before the simulated ssh call so the shell prompt
    // can take back control.
    terminal.clear()?;
    drop(terminal);
    restore_terminal();

    Ok(chosen)
}

fn main() -> io::Result<()> {
    install_panic_hook();

    let chosen = match run_inline() {
        Ok(c) => c,
        Err(e) => {
            restore_terminal();
            eprintln!("inline run failed: {e}");
            std::process::exit(1);
        }
    };

    match chosen {
        Some(alias) => {
            // Simulate the ssh round-trip without actually invoking ssh.
            // The prototype proves that after we tear down the inline UI
            // and return raw mode + cursor, normal stdout printing works.
            println!("Connecting to {alias} (simulated; sleeping 1s)...");
            thread::sleep(Duration::from_secs(1));
            println!("(would now exec ssh {alias}; prototype exits here)");
        }
        None => {
            println!("(cancelled)");
        }
    }

    Ok(())
}
