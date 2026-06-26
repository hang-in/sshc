//! v0.10 G3: clipboard write with an OSC 52 fallback.
//!
//! Two paths:
//! 1. The system clipboard via `arboard` — works on macOS desktop
//!    sessions, X11/Wayland Linux with a compositor, Windows with a
//!    foreground process.
//! 2. OSC 52 escape sequence emitted to stdout — works when sshc is
//!    running in a terminal whose emulator honors OSC 52 (kitty,
//!    iTerm2, foot, alacritty, wezterm; tmux with `set -g
//!    set-clipboard on`). Useful when sshc is being driven from
//!    inside an SSH session itself, or on a headless Wayland setup
//!    where arboard can't connect.
//!
//! The caller (`App::copy_ssh_command_for_selected` in v0.9 G4) tells
//! the user *which path won* by surfacing the `ClipboardBackend` we
//! return.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use std::io::Write;

/// Which path actually delivered the text to the clipboard. Useful
/// to surface in the status bar so the user knows whether arboard
/// landed or the OSC 52 fallback fired.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardBackend {
    System,
    Osc52,
}

/// Failure to copy through every available path. The `String` is a
/// short, human-readable summary (mostly the arboard error followed
/// by why OSC 52 wasn't tried or didn't apply).
#[derive(Debug)]
pub struct ClipboardError(pub String);

impl std::fmt::Display for ClipboardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ClipboardError {}

/// Try the system clipboard via `arboard` first; on any error, fall
/// back to OSC 52 (unless the user opted out via `SSHC_NO_OSC52`).
/// Returns the backend that handled the request.
pub fn copy_to_clipboard(text: &str) -> Result<ClipboardBackend, ClipboardError> {
    match arboard::Clipboard::new().and_then(|mut c| c.set_text(text.to_owned())) {
        Ok(()) => Ok(ClipboardBackend::System),
        Err(arboard_err) => {
            if std::env::var_os("SSHC_NO_OSC52").is_some() {
                return Err(ClipboardError(format!(
                    "system clipboard failed ({arboard_err}); OSC 52 skipped (SSHC_NO_OSC52)"
                )));
            }
            emit_osc52(text)
                .map(|_| ClipboardBackend::Osc52)
                .map_err(|io_err| {
                    ClipboardError(format!(
                        "system clipboard failed ({arboard_err}); OSC 52 stdout write failed ({io_err})"
                    ))
                })
        }
    }
}

/// Build the OSC 52 escape sequence and write it to stdout.
///
/// Format (from the xterm CSI/OSC docs):
///
/// ```text
/// ESC ] 5 2 ; c ; <base64-of-text> ESC \
/// ```
///
/// The `c` selector targets the clipboard (the alternative `p`
/// targets primary selection — we don't use it). The string
/// terminator is `ESC \` (ST). Many emulators also accept BEL
/// (`\x07`) but `ESC \` is the spec-correct form.
fn emit_osc52(text: &str) -> std::io::Result<()> {
    let payload = build_osc52_payload(text);
    let mut out = std::io::stdout().lock();
    out.write_all(payload.as_bytes())?;
    out.flush()
}

/// Pure helper: render the OSC 52 escape sequence as a `String`.
/// Split out for unit tests so we don't have to capture stdout.
fn build_osc52_payload(text: &str) -> String {
    let encoded = B64.encode(text.as_bytes());
    format!("\x1b]52;c;{encoded}\x1b\\")
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::STANDARD as B64;
    use base64::Engine;

    #[test]
    fn osc52_payload_wraps_base64_with_escape_envelope() {
        let payload = build_osc52_payload("hello");
        // Envelope: starts with ESC ] 5 2 ; c ; …
        assert!(payload.starts_with("\x1b]52;c;"), "got {payload:?}");
        // Ends with the spec-correct String Terminator ESC \.
        assert!(payload.ends_with("\x1b\\"), "got {payload:?}");
        // Inner section is exactly base64(text).
        let inner = &payload["\x1b]52;c;".len()..payload.len() - "\x1b\\".len()];
        assert_eq!(inner, B64.encode("hello"));
    }

    #[test]
    fn osc52_payload_handles_empty_text() {
        let payload = build_osc52_payload("");
        assert_eq!(payload, "\x1b]52;c;\x1b\\");
    }

    #[test]
    fn osc52_payload_handles_long_text() {
        // 1 KB string — OSC 52 implementations sometimes cap at
        // 8KB-ish; we don't try to chunk, but we should at least
        // produce a clean payload for routine-sized commands.
        let text: String = "x".repeat(1024);
        let payload = build_osc52_payload(&text);
        // ESC ] 5 2 ; c ; (7) + base64 of 1024 bytes + ESC \ (2)
        assert_eq!(payload.len(), 7 + B64.encode(text.as_bytes()).len() + 2);
    }
}
