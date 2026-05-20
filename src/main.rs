use std::process::ExitCode;

use sshs::error::AppError;
use sshs::run;
use sshs::tui::{install_panic_hook, ScreenMode};

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

fn main() -> ExitCode {
    env_logger::init();
    install_panic_hook();

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
