use std::io;
use std::panic;
use std::sync::atomic::{AtomicBool, Ordering};

use crossterm::{
    cursor, execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

use crate::error::TerminalError;

pub(crate) static TERMINAL_ACTIVE: AtomicBool = AtomicBool::new(false);
static HOOK_INSTALLED: AtomicBool = AtomicBool::new(false);

/// RAII guard owning the terminal raw mode + alternate screen.
///
/// Construction enables raw mode then enters the alternate screen.
/// `Drop` runs the leave sequence idempotently and never panics.
///
/// Only one `TerminalGuard` may exist at a time — construction while another
/// is active panics with a programmer-error message.
pub struct TerminalGuard {}

impl TerminalGuard {
    /// Acquire exclusive ownership of the terminal.
    /// CONTRACT: panics if another guard exists. Returns `Err` (with state rolled back)
    /// if either step of the enter sequence fails.
    pub fn acquire() -> Result<Self, TerminalError> {
        if TERMINAL_ACTIVE.swap(true, Ordering::SeqCst) {
            // Restore the flag so the existing guard is unaffected.
            TERMINAL_ACTIVE.store(true, Ordering::SeqCst);
            panic!("TerminalGuard::acquire called while another guard exists");
        }

        if let Err(e) = Self::setup_terminal() {
            TERMINAL_ACTIVE.store(false, Ordering::SeqCst);
            return Err(e);
        }

        Ok(TerminalGuard {})
    }

    /// Suspend the terminal so a child process can take over stdio.
    /// Pairs with `resume`. No-op if already suspended.
    pub fn suspend(&mut self) -> Result<(), TerminalError> {
        if !TERMINAL_ACTIVE.load(Ordering::SeqCst) {
            return Ok(());
        }

        // Clear the flag FIRST so a panic during leave doesn't re-trigger via the hook.
        TERMINAL_ACTIVE.store(false, Ordering::SeqCst);

        let mut stdout = io::stdout();
        let res_alt = execute!(stdout, LeaveAlternateScreen).map_err(TerminalError::LeaveAltScreen);
        let res_raw = disable_raw_mode().map_err(TerminalError::LeaveRawMode);

        match (res_alt, res_raw) {
            (Err(e), _) => Err(e),
            (_, Err(e)) => Err(e),
            (Ok(_), Ok(_)) => Ok(()),
        }
    }

    /// Re-enter raw mode + alternate screen after a suspension.
    /// No-op if already active.
    pub fn resume(&mut self) -> Result<(), TerminalError> {
        if TERMINAL_ACTIVE.load(Ordering::SeqCst) {
            return Ok(());
        }

        Self::setup_terminal()?;
        TERMINAL_ACTIVE.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn setup_terminal() -> Result<(), TerminalError> {
        enable_raw_mode().map_err(TerminalError::EnterRawMode)?;
        let mut stdout = io::stdout();
        if let Err(e) = execute!(stdout, EnterAlternateScreen) {
            let _ = disable_raw_mode();
            return Err(TerminalError::EnterAltScreen(e));
        }
        Ok(())
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        if TERMINAL_ACTIVE.swap(false, Ordering::SeqCst) {
            let mut stdout = io::stdout();
            let _ = execute!(stdout, LeaveAlternateScreen);
            let _ = disable_raw_mode();
        }
    }
}

/// Install a panic hook that restores the terminal if `TERMINAL_ACTIVE`.
/// Idempotent — calling more than once is a no-op.
/// CONTRACT: call once at program start, before `TerminalGuard::acquire`.
pub fn install_panic_hook() {
    if HOOK_INSTALLED.swap(true, Ordering::SeqCst) {
        return;
    }

    let default_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        if TERMINAL_ACTIVE.swap(false, Ordering::SeqCst) {
            let mut stdout = io::stdout();
            let _ = execute!(stdout, LeaveAlternateScreen);
            let _ = disable_raw_mode();
            let _ = execute!(stdout, cursor::Show);
        }
        default_hook(panic_info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cannot run in parallel — relies on global TERMINAL_ACTIVE state.
    /// Run via: cargo test -- --ignored lifecycle
    #[test]
    #[ignore]
    fn test_terminal_active_initial_false() {
        assert!(!TERMINAL_ACTIVE.load(Ordering::SeqCst));
    }

    #[test]
    fn test_panic_hook_install_idempotent() {
        install_panic_hook();
        install_panic_hook();
        assert!(HOOK_INSTALLED.load(Ordering::SeqCst));
    }
}
