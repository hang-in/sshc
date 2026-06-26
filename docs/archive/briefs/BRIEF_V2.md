# sshc v0.2.0 — Architect Brief (v2)

> Revision of v1 brief after architect review. Status: **implementation-ready spec**.
> The implementer (glm-5.1:cloud via tunaLlama) treats sections marked **CONTRACT**
> as binding. Sections marked **NOTE** are advisory but recommended.
>
> Reviewer of all implementation deliverables: this Claude session (architect).

---

## 0. Revision summary (vs v1)

| Change | v1 | v2 |
|---|---|---|
| In-Scope ambiguities | 4 unresolved | All 4 resolved (§3) |
| Risk-area questions | Open questions Q1–Q5 | All 5 decided (§5) |
| Module boundaries | implicit | explicit table + 5-item regression check (§9) |
| Test strategy | "minimum viable?" | 4-layer plan with assertions (§8) |
| Missing concerns | n/a | 7 added: resize, visual flash, last_connected timing, host-gone, filter-vs-status priority, RAII guard, logging |
| Pre-decisions | 6 items | refined: `r` key kept, mock-ssh via `Command::env` not `env::set_var`, 3000ms as const |
| Handoff | "later" | concrete task units (§13) wired to PLAN.md |

---

## 1. Context

`sshc` v0.1 is a Rust TUI managing SSH hosts from `~/.ssh/config`:
- ratatui + crossterm, hand-rolled parser with cycle-protected `Include`
- 43 tests passing (lib 26 + parser 11 + integration 6)
- `Host::fuzzy_match` via `nucleo`, `Match` directive isolated, quoted/inline-comment values handled
- Connection launches via `exec("ssh", alias)` — TUI never returns

v0.2 transforms one-shot exec into **round-trip spawn+wait**, with **last-host memory**.

---

## 2. Goal (one sentence)

Turn one-shot exec launching into round-trip spawn+wait, and remember the last connected host (in-memory) so the user can `r`-reconnect from the TUI after returning from ssh.

---

## 3. In Scope (clarified)

| # | Item | Clarification |
|---|---|---|
| 1 | Replace `exec` with `spawn+wait` | See §6.4 `ssh_run`. No `CommandExt::exec`. |
| 2 | Terminal toggle | RAII guard (`TerminalGuard`) with `AtomicBool` state tracker. §6.3. |
| 3 | Selection preserved | By **alias**, not index. If alias absent post-refresh, fallback to index 0. |
| 4 | Last-connected marker | Stored as `Option<String>` (alias). Rendered as **1-char `★` prefix** on its row in the host list. Color: yellow on default bg. Sort order unaffected. |
| 5 | `r` key reconnect | In **non-filter mode only**: triggers reconnect to `last_connected`. In filter mode, `r` is consumed as filter input (existing behavior). Silent no-op + status message `"no recent host to reconnect"` if `last_connected` is `None` or the alias no longer exists in current hosts. |
| 6 | Status bar | Shows ssh exit info for **non-zero / error** results only (see §5 Q4). Auto-dismiss after `STATUS_BAR_TIMEOUT_MS = 3000`. Any keypress also dismisses; the dismissing keypress is **also processed as an action** (i.e., not consumed — see §3 NOTE-A). |
| 7 | Integration tests | mock-ssh via `Command::env("PATH", ...)` — **never** `env::set_var` (thread-unsafe under `cargo test`). |
| 8 | Manual checklist | Lives at `docs/TESTING.md`. §10 has the minimum set. |

**NOTE-A** (dismiss behavior): in v1 the brief said "any keypress dismisses, consumed (no action)". v2 changes this to "dismisses but action still fires". Rationale: the user pressing `j` to scroll while a stale error is shown shouldn't lose the keystroke. The error was already visible long enough to register.

---

## 4. Out of Scope (v0.2)

- Persistent `last_connected` (deferred to v0.3)
- ssh `ControlMaster` connection multiplexing
- pty-based integration test harness
- Host editing forms (v0.3 candidate)
- Connection probing / liveness check
- `Match` directive evaluation (only isolation, already done in v0.1.x)
- Multi-session / tmux delegation

---

## 5. Risk-Area Decisions (architect answers)

