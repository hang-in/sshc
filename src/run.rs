//! Mode dispatch helpers — keeps `main.rs` a thin shell under the R-G4
//! 80-line cap. `inline` drives the v0.4 fzf-style flow; `manage` is the
//! v0.3 alternate-screen TUI carved out of the previous main loop.

use std::io;
use std::path::PathBuf;
use std::process::ExitCode;

use ratatui::backend::CrosstermBackend;
use ratatui::{Terminal, TerminalOptions, Viewport};

use crate::app::{App, AppAction, AppMode};
use crate::config::parser::parse_config;
use crate::error::AppError;
use crate::exec::ssh::SshResult;
use crate::inline_app::{InlineAction, InlineApp};
use crate::probe::ProbePool;
use crate::setup::{run_first_run_checks, SetupOutcome};
use crate::state;
use crate::tui::{inline_runtime, runtime, ScreenMode, TerminalGuard};
use crate::ui::modal::{ModalAction, ModalKind};
use crate::ui::status_bar::StatusMessage;

fn config_path() -> PathBuf {
    dirs::home_dir()
        .map(|h| h.join(".ssh/config"))
        .unwrap_or_else(|| PathBuf::from(".ssh/config"))
}

fn exit_code_from(result: SshResult) -> ExitCode {
    match result {
        SshResult::Success | SshResult::Interrupted => ExitCode::SUCCESS,
        SshResult::ConnectFailed(c) | SshResult::Failed(c) => ExitCode::from((c & 0xFF) as u8),
        SshResult::Crashed(_) | SshResult::UnknownTermination => ExitCode::FAILURE,
    }
}

/// v0.4 inline mode: lean fzf-style host browser. Process exits after the
/// first ssh round-trip (or immediately on Quit) — the UI is never resumed.
pub fn inline(viewport_height: u16) -> Result<ExitCode, AppError> {
    let mut app_state = state::load().unwrap_or_default();
    let hosts = parse_config(&config_path());
    let mut app = InlineApp::new_with_state(hosts, &app_state);

    let mut guard = TerminalGuard::acquire(ScreenMode::Inline(viewport_height))?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::with_options(
        backend,
        TerminalOptions {
            viewport: Viewport::Inline(viewport_height),
        },
    )?;

    loop {
        inline_runtime::run_event_loop_inline(&mut terminal, &mut app)?;
        match app.take_action() {
            None | Some(InlineAction::Quit) => return Ok(ExitCode::SUCCESS),
            Some(InlineAction::Connect(alias)) => {
                let result = inline_runtime::handle_connect_inline(
                    &mut guard,
                    &mut terminal,
                    &mut app,
                    &alias,
                )?;
                app_state.memory.last_connected_alias = Some(alias);
                let _ = state::save(&app_state);
                return Ok(exit_code_from(result));
            }
            Some(InlineAction::Reconnect) => {
                let Some(alias) = app.last_connected.clone() else {
                    continue;
                };
                let result = inline_runtime::handle_connect_inline(
                    &mut guard,
                    &mut terminal,
                    &mut app,
                    &alias,
                )?;
                app_state.memory.last_connected_alias = Some(alias);
                let _ = state::save(&app_state);
                return Ok(exit_code_from(result));
            }
        }
    }
}

/// v0.3 manage mode: full alternate-screen TUI with CRUD, tags, probes,
/// first-run setup, and modal subsystem. Returns ExitCode::SUCCESS on
/// graceful quit.
pub fn manage() -> Result<ExitCode, AppError> {
    let config_path = config_path();
    let mut app_state = state::load().unwrap_or_default();
    let outcome = run_first_run_checks(&mut app_state).unwrap_or(SetupOutcome::ReadOnly);
    let hosts = parse_config(&config_path);
    let mut app = App::new_with_state(hosts, app_state);

    if matches!(outcome, SetupOutcome::AwaitingIncludeChoice) {
        let prompt = "sshs needs to add an Include line to ~/.ssh/config so it can\n\
                      manage hosts via ~/.ssh/config.d/sshs.conf. Allow?"
            .to_string();
        app.mode = AppMode::Modal(ModalKind::Confirmation {
            prompt,
            on_yes: ModalAction::Custom("inject_include".into()),
            on_no: ModalAction::Custom("decline_include".into()),
        });
    }

    let probe_pool = ProbePool::start(&app.hosts);

    let mut guard = TerminalGuard::acquire(ScreenMode::Alternate)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;

    loop {
        runtime::run_event_loop(&mut terminal, &mut app, &probe_pool)?;
        match app.take_action() {
            None | Some(AppAction::Quit) => break,
            Some(AppAction::EditConfig) => {
                runtime::handle_edit(&mut guard, &mut terminal, &mut app, &config_path)?;
                probe_pool.refresh(&app.hosts);
            }
            Some(AppAction::Connect(alias)) => {
                runtime::handle_connect(&mut guard, &mut terminal, &mut app, &alias)?;
                let _ = state::save(&app.state);
            }
            Some(AppAction::SaveState) => {
                if let Err(e) = state::save(&app.state) {
                    app.status_message =
                        Some(StatusMessage::new(format!("save state failed: {e}")));
                }
                probe_pool.refresh(&app.hosts);
            }
            Some(AppAction::InjectInclude) => runtime::handle_inject_include(&mut app),
            Some(AppAction::DeclineInclude) => {
                let _ = state::save(&app.state);
            }
        }
    }
    Ok(ExitCode::SUCCESS)
}
