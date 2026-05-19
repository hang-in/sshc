# Changelog

All notable changes to **sshs** are recorded here. The format roughly
follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

Nothing yet.

## [0.3.0] — 2026-05-20

### Added

- **Host manager (in-TUI add / modify / delete)**. New keys `a`, `m`,
  `d` open modal forms backed by a dedicated `~/.ssh/config.d/sshs.conf`
  file. All writes are atomic (tempfile + rename) under a POSIX
  `LOCK_EX` so concurrent sshs instances don't corrupt the file.
- **Tags**. New `t` key edits per-host tags; tags are stored as a
  `# @tags: a, b` comment immediately above each `Host` block. Filter
  with `@<tag>` (e.g. `@prod`) or rely on the default fuzzy filter,
  which also matches tag content as a fallback.
- **First-run setup**. On first launch sshs offers to add an
  `Include ~/.ssh/config.d/sshs.conf` line to `~/.ssh/config`, with a
  dated `.bak.sshs-YYYYMMDD` backup. Decision persisted to
  `~/.config/sshs/state.toml`.
- **Probe column**. A background thread pool issues parallel TCP
  connect probes (≤ 8 workers, 1s timeout) and surfaces results in the
  Status column. Generation guard discards stale updates after refresh.
- **Source-aware UI**. Hosts that live outside `sshs.conf` are marked
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
  introduces a new file (`~/.ssh/config.d/sshs.conf`) and an optional
  one-line `Include` directive in the main config.
- v0.2 binary upgrade: on first launch the setup modal will offer the
  Include line. Declining keeps sshs read-only — the host browser
  still works, but `a`/`m`/`d`/`t` show a "read-only" status.

## [0.2.0] — 2026-05-19

### Added

- Round-trip ssh session: `Enter` spawns ssh as a child, sshs suspends
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