### Q1 — Terminal symmetry

**CONTRACT**
- Enter sequence: `enable_raw_mode()` then `execute!(stdout, EnterAlternateScreen)`.
- Leave sequence: `execute!(stdout, LeaveAlternateScreen)` then `disable_raw_mode()`.
- Both sequences are wrapped inside `TerminalGuard` methods (`acquire/resume` for enter, `suspend/drop` for leave). Each method checks `TERMINAL_ACTIVE` (AtomicBool) and is a no-op if already in the requested state.
- N consecutive (suspend, resume) cycles are safe because each call is a fresh leave/enter pair. Asserted in unit tests via `TERMINAL_ACTIVE` state transitions (see §8.1).

**NOTE**: cursor visibility is restored by `Terminal::show_cursor()` (ratatui) inside `Drop` only — not during suspend, since the child process manages cursor itself.

### Q2 — Panic hook idempotency

**CONTRACT**
- Single global `static TERMINAL_ACTIVE: AtomicBool`.
- Hook installed once via `install_panic_hook()` BEFORE `TerminalGuard::acquire`.
- Hook reads `TERMINAL_ACTIVE`. If `true`, executes leave sequence and clears the flag. If `false`, does nothing (suspended window or already-restored case).
- Hook then defers to `default_hook(panic_info)` — does NOT call `eprintln!` itself (avoids double output).
- `is_raw_mode_enabled()` is NOT used. It does not cover alt-screen state.

### Q3 — Signal flow

**CONTRACT**
- `TerminalGuard::suspend()` calls leave sequence BEFORE `ssh_run` is invoked. With raw mode disabled and alt-screen exited, the terminal returns to cooked mode and ssh inherits stdio normally.
- No `setsid()` call: ssh inherits parent's pgid. SIGINT (Ctrl-C) is delivered to the foreground process group — both ssh and sshc.
- ssh has its own SIGINT handler and exits with code 130. The parent (`sshc`) ignores SIGINT during the `wait()` syscall on most platforms (default Rust behavior is to NOT install a SIGINT handler; the kernel's default for an interactive shell-spawned process is to terminate, but during a blocking `wait` the signal interrupts `wait` returning `EINTR`, which Rust's `Command::status` retries internally).
- **Implementation requirement**: verify with `mock_ssh` integration test that sends `exit 130` and confirms parent observes `SshResult::Interrupted` and continues running.
- **If** the test reveals that the parent is killed by SIGINT before observing the child exit, install a temporary SIGINT-ignore handler around `Command::status()` using `ctrlc` crate. Flag this in implementation if it occurs.

### Q4 — Exit status semantics

**CONTRACT** — `classify_exit_status`:

```rust
use std::os::unix::process::ExitStatusExt;

fn classify_exit_status(s: std::process::ExitStatus) -> SshResult {
    if let Some(code) = s.code() {
        match code {
            0 => SshResult::Success,
            130 => SshResult::Interrupted,
            255 => SshResult::ConnectFailed(255),
            other => SshResult::Failed(other),
        }
    } else if let Some(sig) = s.signal() {
        match sig {
            2 | 15 => SshResult::Interrupted, // SIGINT, SIGTERM
            _ => SshResult::Crashed(sig),
        }
    } else {
        SshResult::UnknownTermination
    }
}
```

Status bar policy:

| Result | Status bar |
|---|---|
| `Success` | silent |
| `Interrupted` | silent |
| `ConnectFailed(c)` | `"Connection failed ({c}): {alias}"` |
| `Failed(c)` | `"ssh exit {c}: {alias}"` |
| `Crashed(sig)` | `"ssh killed by signal {sig}: {alias}"` |
| `UnknownTermination` | `"ssh terminated abnormally: {alias}"` |

Errors before spawn complete:
| `SshError::LaunchFailed(io_err)` | `"failed to launch ssh: {io_err}"` |
| `SshError::WaitFailed(io_err)`   | `"failed to wait for ssh: {io_err}"` |

### Q5 — Test strategy

See §8. Pty harness is OUT of scope for v0.2.

---

## 6. Module Architecture and Public API

### 6.1 Module tree (v0.2 target)

