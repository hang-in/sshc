use std::io;

use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use sshs::app::{App, AppAction, AppMode};
use sshs::config::parser::parse_config;
use sshs::error::AppError;
use sshs::probe::ProbePool;
use sshs::setup::{run_first_run_checks, SetupOutcome};
use sshs::state;
use sshs::tui::{install_panic_hook, runtime, TerminalGuard};
use sshs::ui::modal::{ModalAction, ModalKind};
use sshs::ui::status_bar::StatusMessage;

fn main() -> Result<(), AppError> {
    env_logger::init();
    install_panic_hook();

    // Load (or default) state.toml, then ensure the sshs.conf scaffolding
    // exists and determine include-injection status.
    let mut app_state = state::load().unwrap_or_default();
    let outcome = run_first_run_checks(&mut app_state).unwrap_or(SetupOutcome::ReadOnly);

    let config_path = dirs::home_dir()
        .map(|h| h.join(".ssh/config"))
        .unwrap_or_else(|| std::path::PathBuf::from(".ssh/config"));
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

    let mut guard = TerminalGuard::acquire()?;
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
    Ok(())
}
