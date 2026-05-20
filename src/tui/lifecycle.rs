use std::io;
use std::panic;
use std::sync::atomic::{AtomicBool, Ordering};

use crossterm::{
    cursor, execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

use crate::error::TerminalError;

pub(crate) static RAW_ACTIVE: AtomicBool = AtomicBool::new(false);
pub(crate) static ALT_ACTIVE: AtomicBool = AtomicBool::new(false);
static HOOK_INSTALLED: AtomicBool = AtomicBool::new(false);

/// Which terminal screen the guard owns.
///
/// `Alternate` is the v0.3 manage-mode behaviour: raw mode + alternate
/// screen. `Inline(N)` is the v0.4 inline-mode behaviour: raw mode only,
/// no alternate screen, with `N` reserved viewport lines (the caller
/// uses this when constructing the ratatui `Terminal`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenMode {
    Alternate,
    Inline(u16),
}

/// RAII guard owning the terminal's raw mode (and optionally the alternate
/// screen). `Drop` runs the leave sequence idempotently and never panics.
///
/// Only one `TerminalGuard` may exist at a time.
pub struct TerminalGuard {
    mode: ScreenMode,
}

impl TerminalGuard {
    /// Acquire exclusive ownership of the terminal in the requested mode.
    ///
    /// `Alternate`: enable raw mode, enter the alternate screen, hide cursor.
    /// `Inline(N)`: enable raw mode, hide cursor. Does NOT enter the
    /// alternate screen — caller is responsible for constructing the
    /// ratatui Terminal with `TerminalOptions { viewport: Viewport::Inline(N) }`.
    ///
    /// CONTRACT: panics if another guard is already active.
    pub fn acquire(mode: ScreenMode) -> Result<Self, TerminalError> {
        if RAW_ACTIVE.load(Ordering::SeqCst) || ALT_ACTIVE.load(Ordering::SeqCst) {
            panic!("TerminalGuard::acquire called while another guard exists");
        }

        if let Err(e) = Self::enter(mode) {
            // Best-effort rollback in case raw came up but alt failed.
            let _ = execute!(io::stdout(), cursor::Show);
            return Err(e);
        }

        Ok(TerminalGuard { mode })
    }

    /// The mode this guard was acquired in.
    pub fn mode(&self) -> ScreenMode {
        self.mode
    }

    /// Suspend the terminal so a child process (ssh, $EDITOR) can take over
    /// stdio. Leaves the alternate screen (if active) and disables raw mode,
    /// then shows the cursor. No-op for whichever flag is already cleared.
    pub fn suspend(&mut self) -> Result<(), TerminalError> {
        let alt_res = if ALT_ACTIVE.swap(false, Ordering::SeqCst) {
            execute!(io::stdout(), LeaveAlternateScreen).map_err(TerminalError::LeaveAltScreen)
        } else {
            Ok(())
        };
        let raw_res = if RAW_ACTIVE.swap(false, Ordering::SeqCst) {
            disable_raw_mode().map_err(TerminalError::LeaveRawMode)
        } else {
            Ok(())
        };
        let _ = execute!(io::stdout(), cursor::Show);

        match (alt_res, raw_res) {
            (Err(e), _) | (_, Err(e)) => Err(e),
            (Ok(_), Ok(_)) => Ok(()),
        }
    }

    /// Re-enter the original mode after a suspension. No-op if already active.
    pub fn resume(&mut self) -> Result<(), TerminalError> {
        if RAW_ACTIVE.load(Ordering::SeqCst) || ALT_ACTIVE.load(Ordering::SeqCst) {
            return Ok(());
        }
        Self::enter(self.mode)
    }

    fn enter(mode: ScreenMode) -> Result<(), TerminalError> {
        enable_raw_mode().map_err(TerminalError::EnterRawMode)?;
        RAW_ACTIVE.store(true, Ordering::SeqCst);

        let mut stdout = io::stdout();
        if matches!(mode, ScreenMode::Alternate) {
            if let Err(e) = execute!(stdout, EnterAlternateScreen) {
                RAW_ACTIVE.store(false, Ordering::SeqCst);
                let _ = disable_raw_mode();
                return Err(TerminalError::EnterAltScreen(e));
            }
            ALT_ACTIVE.store(true, Ordering::SeqCst);
        }
        let _ = execute!(stdout, cursor::Hide);
        Ok(())
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        // Idempotent leave sequence. Order: alt → raw → cursor.
        if ALT_ACTIVE.swap(false, Ordering::SeqCst) {
            let _ = execute!(io::stdout(), LeaveAlternateScreen);
        }
        if RAW_ACTIVE.swap(false, Ordering::SeqCst) {
            let _ = disable_raw_mode();
        }
        let _ = execute!(io::stdout(), cursor::Show);
    }
}

/// Install a panic hook that restores the terminal regardless of which
/// `ScreenMode` was active. Idempotent — calling more than once is a no-op.
/// CONTRACT: call once at program start, before `TerminalGuard::acquire`.
pub fn install_panic_hook() {
    if HOOK_INSTALLED.swap(true, Ordering::SeqCst) {
        return;
    }

    let default_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        // Same idempotent sequence as Drop. Inline mode panics never touch
        // ALT_ACTIVE so LeaveAlternateScreen is skipped — avoids spurious
        // escapes from being emitted in a normal-mode terminal.
        if ALT_ACTIVE.swap(false, Ordering::SeqCst) {
            let _ = execute!(io::stdout(), LeaveAlternateScreen);
        }
        if RAW_ACTIVE.swap(false, Ordering::SeqCst) {
            let _ = disable_raw_mode();
        }
        let _ = execute!(io::stdout(), cursor::Show);
        default_hook(panic_info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cannot run in parallel — relies on global RAW_ACTIVE/ALT_ACTIVE state.
    /// Run via: cargo test -- --ignored lifecycle
    #[test]
    #[ignore]
    fn test_terminal_active_initial_false() {
        assert!(!RAW_ACTIVE.load(Ordering::SeqCst));
        assert!(!ALT_ACTIVE.load(Ordering::SeqCst));
    }

    #[test]
    fn test_panic_hook_install_idempotent() {
        install_panic_hook();
        install_panic_hook();
        assert!(HOOK_INSTALLED.load(Ordering::SeqCst));
    }

    #[test]
    fn test_screen_mode_equality() {
        assert_eq!(ScreenMode::Alternate, ScreenMode::Alternate);
        assert_eq!(ScreenMode::Inline(15), ScreenMode::Inline(15));
        assert_ne!(ScreenMode::Alternate, ScreenMode::Inline(15));
        assert_ne!(ScreenMode::Inline(10), ScreenMode::Inline(15));
    }
}