```
src/
├── main.rs              — orchestration ONLY (≤ 80 lines)
├── lib.rs
├── error.rs             — NEW: SshError, TerminalError, EditorError, AppError
├── app.rs               — domain state (last_connected, status_message)
├── config/              — unchanged (parser, model)
├── tui/                 — NEW directory
│   ├── mod.rs
│   └── lifecycle.rs     — TerminalGuard (RAII), install_panic_hook, TERMINAL_ACTIVE
├── ui/                  — presentation (existing)
│   ├── mod.rs
│   ├── layout.rs
│   ├── list.rs          — ★ marker rendering
│   └── status_bar.rs    — NEW: StatusMessage with expiry
└── exec/
    ├── mod.rs
    ├── ssh.rs           — spawn+wait, classify_exit_status, SshResult
    └── editor.rs        — unchanged
```

### 6.2 `src/error.rs` (NEW)

**CONTRACT**: Module-boundary public APIs return these types, NOT `anyhow::Result`. `main.rs` may compose them via `AppError`.

```rust
use std::fmt;

#[derive(Debug)]
pub enum SshError {
    LaunchFailed(std::io::Error),  // ssh binary missing / no permission
    WaitFailed(std::io::Error),    // status() failed after spawn
}

#[derive(Debug)]
pub enum TerminalError {
    EnterRawMode(std::io::Error),
    EnterAltScreen(std::io::Error),
    LeaveAltScreen(std::io::Error),
    LeaveRawMode(std::io::Error),
}

#[derive(Debug)]
pub enum EditorError {
    LaunchFailed(std::io::Error),
}

#[derive(Debug)]
pub enum AppError {
    Terminal(TerminalError),
    Ssh(SshError),
    Editor(EditorError),
    Io(std::io::Error),
}

// Each error type: impl fmt::Display + std::error::Error + From<inner io::Error>
// AppError: impl From<TerminalError>, From<SshError>, From<EditorError>, From<std::io::Error>
```

### 6.3 `src/tui/lifecycle.rs` (NEW)

```rust
use std::sync::atomic::{AtomicBool, Ordering};
use crate::error::TerminalError;

/// Whether the terminal is currently in raw mode + alternate screen.
/// Used by both TerminalGuard and the panic hook.
pub(crate) static TERMINAL_ACTIVE: AtomicBool = AtomicBool::new(false);

/// RAII guard owning the terminal raw mode + alternate screen.
///
/// CONTRACT:
/// - Only one `TerminalGuard` may exist at a time. Construction panics if
///   `TERMINAL_ACTIVE` is already `true` (programmer error).
/// - On Drop, the leave sequence runs idempotently and `TERMINAL_ACTIVE` is cleared.
/// - `suspend()` and `resume()` may be called arbitrarily many times in pairs.
/// - Method failures return `TerminalError` and leave the state cleared
///   (best-effort rollback): if EnterAltScreen fails after raw mode was enabled,
///   raw mode is disabled before returning Err.
pub struct TerminalGuard {
    // private fields, e.g. std::io::Stdout handle
}

impl TerminalGuard {
    /// Enable raw mode then enter alternate screen.
    /// Sets `TERMINAL_ACTIVE = true` on success.
    pub fn acquire() -> Result<Self, TerminalError>;

    /// Leave alt screen + disable raw mode. Sets `TERMINAL_ACTIVE = false`.
    /// No-op (returns Ok) if already suspended.
    pub fn suspend(&mut self) -> Result<(), TerminalError>;

    /// Re-enable raw mode + enter alt screen. Sets `TERMINAL_ACTIVE = true`.
    /// No-op if already active.
    pub fn resume(&mut self) -> Result<(), TerminalError>;
}

impl Drop for TerminalGuard {
    fn drop(&mut self);  // idempotent leave; never panics; never returns err
}

/// Install a panic hook that restores the terminal state if `TERMINAL_ACTIVE`.
/// Idempotent — safe against double-panic / already-disabled states.
/// CONTRACT: call exactly once at program startup, before `TerminalGuard::acquire`.
pub fn install_panic_hook();
```

