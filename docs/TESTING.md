# sshs Testing Guide

This document covers the test surface for `sshs` and the manual checklist
that must be run before tagging a release.

Scope:
- §1 Automated checks (every commit must pass)
- §2 Module-boundary regression greps
- §3 Manual test checklist (v0.2 baseline; every release)
- §4 mock_ssh fixtures (how the integration tests work)
- §5 v0.2.0 release readiness checklist
- §6 v0.3 manual checklist additions (host manager + probe + setup)
- §7 v0.3.0 release readiness checklist
- §8 v0.4 manual checklist additions (inline mode + manage rebind)
- §9 v0.4.0 release readiness checklist

---

## 1. Automated checks

Run all four from the repo root. All must succeed before pushing.

```bash
cargo build --release
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

### Test counts (as of v0.2.0)

| Suite | Count | Source |
|---|---|---|
| lib unit tests | 51 | `src/**/*.rs #[cfg(test)] mod tests` |
| parser integration | 11 | `tests/parser_test.rs` |
| general integration | 6 | `tests/integration_test.rs` |
| round-trip integration | 5 | `tests/round_trip_test.rs` |
| ignored (serial-only) | 1 | `tui::lifecycle::test_terminal_active_initial_false` |
| **total runnable** | **73** | |

### Test counts (as of v0.3.0)

| Suite | Count | Source |
|---|---|---|
| lib unit tests | 124 | `src/**/*.rs #[cfg(test)] mod tests` |
| general integration | 6 | `tests/integration_test.rs` |
| parser integration | 16 | `tests/parser_test.rs` |
| probe integration | 2 | `tests/probe_test.rs` |
| round-trip integration | 5 | `tests/round_trip_test.rs` |
| setup integration | 6 | `tests/setup_test.rs` |
| storage integration | 3 | `tests/storage_test.rs` |
| ignored (serial-only) | 2 | `tui::lifecycle::*` |
| **total runnable** | **162** | |

Probe tests bind a local listener and probe TEST-NET-1; expect ≤ 5s wall time. Run release-profile (`cargo test --release`) for realistic timing.

To run the ignored serial test (must run alone):

```bash
cargo test -- --ignored lifecycle
```

---

## 2. Module-boundary regression greps

These enforce the responsibility boundaries defined in `BRIEF_V2.md §9`.
A passing build is not sufficient — these must also be clean.

```bash
# R-G1: app.rs must not touch terminal state or spawn processes
grep -E "use crossterm::(terminal|execute|cursor)|std::process::Command" src/app.rs \
  && echo FAIL || echo PASS

# R-G2: exec/ssh.rs must not manipulate terminal state
grep -E "enable_raw_mode|EnterAlternateScreen|LeaveAlternateScreen|disable_raw_mode" \
  src/exec/ssh.rs && echo FAIL || echo PASS

# R-G3: tui/lifecycle.rs must not depend on the Host domain model
grep -E "use .*config::model" src/tui/lifecycle.rs && echo FAIL || echo PASS

# R-G4: main.rs must remain a thin bootstrap (≤ 80 non-comment lines)
test "$(grep -cvE '^\s*($|//)' src/main.rs)" -le 80 && echo PASS || echo FAIL

# R-G5: anyhow must be confined (currently zero usage anywhere)
grep -lE "anyhow::Result|anyhow::Error|use anyhow|anyhow!" \
  src/app.rs src/tui/*.rs src/exec/*.rs src/ui/*.rs src/error.rs src/main.rs \
  2>/dev/null && echo FAIL || echo PASS

# R-G6 (v0.3): storage/setup/probe/state modules must not touch TUI
grep -lE "use crossterm|use ratatui" \
  src/storage/*.rs src/setup/*.rs src/probe/*.rs src/state/*.rs \
  2>/dev/null && echo FAIL || echo PASS

# R-G7 (v0.3): probe must not depend on App or UI
grep -lE "crate::app|crate::ui" src/probe/*.rs 2>/dev/null && echo FAIL || echo PASS

# R-G8 (v0.3): ui/forms + ui/modal must not touch the filesystem or spawn processes
grep -lE "std::fs|std::process::Command" src/ui/forms/*.rs src/ui/modal.rs \
  2>/dev/null && echo FAIL || echo PASS

# R-G9 (v0.4): inline_app must remain a lean read-only browser
#               (no probe pool, no modal subsystem, no storage writers)
grep -lE "crate::probe|crate::ui::modal|crate::ui::forms|crate::storage::with_locked_write|crate::storage::inject_include" \
  src/inline_app.rs 2>/dev/null && echo FAIL || echo PASS
```

