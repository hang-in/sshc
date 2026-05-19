# sshs Testing Guide

This document covers the test surface for `sshs` and the manual checklist
that must be run before tagging a release.

Scope:
- §1 Automated checks (every commit must pass)
- §2 Module-boundary regression greps
- §3 Manual test checklist (every release)
- §4 mock_ssh fixtures (how the integration tests work)
- §5 v0.2.0 release readiness checklist

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
```

All five must print `PASS`. If any prints `FAIL`, fix the underlying
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

## End of Testing Guide.
