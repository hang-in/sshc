use std::io;
use std::panic;
use std::time::Duration;

use crossterm::event::{self, Event};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use sshs::app::App;
use sshs::config::parser::parse_config;
use sshs::exec::editor::build_editor_command;
use sshs::exec::ssh::ssh_connect;
use sshs::ui;

fn main() -> anyhow::Result<()> {
    // Install panic hook FIRST — ensures terminal is restored even on panic
    setup_panic_hook();

    // Parse SSH config
    let config_path = dirs::home_dir()
        .map(|h| h.join(".ssh/config"))
        .unwrap_or_else(|| std::path::PathBuf::from(".ssh/config"));

    let hosts = parse_config(&config_path);

    // Setup terminal
    let backend = setup_terminal()?;
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut app = App::new(hosts);

    // Run event loop
    let result = run_app(&mut terminal, &mut app);

    // Restore terminal BEFORE any further action
    restore_terminal(&mut terminal)?;

    // Handle result from event loop
    result?;

    // Post-TUI actions (terminal is now restored)
    if app.should_connect {
        if let Some(host) = app.selected_host() {
            ssh_connect(&host.alias)?;
        }
    } else if app.should_edit {
        if let Some(host) = app.selected_host() {
            let mut cmd = build_editor_command(&host.source_file, host.line_start);
            let _status = cmd.status();

            // After editor exits, re-parse and re-run TUI
            let new_hosts = parse_config(&config_path);
            app.refresh_hosts(new_hosts);
            app.reset_actions();

            // Re-enter TUI
            let backend = setup_terminal()?;
            let mut terminal = Terminal::new(backend)?;
            let result = run_app(&mut terminal, &mut app);
            restore_terminal(&mut terminal)?;
            result?;

            if app.should_connect {
                if let Some(host) = app.selected_host() {
                    ssh_connect(&host.alias)?;
                }
            }
        }
    }

    Ok(())
}

fn setup_terminal() -> anyhow::Result<CrosstermBackend<io::Stdout>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    Ok(CrosstermBackend::new(stdout))
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> anyhow::Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> anyhow::Result<()> {
    loop {
        terminal.draw(|f| ui::render(f, app))?;

        if event::poll(Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                // Ignore key release events (some terminals send both press and release)
                if key.kind != crossterm::event::KeyEventKind::Press {
                    continue;
                }
                app.handle_key(key);
            }
        }

        if app.should_quit || app.should_connect || app.should_edit {
            return Ok(());
        }
    }
}

/// Installs a panic hook that restores the terminal state before printing the panic info.
/// Without this, a panic would leave the terminal in raw mode with the alternate screen active.
fn setup_panic_hook() {
    let default_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        // Restore terminal state FIRST
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        let _ = execute!(io::stdout(), crossterm::cursor::Show);
        eprintln!("{}", panic_info);
        // Then run the default hook (which may also print to stderr)
        default_hook(panic_info);
    }));
}
