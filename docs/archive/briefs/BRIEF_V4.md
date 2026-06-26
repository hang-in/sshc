# sshc v0.4.0 — Architect Brief

## 1. Context

sshc is a Rust TUI for managing SSH host connections. Shipped to date:

- **v0.1.0** — list non-wildcard `Host` entries, fuzzy filter, `exec()` ssh.
- **v0.2.0** — spawn+wait round-trip (TUI suspended around ssh), `★` marker,
  `r` reconnect, transient status bar, panic-safe terminal restore.
- **v0.3.0** — first-run setup (`Include` injection), `sshc.conf` for
  sshc-managed hosts, tags (`# @tags: ...`), TCP probe glyphs, modal-form
  CRUD (`a`/`m`/`d`/`t`), source-aware UI, centered dynamic panel, dual-mode
  status indicator.

Stack: ratatui 0.29 + crossterm 0.28 + nucleo + nix flock + serde/toml.
168 tests passing. glm-5.1 (Ollama Cloud) delegation workflow established.

## 2. v0.4.0 Goal

Add an **inline fzf-pattern mode** for quick host selection that stays in
the shell flow, while preserving the existing alternate-screen full TUI
for management operations.

The default command (`sshc`) becomes inline. The legacy v0.3 behaviour
moves behind `-m` / `--manage`. This is a deliberate breaking change for
a single-user tool; the daily-use path is "browse and ssh", management
is occasional.

## 3. Mode split

| Command | Mode | Capabilities |
|---|---|---|
| `sshc` | **Inline** (new default) | List, fuzzy filter, select → ssh |
| `sshc -m` / `sshc --manage` | **Manage** (v0.3 alternate screen) | Full TUI: CRUD, tags, probes, edit, last-marker |

Both modes share: host loading (parser + Include traversal), fuzzy filter
implementation, ssh spawn+wait, state.toml `last_connected_alias`.

Both modes diverge on: terminal lifecycle, key bindings, post-ssh
behaviour, render layout.

## 4. Inline mode spec

### 4.1 Behaviour
1. Run `sshc` from a shell prompt.
2. Terminal stays in normal mode. **No alternate screen.**
3. Inline viewport opens **below the prompt** via
   `ratatui::TerminalOptions { viewport: Viewport::Inline(N) }`.
4. Host list + filter bar render in the viewport.
5. User selects with `↑/↓` or `j/k`, `Enter` → viewport clears, raw mode
   off, cursor shown, ssh spawn+wait runs.
6. After ssh exit → process exits. Control returns to the shell.
   **The inline UI is NOT re-entered.**
7. Inline viewport area is cleared on exit (`terminal.clear()` before
   tear-down). The shell only sees the "Connecting to <alias>..." line
   plus whatever ssh prints. Decision after R0 prototype verification:
   fzf-style clean exit reads better than leaving the host list in
   scrollback.

### 4.2 Viewport height
- Default: `15` lines.
- Effective: `max(min(15, terminal_height - 5), 8)`.
- If `terminal_height < 12`: print a one-line stderr warning and fall back
  to **manage mode** (alternate screen). Force-inline (`-i`/`--inline`) is
  out of scope for v0.4.

### 4.3 Display
- Bordered header line: `sshc <N>` and the filter query inline.
- Columns: `Alias` | `Account` | `Host` | `Port` | `St` (status: probe
  glyph + last marker, same as manage mode).
- **Tag column omitted** in inline mode. Tag prefix on Alias also omitted
  (space-constrained).
- `★` last-connected marker kept in the St column.
- No probe indicator wiring in v0.4 inline mode — Status column carries
  only the `★` / blank marker. Probe glyph stays manage-only (probes are
  expensive to spin up for a one-shot inline session).