All nine must print `PASS`. If any prints `FAIL`, fix the underlying
violation rather than relaxing the rule.

---

## 3. Manual test checklist

Run on **both macOS Terminal.app and iTerm2** before tagging a release.
Linux gnome-terminal / kitty / alacritty optional but recommended.

Prerequisites:
- A `~/.ssh/config` with at least 3 non-wildcard hosts. At least one
  should resolve (use `localhost` if needed). At least one should fail
  to connect (bad hostname).
- A real ssh binary on `$PATH`.

### 3.1 Basic round-trip

- [ ] Run `cargo run --release` — TUI appears.
- [ ] Press `j`/`k` to navigate — selection moves.
- [ ] Press Enter on a reachable host — terminal switches out of TUI,
      ssh banner appears, you can interact.
- [ ] Type `exit` in ssh — ssh ends, TUI reappears.
- [ ] After return: **same host is still selected**.
- [ ] After return: that host has a yellow `★` prefix in the list.

### 3.2 Reconnect via `r`

- [ ] After a successful round-trip in §3.1, press `r` — reconnects to
      the same host (no need to navigate or press Enter).
- [ ] In a fresh `sshs` session (no prior connect), press `r` —
      status bar shows `"no recent host to reconnect"`. Wait 3s — the
      message clears.
- [ ] Press `/` to enter filter mode, type `r` — the letter is
      appended to the filter query, NO reconnect happens.

### 3.3 Failure modes

- [ ] Select a host with a bad hostname, Enter — ssh prints connection
      error, exits with code 255. TUI reappears. Status bar shows
      `Connection failed (255): {alias}` in bold yellow. Message clears
      after 3s.
- [ ] During an active ssh session, press Ctrl-C — ssh exits, TUI
      reappears, status bar is **silent** (no message).
- [ ] Edit `~/.ssh/config` to break a host's `HostName` to nonexistent.
      In sshs, press `e` on that host — `$EDITOR` opens at the right
      line. Save and quit. TUI reappears with the updated config.
- [ ] Set `EDITOR=/usr/bin/false` and press `e` — TUI reappears
      cleanly, log entry written; no crash.

### 3.4 Stability under repeated round-trips

- [ ] Connect → exit → connect → exit, 10 times in a row.
- [ ] After 10 trips: terminal cursor in the right place, scrollback
      intact, shell prompt color preserved, no garbled escape codes.
- [ ] Resize the terminal window during one of the round-trips.
      Next TUI draw uses the new size. No corruption.

### 3.5 Panic safety (debug-only)

Inject a `panic!("test")` inside `runtime::handle_connect` for this test.

- [ ] `cargo run` → enter TUI → trigger Connect → panic fires.
- [ ] Terminal returns to cooked mode automatically (you can type).
- [ ] Panic message appears on stderr.
- [ ] `stty -a` shows normal flags (no `-icanon`, no `-echo`).
- [ ] Cursor visible, prompt usable.

Revert the panic injection after verifying.

### 3.6 Edge cases

- [ ] Empty `~/.ssh/config` (or missing) — TUI starts with empty list,
      `q` exits cleanly.
- [ ] `Host *` wildcard-only entries — they do NOT appear in the list.
- [ ] Quoted `HostName "my host"` — value displayed without quotes.
- [ ] Inline comment `HostName a.com # prod` — comment stripped.
- [ ] `Match host *` block — content does NOT leak into preceding `Host`.

