//! CLI argument parsing and top-level dispatch.
//!
//! Splits the previous `main.rs` body out of the binary so `main.rs` stays
//! within the R-G4 thin-bootstrap budget. Handles `-h`/`-V`, the v0.5
//! positional `<ALIAS>` direct-connect shortcut, and the inline/manage
//! mode split.

use std::process::ExitCode;

use crate::doctor;
use crate::error::AppError;
use crate::run;
use crate::tui::ScreenMode;

const VIEWPORT_MAX: u16 = 15;
const VIEWPORT_MIN: u16 = 8;
const TERMINAL_TOO_SMALL: u16 = 12;
const RESERVED_LINES: u16 = 5;

fn terminal_height() -> u16 {
    crossterm::terminal::size().map(|(_, h)| h).unwrap_or(24)
}

fn parse_mode(args: &[String]) -> ScreenMode {
    if args.iter().any(|a| a == "-m" || a == "--manage") {
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
             sshc [OPTIONS] [ALIAS]\n\n\
         ARGS:\n    \
             ALIAS           Connect to <ALIAS> immediately; skip the TUI.\n\n\
         OPTIONS:\n    \
             -m, --manage    Open the full management TUI (alternate screen).\n                    \
                             Without this flag sshc opens an inline fzf-style picker.\n    \
             -h, --help      Print this help and exit.\n    \
             -V, --version   Print version and exit.\n    \
             --doctor        Run an environment report (~/.ssh, sshc.conf,\n                    \
                             Include line, ssh binary, SSH_AUTH_SOCK) and exit.\n\n\
         Inline keys: ↑/↓ or j/k navigate, / search, Enter ssh,\n             \
                      q or Esc quit, Ctrl+C quit.\n             \
                      In search: type/Backspace filters, Esc exits search.\n\
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

fn into_exit_code(result: Result<ExitCode, AppError>) -> ExitCode {
    match result {
        Ok(code) => code,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

/// CLI entry point. Handles `-h`/`-V` first, then the positional
/// `<ALIAS>` direct-connect shortcut, then falls back to the inline /
/// manage TUI based on terminal size and `-m`.
pub fn run(args: Vec<String>) -> ExitCode {
    if args.iter().any(|a| a == "-h" || a == "--help") {
        print_help();
        return ExitCode::SUCCESS;
    }
    if args.iter().any(|a| a == "-V" || a == "--version") {
        print_version();
        return ExitCode::SUCCESS;
    }
    if args.iter().any(|a| a == "--doctor") {
        return doctor::run();
    }
    if let Some(alias) = args.iter().skip(1).find(|a| !a.starts_with('-')) {
        return into_exit_code(run::direct(alias));
    }
    let result = match parse_mode(&args) {
        ScreenMode::Inline(h) => run::inline(h),
        ScreenMode::Alternate => run::manage(),
    };
    into_exit_code(result)
}