### 4.4 Key bindings (inline subset)
| Key | Action |
|---|---|
| `↑`/`↓`, `j`/`k` | navigate |
| any printable char | append to filter query (immediate filter — fzf muscle memory) |
| `Backspace` | delete last char of filter query |
| `Esc` | if query non-empty: clear query; if empty: cancel + exit |
| `Ctrl+C` | cancel + exit |
| `Enter` | spawn ssh on selected host, then exit |
| `r` | reconnect to `state.memory.last_connected_alias` if any |

No `a`/`d`/`m`/`t`/`e`/`?` (those are management-only). No `/` filter
prefix (immediate filter mode).

### 4.5 Round-trip behaviour
- `Enter` chosen: `TerminalGuard::suspend()` → clear viewport area →
  `ssh_run(alias, "ssh")` → ssh exits → process exits with the ssh status.
- ssh failure: status code propagates. No "press a key to continue".
- Inline mode never resumes UI after ssh. Re-running `sshc` is one keystroke.

## 5. Manage mode spec

Existing v0.3 alternate-screen TUI, with the following key remapping:

| Key | v0.3 | v0.4 Manage |
|---|---|---|
| `Enter` | ssh connect | **Open edit form** for selected host |
| `s` | (unused) | **ssh connect** to selected host |
| `m` | edit form | **removed** (merged into `Enter`) |
| `a`, `d`, `t`, `e` | unchanged | unchanged |
| `r` | reconnect last | unchanged (kept — light, occasionally useful) |
| `q`, `Esc`, `/`, `?` | unchanged | unchanged |

### 5.1 Enter on read-only hosts
A host whose `source_file` is not `~/.ssh/config.d/sshc.conf` cannot be
edited via the form. Enter on such a host **opens `$EDITOR` at the host's
line** (same effect as `e`). Rationale: "Enter = do something with this
row" stays consistent.

### 5.2 Manage mode delegates
- Probe pool, modal subsystem, forms, tag editor, first-run setup flow:
  unchanged from v0.3.
- `AppAction` adds no new variants in v0.4; the Enter→form rerouting is
  a `handle_list_key` change only.

## 6. CLI dispatch

```rust
// main.rs (sketch)
enum ScreenMode { Alternate, Inline(u16) }

fn parse_mode() -> ScreenMode {
    let manage = std::env::args().any(|a| a == "-m" || a == "--manage");
    if manage {
        ScreenMode::Alternate
    } else {
        let h = terminal_height();
        if h < 12 {
            eprintln!("terminal too small for inline mode; falling back to --manage");
            ScreenMode::Alternate
        } else {
            ScreenMode::Inline(((h.saturating_sub(5)).min(15)).max(8))
        }
    }
}

fn main() -> Result<(), AppError> {
    match parse_mode() {
        ScreenMode::Inline(h) => run_inline(h),
        ScreenMode::Alternate => run_manage(),
    }
}
```

No new dependency. `clap` is overkill for one flag.

## 7. Module architecture

### 7.1 Target tree (additions to v0.3)

```
src/
├── app.rs                — existing (manage mode App). Enter/s/m rebind.
├── inline_app.rs         — NEW. InlineApp + InlineAction.
├── tui/
│   ├── lifecycle.rs      — TerminalGuard extended with ScreenMode.
│   ├── runtime.rs        — existing manage helpers + new inline helpers.
├── main.rs               — CLI dispatch.
```

### 7.2 `src/inline_app.rs` — NEW

```rust
pub struct InlineApp {
    pub hosts: Vec<Host>,
    pub filtered: Vec<usize>,
    pub selected: usize,
    pub query: String,
    pub last_connected: Option<String>,
    pending_action: Option<InlineAction>,
    matcher: nucleo::Matcher,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum InlineAction {
    Quit,
    Connect(String),
    Reconnect,
}

impl InlineApp {
    pub fn new(hosts: Vec<Host>) -> Self;
    pub fn new_with_state(hosts: Vec<Host>, state: &crate::state::State) -> Self;
    pub fn handle_key(&mut self, key: KeyEvent);
    pub fn take_action(&mut self) -> Option<InlineAction>;
    pub fn has_pending_action(&self) -> bool;
    pub fn selected_host(&self) -> Option<&Host>;
    pub fn host_count(&self) -> usize;
    pub fn total_host_count(&self) -> usize;
}
```