---

## 4. mock_ssh fixtures

The round-trip integration tests in `tests/round_trip_test.rs` exercise
the full spawn+wait+classify pipeline without depending on a real ssh
binary or thread-unsafe `env::set_var`.

Each test passes a fixture script path as `ssh_run`'s `ssh_binary` arg:

| Fixture | Behavior | Expected SshResult |
|---|---|---|
| `tests/fixtures/mock_ssh_exit_0.sh` | `exit 0` | `Success` |
| `tests/fixtures/mock_ssh_exit_130.sh` | `exit 130` | `Interrupted` |
| `tests/fixtures/mock_ssh_exit_255.sh` | `exit 255` | `ConnectFailed(255)` |
| `tests/fixtures/mock_ssh_signal.sh` | `kill -11 $$` | `Crashed(11)` |
| `(nonexistent path)` | spawn fails | `Err(SshError::LaunchFailed)` |

All four shell scripts have `chmod +x` applied. If git ever loses the
executable bit (e.g. on a Windows clone), restore via:

```bash
chmod +x tests/fixtures/mock_ssh_*.sh
```

Adding new mock scenarios: write a new POSIX shell script in
`tests/fixtures/`, `chmod +x`, and add a test case in
`tests/round_trip_test.rs`. Do NOT introduce `env::set_var` to vary
behavior — write separate scripts instead.

---

## 5. v0.2.0 release readiness checklist

All MUST be true to tag v0.2.0:

- [x] R1–R5 commits landed on master
- [x] All 73 automated tests pass
- [x] All 5 regression greps clean
- [x] `cargo clippy --all-targets -- -D warnings` 0 warnings
- [x] `cargo fmt --check` clean
- [x] `BRIEF_V2.md` and `PLAN_V0.2.md` checked in
- [ ] §3 manual checklist run by a human on macOS Terminal + iTerm2
- [ ] No panic during §3.4 (10 consecutive round-trips)
- [ ] `Cargo.toml` `version` bumped to `0.2.0`
- [ ] `git tag v0.2.0` applied
- [ ] User explicit approval to push the tag

The first 7 items are gates that the architect/CI can verify
mechanically. The last 3 are user-driven.

---

## 6. v0.3 manual checklist additions

v0.3 introduces the host manager flow (add/modify/delete/tags via
modal forms), probe glyph in the status column, source markers,
and the first-run Include injection prompt.

Prerequisites:
- `~/.ssh/config` exists (the first-run flow needs it). At least one
  unrelated `Host` block — used to verify the source marker.
- `~/.ssh/config.d/sshs.conf` may or may not exist; first-run sets it up.
- One reachable host (e.g. `localhost`) and one unreachable (e.g. a host
  whose `HostName` resolves to `192.0.2.1`).

### 6.1 First-run flow

- [ ] `mv ~/.ssh/config.d/sshs.conf{,.bak} 2>/dev/null` AND remove
      any prior `Include …/sshs.conf` line from `~/.ssh/config`.
      Then `rm ~/.config/sshs/state.toml 2>/dev/null` to simulate first run.
- [ ] `cargo run --release` — TUI starts with a centered confirmation
      modal: "sshs needs to add an Include line … Allow?".
- [ ] Press `n` — modal closes, status line shows `sshs.conf` is
      read-only in subsequent host operations. `~/.config/sshs/state.toml`
      now records `declined_include_injection = true`.
- [ ] Restart sshs. Confirmation modal does NOT reappear (state remembered).
- [ ] Restore: delete `state.toml`, rerun. Confirmation modal returns.
      This time press `y` — Include line added to `~/.ssh/config`,
      backup `.bak.sshs-YYYYMMDD` created, status: "Include added".

### 6.2 Add / modify / delete / tags

(Requires sshs.conf in writable state; complete §6.1 with `y` first.)

- [ ] Press `a` — Host form modal opens. Tab through 6 fields.
- [ ] Submit with `alias=test1`, `HostName=127.0.0.1` only. Modal closes,
      `test1` appears in the list. `~/.ssh/config.d/sshs.conf` was rewritten.
