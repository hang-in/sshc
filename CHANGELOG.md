# Changelog

All notable changes to **sshc** are recorded here. The format roughly
follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

Nothing yet.

## [0.4.1] — 2026-05-20

### Changed

- **Renamed binary, package, and on-disk paths from `sshs` to `sshc`.**
  The previous name (v0.1.0–v0.4.0) collided with an unrelated CLI also
  called `sshs`. The Cargo package, binary, UI strings, file paths
  (`~/.ssh/config.d/sshc.conf`, `~/.config/sshc/state.toml`), and the
  `Include` backup filename suffix all now use `sshc`. The state.toml
  schema is unchanged — only the parent directory moved.
- Replaced two misleading `StorageError::LockHeldByOther` fallbacks in
  `src/app.rs` flagged in PR review:
  - `apply_add` / `apply_modify` now `.expect(...)` on
    `host_from_payload` since the caller already matched the Host
    variant — the previous `None` branch was unreachable.
  - `persist_sshc_conf` returns
    `AppError::Setup(SetupError::HomeDirMissing)` when `dirs::home_dir`
    is unresolvable, instead of disguising it as lock contention.

### Added

- **Homebrew distribution via [`hang-in/homebrew-tap`](https://github.com/hang-in/homebrew-tap)**.
  `brew install hang-in/tap/sshc` installs the latest release as a
  pre-built binary.
- **GitHub Actions `release.yml`**: tag push (`v*`) triggers cross-
  compiled binaries for `{x86_64,aarch64}-{apple-darwin,unknown-linux-gnu}`,
  stripped, packed as `sshc-<target>.tar.gz`, attached to the GitHub
  release.
- **GitHub Actions `bump-homebrew.yml`**: `release.published` event
  triggers a PR on the tap repo updating `Formula/sshc.rb`.
- `README.ko.md` (Korean translation, content parity with `README.md`).
- README rewritten in a leaner shape: tagline + brew badge + demo
  placeholder + Why / Quickstart / Two modes / Keybindings / Install
  / Configuration / Comparison.

## [0.4.0] — 2026-05-20

### Added

- **Inline mode (`sshc`, no args)**. Default command opens an
  `ratatui::Viewport::Inline(N)` host browser BELOW the shell prompt
  instead of an alternate screen. Type to filter (immediate, fzf-style),
  `↑/↓` or `j/k` to navigate, `Enter` to ssh, `Esc`/`Ctrl+C` to cancel,
  `r` to reconnect to the last alias. Viewport height is
  `(terminal_height − 5).clamp(8, 15)`; below 12 rows the binary falls
  back to manage mode with a one-line stderr notice.
- **Manage mode (`sshc -m` / `sshc --manage`)**. The v0.3 alternate-
  screen TUI, retained behind a flag. Default command behaviour changed
  in v0.4 — this is intentionally breaking for a single-user tool.
- **`InlineApp`** — lean read-only host browser (no probes, no modal
  subsystem, no forms, no storage writes). 144 lines, 13 unit tests.
- **`tui::inline_runtime`** — `run_event_loop_inline` +
  `handle_connect_inline`. Inline mode tears down the viewport before
  ssh spawn and never re-enters the UI on ssh exit; the binary exits
  with an `SshResult`-derived `ExitCode` so failures propagate to the
  parent shell.
- **`ScreenMode { Alternate, Inline(u16) }`** on `TerminalGuard`. The
  panic hook tracks `RAW_ACTIVE` and `ALT_ACTIVE` independently, so a
  panic in inline mode does NOT emit `LeaveAlternateScreen` (would
  corrupt a normal-mode terminal).
- **R-G9** boundary gate — `inline_app` cannot import `probe`,
  `ui::modal`, `ui::forms`, or storage writers.
- **`src/run.rs`** — `inline()` / `manage()` dispatch helpers. Keeps
  `main.rs` thin (41 non-comment lines, R-G4 ≤ 80).
- **`examples/inline_prototype.rs`** — standalone ratatui Viewport
  smoke test for manual verification. Useful for terminal compat
  triage.

### Changed

- **Manage-mode key rebind**:
  - `Enter` opens the modify form for sshc.conf-managed hosts; falls
    through to `AppAction::EditConfig` (`$EDITOR` jump) for external
    hosts. Old "Enter = ssh" semantics moved to `s`.
  - `s` — ssh connect for the selected host.
  - `m` — removed. (Was previously "open modify form"; merged into
    `Enter`.)
  - Help modal text updated.
- **CLI dispatch**: `main()` returns `ExitCode` (via Termination) so
  ssh failure codes propagate to the parent shell. Inline `Quit` →
  `SUCCESS`, `Connect/Reconnect` → low byte of the ssh result code,
  `Crashed/UnknownTermination` → `FAILURE`.

### Internal

- `TerminalGuard` no longer holds a single `TERMINAL_ACTIVE` flag;
  split into `RAW_ACTIVE` + `ALT_ACTIVE` atomics so mode-specific
  enter/leave is idempotent.
- Inline viewport is `terminal.clear()`-ed before ssh spawn so the
  shell sees no frozen frame (fzf-style clean exit).

### Tests

- Total: 162 (v0.3.0) → **190** (v0.4.0).
- New: `inline_app` 13 unit + `tests/inline_test.rs` 5 integration +
  `ScreenMode` equality + manage-rebind 3 (`s` connects, Enter on
  external opens editor, Enter on managed opens form, `m` unbound).
  Old `test_app_enter_connect` renamed to `test_app_s_connects`.

### Compatibility

- `~/.ssh/config`, `~/.ssh/config.d/sshc.conf`, `state.toml` unchanged.
- v0.3 users running `sshc` will land in inline mode on first launch.
  The host list looks similar; selection and `Enter` ssh-connect work
  as expected. To get the v0.3 behaviour back, use `sshc -m`.
- First-run setup flow (`Include` injection) runs in **manage mode
  only**. Inline mode reads whatever hosts are already visible to
  `~/.ssh/config`; users who never run manage mode will not see the
  setup prompt and inline still works (sshc.conf is simply absent).

## [0.3.0] — 2026-05-20

### Added

- **Host manager (in-TUI add / modify / delete)**. New keys `a`, `m`,
  `d` open modal forms backed by a dedicated `~/.ssh/config.d/sshc.conf`
  file. All writes are atomic (tempfile + rename) under a POSIX
  `LOCK_EX` so concurrent sshc instances don't corrupt the file.
- **Tags**. New `t` key edits per-host tags; tags are stored as a
  `# @tags: a, b` comment immediately above each `Host` block. Filter
  with `@<tag>` (e.g. `@prod`) or rely on the default fuzzy filter,
  which also matches tag content as a fallback.
- **First-run setup**. On first launch sshc offers to add an
  `Include ~/.ssh/config.d/sshc.conf` line to `~/.ssh/config`, with a
  dated `.bak.sshc-YYYYMMDD` backup. Decision persisted to
  `~/.config/sshc/state.toml`.
- **Probe column**. A background thread pool issues parallel TCP
  connect probes (≤ 8 workers, 1s timeout) and surfaces results in the
  Status column. Generation guard discards stale updates after refresh.
- **Source-aware UI**. Hosts that live outside `sshc.conf` are marked
  `·` and protected from the in-TUI add/modify/delete flow; press `e`
  to jump to the source file in `$EDITOR` instead.
- **5-column responsive table**. Alias | Account | Host | Port | Status
  with priority-based column hiding for narrow widths (Account first,
  then Port, then Host). Below 60×10 shows a "terminal too small"
  notice rather than rendering a broken layout.
- **Help modal**. `?` key opens a key reference in an info modal.
- **Modal subsystem**. Generic `ModalKind { Confirmation, Info, Form }`
  with a `FormState` trait that owns its own per-key state machine.
  Tab/Shift+Tab navigation, Enter submit-or-advance, Esc cancel, and
  Ctrl+U clear are standard across all forms.
- **Integration tests**. 11 new tests across `tests/storage_test.rs`,
  `tests/probe_test.rs`, and `tests/setup_test.rs`. Total runnable
  tests grew from 73 (v0.2.0) to 162.
- **Module-boundary gates** R-G6, R-G7, R-G8 enforced via grep:
  `storage`/`setup`/`probe`/`state` cannot import TUI crates;
  `probe` cannot depend on `app` or `ui`; `ui/forms` and `ui/modal`
  cannot touch the filesystem or spawn processes.

### Changed

- `App` gains `mode: AppMode { List, Modal(ModalKind) }`,
  `probe_states: Vec<ProbeState>`, `state: state::State`. `handle_key`
  dispatches to the modal handler whenever `mode` is not `List`.
- `AppAction` grew `SaveState`, `InjectInclude`, `DeclineInclude`.
  Form submit handlers emit `SaveState`, which the runtime turns into
  a `state::save()` + `ProbePool::refresh()`.
- `ui/list.rs` swaps from a `List<Line>` to a ratatui `Table` with
  per-row cells, so column widths can be driven by `Constraint`.
- `runtime::run_event_loop` now takes `&ProbePool` and drains
  `poll_updates()` before each draw, so probe state changes appear
  with ≤ one tick of latency.
- `main.rs` orchestrates first-run setup and routes the new
  AppActions; it stays under 100 LOC.

### Internal

- New modules: `state/*`, `setup/*`, `storage/*`, `probe/*`,
  `ui/modal.rs`, `ui/forms/*`, `config/tags.rs`.
- Error taxonomy: `StorageError`, `SetupError`, `ProbeError` join
  `SshError`, `TerminalError`, `EditorError` under `AppError` with
  `From` impls.

### Compatibility

- Existing `~/.ssh/config` continues to work unchanged. v0.3 only
  introduces a new file (`~/.ssh/config.d/sshc.conf`) and an optional
  one-line `Include` directive in the main config.
- v0.2 binary upgrade: on first launch the setup modal will offer the
  Include line. Declining keeps sshc read-only — the host browser
  still works, but `a`/`m`/`d`/`t` show a "read-only" status.

## [0.2.0] — 2026-05-19

### Added

- Round-trip ssh session: `Enter` spawns ssh as a child, sshc suspends
  raw mode + alt screen, resumes when ssh exits, and shows a transient
  status message classifying the exit (success / interrupted /
  ConnectFailed / Failed / Crashed / UnknownTermination).
- `★` marker on the last-connected host and `r` reconnect shortcut.
- Status bar with auto-dismissing messages (3 s timeout).
- `TerminalGuard` (RAII raw-mode + alt-screen) and a panic hook that
  always restores the terminal before unwinding.
- Round-trip integration tests using mock_ssh shell fixtures (no real
  ssh binary needed).
- Module-boundary gates R-G1..R-G5 documented in `docs/TESTING.md`.

### Changed

- `App` state cleaned up: removed transitional `should_quit` /
  `should_connect` / `should_edit` flags in favor of
  `pending_action: Option<AppAction>`.

## [0.1.0] — 2026-05-19 (initial)

### Added

- Minimal TUI listing non-wildcard `Host` entries from `~/.ssh/config`.
- Fuzzy filter (nucleo) by alias or hostname.
- `Enter` to ssh, `e` to open `$EDITOR` at the host's line.
- `Include` directive support with circular detection and depth limit.
- Handles missing `~/.ssh/config` gracefully (empty list).
