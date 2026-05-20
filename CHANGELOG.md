# Changelog

All notable changes to **sshc** are recorded here. The format roughly
follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

Nothing yet.

## [0.7.0] — 2026-05-20

Platform expansion: native Windows support. No new features for
existing macOS / Linux users — daily behavior is unchanged.

### Added

- **Native Windows builds** (x86_64-pc-windows-msvc). cargo-dist now
  produces `sshc-x86_64-pc-windows-msvc.zip` alongside the existing
  macOS / Linux artifacts, plus a `powershell` installer (`irm | iex`
  one-liner) as the Windows analog of the `shell` installer.
- **`windows-sys 0.59`** picked up as a target-gated dependency for
  the LockFileEx-based lock path.

### Changed

- **File locking** (`storage/with_locked_write`) factored into a
  small `try_lock_exclusive` helper, cfg-split:
  - Unix: `nix::fcntl::flock(LOCK_EX | LOCK_NB)` — unchanged.
  - Windows: `LockFileEx(LOCKFILE_EXCLUSIVE_LOCK |
    LOCKFILE_FAIL_IMMEDIATELY)` over the whole file.
    `ERROR_LOCK_VIOLATION` maps to `StorageError::LockHeldByOther`
    so the caller-facing semantics match the Unix path.
- **File permissions** (`setup::ensure_file_mode`, doctor's `~/.ssh`
  check) wrap their Unix-mode logic in `#[cfg(unix)]`. The Windows
  arm is a no-op (or, in doctor, a `PASS` line annotated "ACL not
  checked"). Windows ACL enforcement is explicitly deferred to v0.8+.
- **`$EDITOR` fallback**: when the env var is unset, default to
  `notepad.exe` on Windows instead of `vi`.
- **`SSH_AUTH_SOCK` doctor check**: on Windows, missing
  `SSH_AUTH_SOCK` is `PASS` with the note "not applicable on Windows
  (use Windows OpenSSH agent or Pageant)" — the env var is the wrong
  signal there. Unix behavior unchanged.
- `nix` moved under `[target.'cfg(unix)'.dependencies]`, so Windows
  builds see zero transitive Unix-only deps.

### Internal

- Unix-only integration tests (`tests/setup_test.rs`,
  `tests/storage_test.rs`, `tests/round_trip_test.rs`) and the
  `src/exec/ssh.rs::tests` module gated with `#![cfg(unix)]`.
- `cargo check --target x86_64-pc-windows-msvc` clean.
- `cargo clippy --all-targets --target x86_64-pc-windows-msvc -- -D
  warnings` clean.
- `cargo clippy --all-targets -- -D warnings` (host) clean.
- All 162 + 38 integration tests still green on Unix.
- R-G1..R-G9 still clean. main.rs still 8 LOC.

### Out of scope (deferred to v0.8+)

- Windows ARM64 (`aarch64-pc-windows-msvc`) — cargo-dist target add
  is trivial but there's no runner to exercise the binary yet.
- Windows ACL enforcement of "private key files must be private".
- Pageant / Windows OpenSSH agent socket discovery.

## [0.6.0] — 2026-05-20

Picker depth + edit-safety pass. Two threads:

### Added — every-day picker

- **Favorites / pin (`f` in manage mode).** Toggles a host as
  pinned. Pinned hosts float to the top of the picker regardless of
  fuzzy score, in both inline and manage. Stored in `state.toml`
  under a new `[memory] favorites` list (separate from tags).
- **Recent-connection history.** `last_connected_alias` (single
  `String`) bumps to `recent: Vec<RecentEntry>` (max 20, most-
  recent first). Picker uses recency as the 2nd-tier sort key,
  after favorites, before fuzzy score. Loading a pre-v0.6
  `state.toml` migrates transparently: the legacy alias becomes
  `recent[0]` with `ts = state.toml mtime`.
- **Inline-mode one-line summary** under the host table:
  `→ user@hostname:port` for the highlighted row.
- **Manage-mode right-side preview panel** with HostName / User /
  Port / Identity / Tags / Extra. Visible when terminal width ≥
  100 cols; hidden gracefully on narrower terminals.
- **★ glyphs.** Yellow ★ = favorite. Cyan ★ = last-connected. Both
  shown in inline picker and manage status column.

### Added — safe management

- **`v` in manage mode runs `ssh -G <alias>`** and shows the parsed
  effective config in an Info modal. Cached per session; cache is
  cleared on any successful form submit / delete / tag edit. Falls
  back to a status-bar warning if `ssh` is missing or exits
  non-zero — never blocks the user.

### Changed — inline picker is now modal

Inline switched from "fzf-style: every char filters" to explicit
modes:

- Nav mode (default): `j/k/↑/↓` navigate, Enter `ssh`, `/` enter
  search mode, `q` / Esc quit, Ctrl+C quit anywhere.
- Search mode: printable chars append, Backspace pops, Esc exits
  search (picker stays open), Enter ssh-launches the highlight.

The previous fzf shortcut for "type to filter immediately" was
trading away the `j/k` navigation key once any character had been
typed, which surprised the user. Modal aligns inline with manage
mode (both use `/`).

### Removed

- **`r` reconnect key (both inline and manage).** The R3 recency
  sort already puts the last-connected host at row 0, so a single
  `s` (manage) or Enter (inline) covers the reconnect case. The
  dedicated key was redundant once history landed.

### Internal

- `src/ui/preview.rs` — new widget module.
- `src/exec/ssh_config.rs` — `validate_alias` helper, with
  `ValidationError { SshNotFound, NonZeroExit }`. Lives in
  `src/exec/` so R-G1 (no `Command::new` in `src/app/*`) stays clean.
- `App.validation_cache: HashMap<String, String>` for ssh -G.
- `State::record_recent(alias)` central helper used by inline /
  manage / direct connect paths.
- `tests/fixtures/state_v05.toml` + migration tests guard the
  schema bump against regressions.
- 159 → 162 unit + integration tests.
- R-G1..R-G9 still clean; main.rs at 8 non-comment lines.

## [0.5.1] — 2026-05-20

Tiny patch — adds a read-only environment check. Nothing else changes.

### Added

- **`sshc --doctor`** prints a six-line report:
  - `~/.ssh/config` exists
  - `~/.ssh` directory mode (expects 0700)
  - `~/.ssh/config.d/sshc.conf` exists
  - `Include` line present in `~/.ssh/config`
  - `ssh` binary on PATH (shows the OpenSSH version banner)
  - `SSH_AUTH_SOCK` environment variable

  Report-only — no files are modified. Exit code is 0 unless any check
  is `FAIL`; `WARN` does not fail the run (e.g. a missing
  `SSH_AUTH_SOCK` is a heads-up, not an error).

## [0.5.0] — 2026-05-20

Refactor lands: `src/app/mod.rs` (944 LOC pre-split) is now broken into
five focused sub-modules. One small user-facing feature comes with it.

### Added

- **`sshc <alias>` direct-connect.** A positional alias on the command
  line skips the TUI entirely, looks the alias up in your parsed
  config, and runs `ssh <alias>` in the inherited terminal. Designed
  for shell aliases and scripts that already know which host they
  want. Unknown alias prints to stderr and exits 1 without invoking
  ssh. `state.last_connected_alias` is updated on a launch attempt
  (matches inline/manage).
- **`src/cli.rs`** picks up the entire dispatch path
  (`parse_mode`, `print_help`, `print_version`, positional handling).
  `main.rs` is now 8 non-comment lines.

### Changed

- **`src/app/mod.rs` split** into thematic sub-modules:
  - `src/app/input.rs` — `handle_key`, `handle_list_key`,
    `handle_modal_key`, `dispatch_modal_action`, `activate_selected`.
  - `src/app/forms.rs` — `open_*_form`, `open_help_modal`, `apply_form`,
    `apply_add` / `apply_modify` / `apply_delete` / `apply_tags`,
    `persist_sshc_conf`, plus the new `build_host` and
    `normalized_tags` helpers.
  - `src/app/tests.rs` — the entire `#[cfg(test)] mod tests` block.
  - `src/app/filter.rs` was already extracted in v0.4.3.
  - `mod.rs` shrinks from 1040 → 246 lines and keeps only the `App`
    struct, the public enums, constructors, navigation, accessors,
    and the SSH lifecycle hooks (`try_reconnect`, `on_ssh_finished`,
    `replace_hosts`, `apply_probe_updates`).
- **`apply_add` / `apply_modify` no longer call
  `host_from_payload(...).expect(...)`.** `apply_form` destructures
  `FormPayload::Host` inline and calls a new `build_host` helper that
  returns an already-built `Host`. Closes the deepseek-v4-pro review
  item flagging the unreachable `expect`.
- **`normalized_tags(csv: &str) -> Vec<String>`** consolidates the
  `split(',') → filter_map(normalize_tag) → dedup` chain that was
  previously duplicated in `apply_tags` and the now-removed
  `host_from_payload`.

### Internal

- All 147 existing unit tests still pass; integration tests untouched.
- R-G1..R-G9 module-boundary greps still clean.
- `main.rs` is well under the R-G4 80-line bootstrap cap (8 lines).
- No file in `src/app/` exceeds ~290 non-comment lines.
- `clippy --all-targets -- -D warnings` clean.
- `fmt --check` clean.

## [0.4.3] — 2026-05-20

Refactor + small fixes pass before v0.5 starts adding new features. No
behaviour change for the user; internals only.

### Changed

- **`apply_filter` extracted into `src/app/filter.rs`** as the first
  step of breaking `src/app.rs` (~944 LOC) into thematic sub-modules.
  Full split (input.rs + forms.rs) is planned for v0.5.0 under a
  proper BRIEF/PLAN since the visibility surgery is non-trivial. For
  v0.4.3 only the filter logic moves — `src/app/mod.rs` shrinks
  slightly and `filter.rs` declares a single `impl super::App` block
  with the relocated method.
- **`sshc_conf_path` now cached on `App`** (`Option<PathBuf>`)
  instead of called via the throwaway associated helper. The
  previous `unwrap_or_default()` produced an empty `PathBuf` sentinel
  when home-dir resolution failed; in that edge case any host whose
  `source_file` also happened to be empty would have falsely matched
  as "sshc.conf-managed". The new cache is `None` in that scenario
  and the comparison helper documents the intended semantics.
  (Reviewer: gemini-code-assist + deepseek-v4-pro@ollama-cloud.)

### Internal

- `cargo-dist` publish-homebrew-formula now self-serves with
  `HOMEBREW_TAP_TOKEN` configured (v0.4.2 needed a manual tap push).

## [0.4.2] — 2026-05-20

### Added

- **`--help` / `-h`** and **`--version` / `-V`** flags. Help text covers
  the inline / manage split, keys, and on-disk files. Version comes from
  `CARGO_PKG_VERSION`, so the Homebrew formula's smoke test can call
  `sshc --version` instead of just checking the binary exists.
- **Modal overlay rendering**. v0.4.0/0.4.1 had a bug where
  `ui/mod.rs::render` only drew the host table; an active `ModalKind`
  (Confirmation/Info/Form) was never painted. First-run users saw the
  host list and thought "no key works except Esc" — but Esc was
  actually triggering the modal's on-no path (decline_include). Modal
  is now overlaid via `Clear` + chrome + body.
- **Manage `i` key**: force-retry the Include injection. Useful when
  the user previously declined first-run setup, the Include line was
  removed by hand, or `state.toml` got into a stale state. Flips
  `declined_include_injection = false` and emits
  `AppAction::InjectInclude`.
- **`Host.extra: Vec<String>`** — freeform SSH directives (ProxyJump,
  ForwardAgent, LocalForward, …) preserved across read/write
  round-trips. Parser pushes unknown lines inside a Host block into
  `extra`; serializer emits them with the standard 4-space indent
  after the typed fields.
- **HostForm "Options (a; b)" field** (7th field). Semicolon-
  separated entry: each `KeyValue` becomes one extra line. Tab
  wraparound updated to 7 fields.

### Changed

- **Renamed `sshs` → `sshc`** across the codebase (folded in from
  v0.4.1 — name collision with an unrelated CLI). Binary, package,
  on-disk paths (`~/.ssh/config.d/sshc.conf`, `~/.config/sshc/...`),
  UI strings, docs, file backup suffix. The state.toml schema is
  unchanged; only its parent directory moved.
- **Tags column moved off Alias into a dedicated right-side column**
  with `show_tags` visibility (hides first as panels narrow). Alias
  cells now always start at column 0, so vertical scanning works.
- **Inline mode layout**: no border, left-aligned, width sized to the
  data (no longer wastes the full terminal width on a sparse table).
  Status bar marker `/` → `▸` since the user never typed `/` to start
  filtering — fzf semantics.
- **Inline viewport height** is now `(host_count + 3).clamp(5,
  viewport_height)` instead of a fixed 15 rows. Avoids reserving
  blank rows and pushing the shell prompt far up the scrollback.
- **Read-only status messages are actionable**: all `a/m/d/t`
  read-only branches now suggest `press 'i' to add Include line`.
- **Manage Enter key**: opens the modify form for sshc.conf-managed
  hosts; falls through to `$EDITOR` for external hosts. `s` is now
  the ssh-connect shortcut; `m` is unbound (already in v0.4.0/0.4.1
  but reiterated here for the `i` help-text update).
- **CI/CD**: replaced the handcrafted `release.yml` +
  `bump-homebrew.yml` pair with **cargo-dist v0.31.0**. Single
  `dist-workspace.toml` drives cross-compile + tarballs + GitHub
  release + Homebrew tap formula push. The `release.published`
  event from `GITHUB_TOKEN`-created releases didn't trigger the
  downstream `bump-homebrew.yml` (known constraint); cargo-dist
  sidesteps that with a single pipeline.
- **Misleading error mappings fixed** (PR review feedback,
  gemini-code-assist): `apply_add` / `apply_modify` now
  `.expect()` the unreachable `host_from_payload` `None` branch;
  `persist_sshc_conf` reports
  `AppError::Setup(SetupError::HomeDirMissing)` instead of
  disguising it as lock contention.

### Internal

- `Cargo.toml` gains `repository`, `homepage`, `readme`,
  `[profile.dist]`.
- `dist-workspace.toml` (new) — cargo-dist config.
- `examples/render_preview.rs` + tests updated for the new
  `Host.extra` field and the tag column move.
- `docs/demos/` (new) — fixture ssh config + fake ssh wrapper +
  vhs tapes for layout previews.

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
