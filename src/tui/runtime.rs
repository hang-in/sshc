//! Main-loop orchestration helpers — extracted from `main.rs` so the binary
//! entry point can stay focused on bootstrap. Holds the event loop and the
//! Connect / EditConfig action handlers.

use std::io;
use std::path::Path;
use std::time::Duration;

use crossterm::event::{self, Event, KeyEventKind};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::app::App;
use crate::config::parser::parse_config;
use crate::error::AppError;
use crate::exec::editor::build_editor_command;
use crate::exec::ssh::ssh_run;
use crate::tui::TerminalGuard;
use crate::ui;
use crate::ui::status_bar::StatusMessage;

/// Run the TUI event loop until the user signals an action.
pub fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<(), AppError> {
    loop {
        terminal.draw(|f| ui::render(f, app))?;
        if event::poll(Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    app.handle_key(key);
                }
            }
        }
        if app.should_quit || app.should_connect || app.should_edit {
            return Ok(());
        }
    }
}

/// Suspend the TUI, spawn ssh, resume the TUI, then update app state.
/// `last_connected` is recorded BEFORE spawn so that `r` reconnect works
/// even after a failed connection attempt.
pub fn handle_connect(
    guard: &mut TerminalGuard,
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    alias: &str,
) -> Result<(), AppError> {
    app.last_connected = Some(alias.to_string());
    guard.suspend()?;
    let result = ssh_run(alias, "ssh");
    guard.resume()?;
    terminal.clear()?;
    match result {
        Ok(r) => app.on_ssh_finished(alias, r),
        Err(e) => app.status_message = Some(StatusMessage::new(format!("{}", e))),
    }
    Ok(())
}

/// Suspend the TUI, run the user's `$EDITOR` on the selected host's
/// source file, re-parse the config, then resume the TUI.
pub fn handle_edit(
    guard: &mut TerminalGuard,
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    config_path: &Path,
) -> Result<(), AppError> {
    let Some(host) = app.selected_host().cloned() else {
        return Ok(());
    };
    guard.suspend()?;
    let status = build_editor_command(&host.source_file, host.line_start).status();
    guard.resume()?;
    terminal.clear()?;
    match &status {
        Ok(s) if !s.success() => log::warn!("Editor exited with non-zero status: {}", s),
        Ok(_) => {}
        Err(e) => log::error!("Editor failed to launch: {}", e),
    }
    app.replace_hosts(parse_config(config_path));
    Ok(())
}