InlineApp is intentionally lean: no modes, no probes, no state mutation
on connect (the runtime persists last_connected_alias via state::save
after the ssh call returns).

### 7.3 `src/tui/lifecycle.rs` — extended

```rust
#[derive(Debug, Clone, Copy)]
pub enum ScreenMode {
    Alternate,
    Inline(u16),  // viewport line count
}

pub struct TerminalGuard {
    mode: ScreenMode,
    raw_active: bool,
    alt_active: bool,
}

impl TerminalGuard {
    pub fn acquire(mode: ScreenMode) -> Result<Self, AppError>;
    pub fn suspend(&mut self) -> Result<(), AppError>;
    pub fn resume(&mut self) -> Result<(), AppError>;
}

pub fn install_panic_hook();  // idempotent leave: safe regardless of which mode entered
```

`acquire`:
- `Alternate` → enter raw mode + alternate screen + hide cursor (v0.3 behaviour).
- `Inline(h)` → enter raw mode + hide cursor. **No alternate screen.**

`suspend` (ssh round-trip): disable raw, show cursor, leave alternate
*only if active*. Symmetric `resume` re-applies based on mode. In inline
mode, `resume` is never called by the inline runtime; suspend is enough.

`install_panic_hook` must be safe regardless of mode. Leave-alternate is
idempotent on crossterm targets: calling `LeaveAlternateScreen` when
alternate is not active is a no-op or emits a benign escape. Verified
in prototype before R0.

### 7.4 `src/tui/runtime.rs` — extension

```rust
// Existing v0.3 manage helpers retained verbatim:
pub fn run_event_loop(terminal, app, probe_pool) -> Result<(), AppError>;
pub fn handle_connect(guard, terminal, app, alias) -> Result<(), AppError>;
pub fn handle_edit(guard, terminal, app, config_path) -> Result<(), AppError>;
pub fn handle_inject_include(app);

// New inline helpers:
pub fn run_event_loop_inline(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut InlineApp,
) -> Result<(), AppError>;

pub fn handle_connect_inline(
    guard: &mut TerminalGuard,
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut InlineApp,
    alias: &str,
) -> Result<SshResult, AppError>;
// Returns the SshResult so main.rs can choose the process exit code.
// Does NOT call guard.resume() — inline mode exits after ssh.
```

### 7.5 `src/main.rs` — dispatch

```rust
fn main() -> std::process::ExitCode {
    env_logger::init();
    install_panic_hook();
    let mode = parse_mode();
    let result = match mode {
        ScreenMode::Inline(h) => run_inline(h),
        ScreenMode::Alternate => run_manage(),
    };
    match result {
        Ok(code) => code,
        Err(e) => { eprintln!("{e}"); ExitCode::FAILURE }
    }
}

fn run_inline(viewport_height: u16) -> Result<ExitCode, AppError>;
fn run_manage() -> Result<ExitCode, AppError>;
```