- [ ] Press `m` on `test1` — form opens pre-populated. Change port to
      `2222`. Submit. List + sshs.conf reflect the change.
- [ ] Press `t` on `test1` — tag form opens. Enter `prod, api`. Submit.
      List shows `[prod,api] test1` cyan prefix.
- [ ] Press `/`, type `@prod`, Enter — only `test1` (and any other tagged
      hosts with `prod` substring) remain visible.
- [ ] Press `d` on `test1` — Yes/No confirmation. Press `n` — host stays.
      Press `d` again, press `y` — host removed. sshs.conf updated.

### 6.3 External host (source marker)

- [ ] In `~/.ssh/config` (not sshs.conf), add a `Host external1` block.
- [ ] Restart sshs (Edit re-parses, but a clean restart picks it up
      regardless). `external1` shows a `·` marker in the Status column.
- [ ] Press `m` on `external1` — status shows "this host lives outside
      sshs.conf; press 'e' to edit source". Form does NOT open.
- [ ] Press `d` on `external1` — status shows "can only delete sshs.conf
      hosts". No confirmation modal.
- [ ] Press `e` on `external1` — `$EDITOR` opens at the right line.

### 6.4 Probe glyph

- [ ] In sshs.conf, ensure one reachable host (`HostName 127.0.0.1`,
      `Port` = a locally-bound port) and one unreachable
      (`HostName 192.0.2.1`).
- [ ] At startup the reachable row's Status glyph eventually settles
      (T11 wires probe results; placeholder space is acceptable until
      the probe completes). The unreachable row stays unsettled longer.
- [ ] Add a host via `a` — new row gets an Unknown initial state, then
      transitions after the next probe round refreshes.

### 6.5 Narrow terminal

- [ ] Shrink the terminal to ~50 columns. Account column hides first.
- [ ] Shrink to ~35 columns. Port column hides next.
- [ ] Shrink to ~28 columns. Host column hides; Alias + Status remain.
- [ ] Shrink below 60x10. Screen shows "terminal too small (≥60x10)".

### 6.6 Help modal

- [ ] Press `?` — Info modal lists the v0.3 keys
      (a / d / m / t / e / ? / q). Enter or Esc dismisses.

---

## 7. v0.3.0 release readiness checklist

All MUST be true to tag v0.3.0:

- [ ] R0–R9 commits landed on master
- [ ] All 162 automated tests pass (release profile)
- [ ] R-G1..R-G8 regression greps clean
- [ ] `cargo clippy --all-targets -- -D warnings` 0 warnings
- [ ] `cargo fmt --check` clean
- [ ] §3 v0.2 manual checklist still green (regression)
- [ ] §6 v0.3 manual checklist run by a human on macOS Terminal + iTerm2
- [ ] First-run flow tested with a clean `state.toml`
- [ ] Include injection backup file (`.bak.sshs-YYYYMMDD`) verified
- [ ] `Cargo.toml` `version` bumped to `0.3.0`
- [ ] CHANGELOG.md updated
- [ ] README.md updated for v0.3
- [ ] `git tag v0.3.0` applied
- [ ] User explicit approval to push the tag
- [ ] GitHub release notes published

---

## 8. v0.4 manual checklist additions

Inline mode introduces a new terminal lifecycle (no alternate screen).
These exercise the round-trip + panic + fallback behaviour that
TestBackend cannot simulate.

Prerequisites:
- One reachable ssh host (e.g. `localhost` or any host in your
  `~/.ssh/config`).
- One unreachable host (any alias with a bad `HostName`).
- shell prompt visible at the start of each test (no stale TUI output).

### 8.1 Inline first-run

- [ ] `cargo run --release` from a shell prompt.
- [ ] An **inline viewport** opens BELOW the prompt — NOT a full-screen
      alternate buffer. ~15 lines reserved.
