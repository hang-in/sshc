use std::io;

use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use sshs::app::{App, AppAction};
use sshs::config::parser::parse_config;
use sshs::error::AppError;
use sshs::tui::{install_panic_hook, runtime, TerminalGuard};

fn main() -> Result<(), AppError> {
    env_logger::init();
    install_panic_hook();
    let mut guard = TerminalGuard::acquire()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let config_path = dirs::home_dir()
        .map(|h| h.join(".ssh/config"))
        .unwrap_or_else(|| std::path::PathBuf::from(".ssh/config"));
    let mut app = App::new(parse_config(&config_path));

    loop {
        runtime::run_event_loop(&mut terminal, &mut app)?;
        match app.take_action() {
            None | Some(AppAction::Quit) => break,
            Some(AppAction::EditConfig) => {
                runtime::handle_edit(&mut guard, &mut terminal, &mut app, &config_path)?
            }
            Some(AppAction::Connect(alias)) => {
                runtime::handle_connect(&mut guard, &mut terminal, &mut app, &alias)?
            }
            Some(AppAction::SaveState)
            | Some(AppAction::InjectInclude)
            | Some(AppAction::DeclineInclude) => {
                // R7/T11 wires these up. For R6, the state mutations have
                // already happened in-memory inside App; persistence and
                // include-injection land in the runtime layer next round.
            }
        }
    }
    Ok(())
}