Both helpers are short. `run_inline` skips first-run setup (the manage
flow handles that); inline simply reads the existing hosts and runs.
If the user has never run manage mode, inline still works (state.toml
defaults are fine, sshc.conf simply doesn't exist or is empty).

## 8. Risk-area decisions

### Q1 — Inline panic restoration
TerminalGuard tracks `raw_active` and `alt_active`. Drop and panic hook
call only the entries that were activated. install_panic_hook installs
a closure that performs `disable_raw_mode` + `Show cursor` + (if
applicable) `LeaveAlternateScreen`. The hook holds an `Arc<AtomicBool>`
or reads from `TerminalGuard::ACTIVE` so it can decide.

Simpler alternative: always call `disable_raw_mode` + `Show cursor`
unconditionally (both idempotent), and skip `LeaveAlternateScreen` if
inline mode is currently active. Track via a static `OnceLock<ScreenMode>`.

### Q2 — Inline ssh round-trip
- spawn pre-work: `terminal.clear()` + `terminal.draw(|_| {})` empty
  frame so the viewport collapses; `guard.suspend()` flips raw off and
  cursor on.
- `ssh_run(alias, "ssh")` runs (existing function, unchanged).
- Post-ssh: do **not** call `guard.resume()`. Return the SshResult to
  main.rs. main.rs translates Success → ExitCode::SUCCESS, ConnectFailed
  → ExitCode::from(N as u8), etc.

### Q3 — Viewport height heuristic
See §4.2. `term_h < 12` → manage fallback with stderr notice.

### Q4 — Filter UX
Inline = immediate filter (every printable key appends to query).
Manage = v0.3 `/`-prefix unchanged. Different by design.

### Q5 — Tests
- v0.3 suite (168 tests) must remain green. Manage mode key rebind
  changes 1-2 existing tests (`test_app_enter_connect` becomes
  `test_app_enter_opens_form` or equivalent).
- New: `inline_app` unit tests (~12), `tests/inline_test.rs` (~3),
  `tui/lifecycle` mode-aware unit tests (~3). Target: 185+ total.

### Q6 — Read-only Enter in manage
Read-only host (source_file ≠ sshc.conf): `Enter` falls through to
`AppAction::EditConfig` (same as `e`). status_message announces the
fallback: "external host — opening editor".

### Q7 — `r` in manage
Kept. Low cost, occasionally useful even in manage context.

## 9. Module responsibility boundaries

v0.3 R-G1..R-G8 carry over. New rule:

```bash
# R-G9: inline_app must not depend on probe, modal, forms, or storage
#       (inline is a lean read-only host browser by design)
grep -lE "crate::probe|crate::ui::modal|crate::ui::forms|crate::storage::with_locked_write|crate::storage::inject_include" \
  src/inline_app.rs 2>/dev/null && echo FAIL || echo PASS
```

`storage::sshc_conf_path` is allowed (read-only path helper).

## 10. Backward compatibility

- `~/.ssh/config` and `~/.ssh/config.d/sshc.conf` unchanged.
- `state.toml` unchanged (inline mode reads `last_connected_alias` for `r`).
- v0.3 users running `sshc` with no args see inline mode for the first
  time. Behaviour is intuitive enough that no migration note is needed
  beyond the CHANGELOG. README v0.4 leads with the inline example.
- v0.3 users wanting the old behaviour: `sshc -m` or `sshc --manage`.

## 11. Definition of Done (v0.4.0)

All MUST be true to tag v0.4.0:

- R0–R5 commits landed on master
- All ≥185 automated tests pass (release profile)
- R-G1..R-G9 regression greps clean
- `cargo clippy --all-targets -- -D warnings` 0 warnings
- `cargo fmt --check` clean
- v0.3 §6 manual checklist still green (manage mode unbroken)
- New v0.4 manual checklist (inline mode round-trip on macOS Terminal +
  iTerm2) run by a human
- Inline mode round-trip leaves shell prompt functional (no garbled
  escapes, no terminal lock)
- Panic during inline → shell still usable
- `Cargo.toml` `version = "0.4.0"`
- CHANGELOG.md updated
- README.md updated for v0.4 (inline example first)
- `git tag v0.4.0` applied
- GitHub release notes published

## 12. Out of scope for v0.4

- Inline CRUD/tag editing (requires alternate screen).
- Shell completion (zsh/bash function for `sshc <Tab>`) — v0.5.
- Config knob to make manage the default — v0.5.
- `-i` / `--inline` force flag — v0.5.
- Probes in inline mode — v0.5 (will require careful lifecycle: pool must
  exit fast on inline-mode exit).

## End of Brief.
