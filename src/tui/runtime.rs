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
use crate::probe::ProbePool;
use crate::tui::TerminalGuard;
use crate::ui;
use crate::ui::status_bar::StatusMessage;

/// Run the TUI event loop until the user signals an action.
///
/// Each iteration drains any available ProbeUpdates from `probe_pool` into
/// `app` BEFORE the draw, so probe-glyph changes appear with minimal latency
/// (≤ one poll interval). Modal-mode short-circuit lives inside
/// `App::handle_key` — runtime hands every keystroke to App, which routes
/// it to the modal handler when `app.mode` is not `List`.
pub fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    probe_pool: &ProbePool,
) -> Result<(), AppError> {
    loop {
        let updates = probe_pool.poll_updates();
        if !updates.is_empty() {
            app.apply_probe_updates(updates);
        }
        terminal.draw(|f| ui::render(f, app))?;
        if event::poll(Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
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

/// Apply `AppAction::InjectInclude`: add the Include line to the user's
/// `~/.ssh/config`, persist the updated state, and surface the outcome on
/// the status bar.
pub fn handle_inject_include(app: &mut App) {
    let (Some(main_cfg), Some(sshc_conf)) = (
        crate::storage::main_ssh_config_path(),
        crate::storage::sshc_conf_path(),
    ) else {
        app.status_message = Some(StatusMessage::new("could not resolve ssh config paths"));
        return;
    };
    match crate::storage::inject_include(&main_cfg, &sshc_conf) {
        Ok(added) => {
            app.state.setup.include_check_done = true;
            app.state.setup.declined_include_injection = false;
            let _ = crate::state::save(&app.state);
            let msg = if added {
                "Include line added to ~/.ssh/config — writes enabled"
            } else {
                "Include line already present — writes already enabled"
            };
            app.status_message = Some(StatusMessage::new(msg));
        }
        Err(e) => {
            app.status_message = Some(StatusMessage::new(format!("include injection failed: {e}")));
        }
    }
}

/// Suspend the TUI, spawn ssh, resume the TUI, then update app state.
/// `last_connected` is recorded BEFORE spawn so that `r` reconnect works
/// even after a failed connection attempt. Also mirrors the alias into
/// `app.state.memory` so the next startup remembers it.
pub fn handle_connect(
    guard: &mut TerminalGuard,
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    alias: &str,
) -> Result<(), AppError> {
    app.last_connected = Some(alias.to_string());
    app.state.memory.last_connected_alias = Some(alias.to_string());
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
