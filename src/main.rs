use std::process::ExitCode;

use sshc::error::AppError;
use sshc::run;
use sshc::tui::{install_panic_hook, ScreenMode};

const VIEWPORT_MAX: u16 = 15;
const VIEWPORT_MIN: u16 = 8;
const TERMINAL_TOO_SMALL: u16 = 12;
const RESERVED_LINES: u16 = 5;

fn terminal_height() -> u16 {
    crossterm::terminal::size().map(|(_, h)| h).unwrap_or(24)
}

fn parse_mode() -> ScreenMode {
    let manage = std::env::args().any(|a| a == "-m" || a == "--manage");
    if manage {
        return ScreenMode::Alternate;
    }
    let h = terminal_height();
    if h < TERMINAL_TOO_SMALL {
        eprintln!("terminal too small for inline mode; falling back to --manage");
        return ScreenMode::Alternate;
    }
    let viewport = h
        .saturating_sub(RESERVED_LINES)
        .clamp(VIEWPORT_MIN, VIEWPORT_MAX);
    ScreenMode::Inline(viewport)
}

fn print_help() {
    println!(
        "sshc {} — terminal UI for managing SSH hosts\n\n\
         USAGE:\n    \
             sshc [OPTIONS]\n\n\
         OPTIONS:\n    \
             -m, --manage    Open the full management TUI (alternate screen).\n                    \
                             Without this flag sshc opens an inline fzf-style picker.\n    \
             -h, --help      Print this help and exit.\n    \
             -V, --version   Print version and exit.\n\n\
         Inline keys: type to filter, ↑/↓ or j/k navigate, Enter ssh,\n             \
                      r reconnect, Esc clear/quit, Ctrl+C quit.\n\
         Manage keys: see `?` inside the TUI.\n\n\
         Files:\n    \
             ~/.ssh/config.d/sshc.conf   hosts added via manage mode\n    \
             ~/.config/sshc/state.toml   last-connected + setup state\n\n\
         Source: https://github.com/hang-in/sshc",
        env!("CARGO_PKG_VERSION")
    );
}

fn print_version() {
    println!("sshc {}", env!("CARGO_PKG_VERSION"));
}

fn main() -> ExitCode {
    env_logger::init();
    install_panic_hook();

    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        print_help();
        return ExitCode::SUCCESS;
    }
    if args.iter().any(|a| a == "-V" || a == "--version") {
        print_version();
        return ExitCode::SUCCESS;
    }

    let result: Result<ExitCode, AppError> = match parse_mode() {
        ScreenMode::Inline(h) => run::inline(h),
        ScreenMode::Alternate => run::manage(),
    };
    match result {
        Ok(code) => code,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