- [ ] Typing immediately filters (`web` → only web-prefix hosts).
- [ ] `Esc` clears the filter; `Esc` again exits to the shell.
- [ ] After exit: shell prompt functional. `stty -a | grep -E "icanon|echo"`
      shows BOTH enabled (no `-icanon`, no `-echo`).

### 8.2 Inline ssh round-trip

- [ ] Run `cargo run --release`, select a reachable host with `↑/↓`,
      press `Enter`.
- [ ] Viewport clears, raw mode releases, cursor returns. ssh banner
      appears in normal shell flow.
- [ ] Run something (`hostname`, `ls`), then `exit`.
- [ ] After ssh exits: **shell prompt returns directly** — the inline
      TUI does NOT re-open.
- [ ] `echo $?` reflects ssh's exit code (0 for clean disconnect).

### 8.3 Inline ssh failure exit code

- [ ] Run `cargo run --release` and connect to the unreachable host.
- [ ] ssh prints connection error, exits with code 255.
- [ ] `echo $?` shows `255` (or low byte of the ssh code).

### 8.4 Inline `r` reconnect

- [ ] After §8.2 left `last_connected_alias` saved in state.toml, run
      `cargo run --release` again.
- [ ] Without typing anything, press `r` → ssh spawns against the
      previous host.
- [ ] If state.toml is fresh and no prior connect: `r` is a silent no-op
      (filter stays empty).

### 8.5 Inline Ctrl-C

- [ ] Run `cargo run --release`, type `abc` into the filter, press Ctrl-C.
- [ ] Returns to shell prompt without spawning ssh. `stty -a` clean.

### 8.6 Inline panic restoration

- [ ] Temporarily inject a `panic!()` into `sshs::run::inline` (after
      the TerminalGuard is acquired). Build + run.
- [ ] Panic message reaches stderr; shell prompt remains usable.
- [ ] Revert the panic injection.

### 8.7 Terminal-too-small fallback

- [ ] Resize the terminal to < 12 rows tall.
- [ ] `cargo run --release` → stderr prints `terminal too small for
      inline mode; falling back to --manage` and the alternate-screen
      TUI opens instead.

### 8.8 Manage rebind (`-m` flag)

- [ ] `cargo run --release -- -m` → alternate-screen TUI opens.
- [ ] On a sshs.conf-managed host, press `Enter` → the modify form
      opens (Modal/Form mode). `Esc` cancels.
- [ ] On an external-source host (in `~/.ssh/config` directly, not
      sshs.conf), press `Enter` → `$EDITOR` opens at the host's line.
- [ ] Press `s` on the selected host → ssh round-trip (alternate screen
      suspended, ssh runs, on exit the TUI resumes).
- [ ] Press `m` → no action (key unbound in v0.4).
- [ ] Press `?` → help modal shows `Enter open  s ssh` (NOT `Enter ssh`).
- [ ] `a / d / t / e / r / q` all still work as in v0.3.

### 8.9 `--manage` long form

- [ ] `cargo run --release -- --manage` → same as `-m`.

---

## 9. v0.4.0 release readiness checklist

All MUST be true to tag v0.4.0:

- [ ] R0–R6 commits landed on master
- [ ] All 190 automated tests pass (release profile)
- [ ] R-G1..R-G9 regression greps clean
- [ ] `cargo clippy --all-targets -- -D warnings` 0 warnings
- [ ] `cargo fmt --check` clean
- [ ] §3 v0.2 manual checklist still green (manage mode regression)
- [ ] §6 v0.3 manual checklist still green (host manager regression)
- [ ] §8 v0.4 manual checklist signed off (inline + rebind)
- [ ] `examples/inline_prototype.rs` runs cleanly + SSHS_PROTOTYPE_PANIC
      restores the shell
- [ ] `Cargo.toml` `version` bumped to `0.4.0`
- [ ] CHANGELOG.md updated
- [ ] README.md updated for v0.4 (inline-first)
- [ ] `git tag v0.4.0` applied
- [ ] User explicit approval to push the tag
- [ ] GitHub release notes published

---

## End of Testing Guide.