**Implementation note**: `TERMINAL_ACTIVE` must be set AFTER successful enter sequence (so a failure mid-enter doesn't poison the flag), and cleared BEFORE leave sequence runs to prevent the panic hook re-entering during leave.

### 6.4 `src/exec/ssh.rs` (REWRITTEN)

```rust
use crate::error::SshError;

#[derive(Debug, PartialEq, Eq)]
pub enum SshResult {
    Success,
    Interrupted,
    ConnectFailed(i32),
    Failed(i32),
    Crashed(i32),         // killed by signal `i32`
    UnknownTermination,
}

/// Spawn ssh with the given alias, inherit stdio, wait for exit, classify.
///
/// CONTRACT:
/// - PRE: caller has suspended the TUI terminal (raw mode off, alt screen exited).
///   Violation produces correct ssh behavior but garbled TUI on resume — caller's bug.
/// - This function NEVER manipulates terminal state.
/// - `ssh_binary` is the path/name of the ssh executable. Production: "ssh".
///   Tests: path to mock_ssh binary.
/// - Returns Err(SshError::LaunchFailed) if spawn fails.
/// - Returns Err(SshError::WaitFailed) if status() fails post-spawn.
/// - Returns Ok(SshResult::*) for any termination.
pub fn ssh_run(host_alias: &str, ssh_binary: &str) -> Result<SshResult, SshError>;

/// Pure function — testable without spawning. Exposed for unit tests.
pub(crate) fn classify_exit_status(status: std::process::ExitStatus) -> SshResult;
```

### 6.5 `src/ui/status_bar.rs` (NEW)

```rust
use std::time::{Duration, Instant};

pub const STATUS_BAR_TIMEOUT_MS: u64 = 3_000;

/// An ephemeral status line with a deadline.
#[derive(Debug, Clone)]
pub struct StatusMessage {
    text: String,
    expires_at: Instant,
}

impl StatusMessage {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            expires_at: Instant::now() + Duration::from_millis(STATUS_BAR_TIMEOUT_MS),
        }
    }

    pub fn text(&self) -> &str { &self.text }

    /// True if not yet expired.
    pub fn is_visible(&self) -> bool { Instant::now() < self.expires_at }
}
```

### 6.6 `src/app.rs` (EXTENDED)

```rust
use crate::ui::status_bar::StatusMessage;
use crate::exec::ssh::SshResult;

pub struct App {
    // ... existing fields ...
    pub last_connected: Option<String>,            // alias
    pub status_message: Option<StatusMessage>,
    pending_action: Option<AppAction>,             // private — drained by main loop
    matcher: Matcher,
}

#[derive(Debug, PartialEq, Eq)]
pub enum AppAction {
    Quit,
    Connect(String),    // alias
    EditConfig,
}

impl App {
    /// Drain the pending action for the main loop. Resets internal flags.
    pub fn take_action(&mut self) -> Option<AppAction>;

    /// Called after ssh exits. Updates last_connected (already set pre-spawn)
    /// and status_message (only for non-silent results).
    pub fn on_ssh_finished(&mut self, host_alias: &str, result: SshResult);

    /// Replaces refresh_hosts. Preserves selection by alias.
    pub fn replace_hosts(&mut self, new_hosts: Vec<Host>);

    // Internal:
    fn try_reconnect(&mut self);
    fn dismiss_status_if_expired(&mut self);
}
```

**CONTRACT**:
- `last_connected` is set to `Some(alias)` BEFORE the main loop suspends the terminal (so even failed connections record a "most recent attempt"). This is the responsibility of `App::take_action` returning `Connect(alias)`.
- `on_ssh_finished` only writes to `status_message`. Never touches terminal state. Never calls io.
- Filter mode and reconnect: in `handle_key`, when `filter_mode == true`, the `'r'` char is appended to query as usual. When `filter_mode == false`, `'r'` is wired to `try_reconnect`.

### 6.7 `src/main.rs` (REWRITTEN)

```rust
fn main() -> Result<(), AppError> {
    init_logging();                       // env_logger; never prints to stdout/stderr while TUI active
    install_panic_hook();
    let mut guard = TerminalGuard::acquire()?;
    let mut terminal = build_terminal()?;
    let mut app = App::new(parse_config(&default_config_path()));

    loop {
        run_event_loop(&mut terminal, &mut app)?;
        match app.take_action() {
            None | Some(AppAction::Quit) => break,
            Some(AppAction::EditConfig) => handle_edit(&mut guard, &mut terminal, &mut app)?,
            Some(AppAction::Connect(alias)) => handle_connect(&mut guard, &mut terminal, &mut app, &alias)?,
        }
    }
    Ok(())
}

fn handle_connect(
    guard: &mut TerminalGuard,
    terminal: &mut Terminal<...>,
    app: &mut App,
    alias: &str,
) -> Result<(), AppError> {
    app.last_connected = Some(alias.to_string());  // record BEFORE spawn
    guard.suspend()?;
    let result = ssh_run(alias, "ssh");
    guard.resume()?;
    terminal.clear()?;                              // force redraw on resume
    match result {
        Ok(r) => app.on_ssh_finished(alias, r),
        Err(e) => app.status_message = Some(StatusMessage::new(format!("{}", e))),
    }
    Ok(())
}
```

**CONTRACT (orchestration boundary)**:
- `main.rs` calls `guard.suspend()` exactly once per Connect/EditConfig action and `guard.resume()` exactly once.
- `main.rs` does NOT directly call crossterm primitives (uses `TerminalGuard` only).
- `main.rs` does NOT contain ssh exit code logic (delegated to `app.on_ssh_finished`).

---

## 7. Contracts and Pre/Post Conditions (matrix)

| Boundary | Precondition | Postcondition |
|---|---|---|
| `TerminalGuard::acquire` | `TERMINAL_ACTIVE == false` | `TERMINAL_ACTIVE == true` or unchanged on Err |
| `TerminalGuard::suspend` | `TERMINAL_ACTIVE == true` (else no-op) | `TERMINAL_ACTIVE == false` |
| `TerminalGuard::resume` | `TERMINAL_ACTIVE == false` (else no-op) | `TERMINAL_ACTIVE == true` |
| `TerminalGuard::drop` | any | `TERMINAL_ACTIVE == false` |
| `ssh_run` | `TERMINAL_ACTIVE == false` | unchanged |
| `App::on_ssh_finished` | any | only `app.status_message` may change |
| `App::take_action` | any | clears internal pending flags |
| `panic hook` | any | `TERMINAL_ACTIVE == false` if it was true |

---

## 8. Test Strategy (4 layers)

### 8.1 Unit tests (no real TTY)
- `tui::lifecycle`:
  - state-transition tests on a `TerminalGuard` BUT mocked at the crossterm boundary
    (use `#[cfg(test)]` to replace actual crossterm calls with a counter — implementer's choice
    whether to introduce a trait or use a feature flag). Minimum: assert `TERMINAL_ACTIVE`
    transitions through N (suspend, resume) cycles.
  - panic hook idempotency: install hook, set `TERMINAL_ACTIVE = false` manually, invoke hook
    via `std::panic::resume_unwind` in a child thread — verify no double leave.
- `exec::ssh::classify_exit_status`: exhaustive on (code, signal) inputs.
- `ui::status_bar::StatusMessage`: `is_visible()` true immediately, false after sleep
  > `STATUS_BAR_TIMEOUT_MS`. Use a smaller timeout via const override for the test (or
  test the comparison logic with a constructed `Instant`).
- `app::App::on_ssh_finished`: for each `SshResult` variant, assert correct `status_message`
  presence/absence and content.
- `app::App::replace_hosts`: selection-by-alias preserved when alias still present;
  fallback to 0 when alias removed.
- `app::App` reconnect: `try_reconnect` with `last_connected = None` sets status message
  `"no recent host to reconnect"`. With valid alias, returns `Connect(alias)` via `take_action`.

### 8.2 Integration tests (mock ssh)
- Build a small Rust mock binary at `tests/bin/mock_ssh.rs` (cargo workspace bin or a build
  script — implementer choice). It reads `$MOCK_SSH_EXIT_CODE` and exits with that code.
  If unset, exits 0.
- `tests/round_trip_test.rs` (NEW):
  - test_round_trip_exit_0: invoke `ssh_run` with PATH overridden via `Command::env`
    to point at mock_ssh; assert `SshResult::Success`.
  - test_round_trip_exit_255: assert `SshResult::ConnectFailed(255)`.
  - test_round_trip_exit_130: assert `SshResult::Interrupted`.
  - test_round_trip_exit_signal: mock_ssh self-kills via SIGTERM; assert `SshResult::Interrupted`
    (sig 15) or `Crashed(sig)` for SIGKILL. Implementer picks portable signal.
  - test_round_trip_launch_failed: PATH points at nonexistent file; assert `SshError::LaunchFailed`.

### 8.3 Regression
All v0.1.x tests (43) must pass unchanged.

### 8.4 Manual (`docs/TESTING.md` — see §10 for minimum)

---

## 9. Module Responsibility Boundaries (REGRESSION GUARD)

| Module | Owns | Forbidden |
|---|---|---|
| `app.rs` | domain state, key event → state delta | crossterm imports, `std::process::Command`, terminal state mutation |
| `tui/lifecycle.rs` | raw mode + alt screen + AtomicBool + panic hook | `Host` import, ssh exec, key handling |
| `exec/ssh.rs` | spawn + wait + exit classification | terminal state mutation, ui imports |
| `exec/editor.rs` | build editor Command | terminal state mutation |
| `ui/*` | render App state | mutating App state |
| `main.rs` | orchestration | ≤ 80 lines, no business logic |
| `error.rs` | typed errors | depends on nothing else in the crate |

### Regression check (must pass before merge of any v0.2 commit)

Mechanical (grep) checks — runnable by both implementer and reviewer:

```bash
# 1. app.rs must not import crossterm or spawn processes
grep -E "use crossterm|std::process::Command" src/app.rs && echo FAIL

# 2. exec/ssh.rs must not enable raw mode or enter alt screen
grep -E "enable_raw_mode|EnterAlternateScreen|LeaveAlternateScreen|disable_raw_mode" src/exec/ssh.rs && echo FAIL

# 3. tui/lifecycle.rs must not import Host
grep -E "use .*config::model" src/tui/lifecycle.rs && echo FAIL

# 4. main.rs must be ≤ 80 lines (excluding blank/comment lines)
test "$(grep -cvE '^\s*($|//)' src/main.rs)" -le 80 || echo FAIL

# 5. public APIs in lib boundaries must not return anyhow::Result
#    (anyhow allowed only in main.rs)
grep -lE "anyhow::Result|anyhow::Error" src/app.rs src/tui/lifecycle.rs src/exec/ssh.rs src/ui/*.rs && echo FAIL
```

---

## 10. Manual Test Checklist (excerpt — full at `docs/TESTING.md`)

Each of the following on **macOS Terminal.app** + **iTerm2** at minimum:

- [ ] Start `sshc`, pick a host, Enter → ssh connects, `exit` from ssh, TUI reappears with same host selected.
- [ ] During ssh: Ctrl-C → ssh exits 130, TUI reappears, status bar **silent**.
- [ ] Connect to a host with bad hostname → status bar shows `Connection failed (255): {alias}` for ~3s, then clears.
- [ ] Connect to a host; after returning, the host shows `★` prefix in the list.
- [ ] Press `r` after a successful connect → reconnects to the same host.
- [ ] Press `r` with no prior connect → status bar `"no recent host to reconnect"`.
- [ ] 10 round-trips in a row → cursor placement, scrollback, prompt color all intact.
- [ ] Resize terminal during one of the round-trips → next TUI draw uses new size.
- [ ] (debug build only) inject `panic!("test")` in `handle_connect` → terminal restored cleanly, panic message visible on stderr, no garbled mode.

---

## 11. Implementation Notes (for implementer; non-obvious pitfalls)

1. **Order of `TERMINAL_ACTIVE` mutation vs underlying crossterm calls**: set the flag AFTER successful enter; clear it BEFORE leave. This avoids the panic hook re-entering during leave.
2. **`Stdout` lock during panic**: do not hold a `Stdout` lock across `panic_info` printing; use `let _ =` patterns.
3. **`Drop` must never panic**: `TerminalGuard::drop` uses `let _ = ...` on every crossterm call.
4. **`Command::env("PATH", ...)`** for mock_ssh: set only the spawned child's env, not the test process's env. This avoids races with other parallel tests.
5. **ratatui resize**: no explicit handling needed — `terminal.draw()` on next loop tick reads new size. Just call `terminal.clear()` after `guard.resume()` to force full redraw.
6. **`Instant` in `StatusMessage`**: unit tests cannot easily fast-forward time. Either (a) extract the comparison `now >= expires_at` into a helper that takes `now: Instant` as a parameter, or (b) use a very short timeout in tests via a `#[cfg(test)]` const override.
7. **Selection by alias** in `replace_hosts`: store the current `selected_alias = filtered.get(selected).and_then(|i| hosts[*i].alias.clone())` BEFORE replacing `hosts`, then rebuild `filtered`, then find new index by alias. If not found, use 0.
8. **`last_connected` recorded before spawn**: this means even failed connections become "the most recent attempt". This is intentional — users want `r` to retry the bad host.
9. **Filter mode `r` handling**: must NOT route through `try_reconnect` when in filter mode. Test must cover this.
10. **Logging**: use `log::warn!` etc.; do NOT call `eprintln!` while `TERMINAL_ACTIVE` (would corrupt TUI). `env_logger` writes to stderr — if running with `RUST_LOG=warn` and TUI active, log writes appear in alt-screen which is then discarded on exit, which is fine.

---

## 12. Pre-decisions (final state)

| Decision | Status | Note |
|---|---|---|
| spawn+wait (not pty/tmux) | ✅ kept | |
| No persistence in v0.2 | ✅ kept | |
| `r` key for reconnect | ✅ kept | acknowledged minor convention clash w/ vim's `r` |
| 3s status timeout | ✅ kept | as `STATUS_BAR_TIMEOUT_MS` const |
| Mock ssh via PATH | ⚠️ revised | `Command::env`, NOT `env::set_var` |
| Status bar key dismiss | ⚠️ revised | dismiss + still-process-action (§3 NOTE-A) |

---

## 13. Handoff Spec (task units)

Task unit table — drives `PLAN_V0.2.md` execution. Each task lists:
- **Files** it owns
- **Dependencies** (must complete before)
- **Acceptance** (must-pass test + grep)
- **Estimated parallelism** for the 3-model Ollama Cloud limit.

| ID | Title | Files | Depends on | Parallelizable with |
|---|---|---|---|---|
| T1 | `error.rs` types | `src/error.rs`, `src/lib.rs` (mod decl) | – | T2, T3 |
| T2 | `tui/lifecycle.rs` skeleton + tests | `src/tui/lifecycle.rs`, `src/tui/mod.rs`, `src/lib.rs` | – | T1, T3 |
| T3 | `ui/status_bar.rs` + tests | `src/ui/status_bar.rs`, `src/ui/mod.rs` | – | T1, T2 |
| T4 | `exec/ssh.rs` rewrite + `classify_exit_status` tests | `src/exec/ssh.rs` | T1 | T5 |
| T5 | `app.rs` extensions: last_connected, status, on_ssh_finished, reconnect, replace_hosts | `src/app.rs` | T1, T3 | T4 |
| T6 | `ui/list.rs` ★ marker | `src/ui/list.rs`, `src/ui/layout.rs` (status row) | T3, T5 | T7 |
| T7 | mock_ssh binary + integration tests | `tests/bin/mock_ssh.rs`, `tests/round_trip_test.rs` | T4 | T6 |
| T8 | `main.rs` orchestration rewrite | `src/main.rs` | T1, T2, T4, T5 | – |
| T9 | `docs/TESTING.md` manual checklist | `docs/TESTING.md` | T8 | – |

**Parallel rounds** (Ollama Cloud 3-model limit):
- **Round R1** (independent foundation): T1, T2, T3 — three slots
- **Round R2** (post-foundation): T4, T5 — two slots (T6 is held until both T3,T5 done)
- **Round R3**: T6, T7 — two slots (T7 depends only on T4 which is in R2)
- **Round R4** (serial — final integration): T8
- **Round R5**: T9

Actual scheduling: see `PLAN_V0.2.md` §3.

---

## End of Brief v2.
