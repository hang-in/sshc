# sshc

A terminal user interface for browsing, connecting to, and managing the
hosts defined in your SSH configuration. Reads `~/.ssh/config` plus any
files it `Include`s, and (in manage mode) writes new hosts to a separate
`~/.ssh/config.d/sshc.conf` so your hand-written config stays untouched.

> 한국어 문서: [README.ko.md](README.ko.md)

- **Status**: v0.4.0 — Linux + macOS, MIT.
- **Stack**: Rust, ratatui 0.29, crossterm 0.28, nucleo 0.5, nix flock,
  serde/toml.
- **Test surface**: 190 automated tests (147 lib units + 43 integration).

## Install

### Homebrew (macOS / Linux)

```sh
brew install hang-in/tap/sshc
```

`brew upgrade sshc` picks up new releases once the tap formula is bumped.

### From source

```sh
git clone https://github.com/hang-in/sshc
cd sshc
cargo install --path .
```

Requires a Rust 1.85+ toolchain. The release profile binary lives at
`target/release/sshc` after `cargo build --release`.

## Demo

<!-- Insert a recorded GIF here once the brew install path is live. -->
<!-- Suggested capture: `brew install hang-in/tap/sshc` → `sshc` (inline -->
<!-- mode) → fuzzy filter → Enter → ssh session → exit → shell prompt.   -->

## Two modes

`sshc` runs in one of two modes selected by a command-line flag:

| Command | Mode | Terminal | Purpose |
|---|---|---|---|
| `sshc` | **Inline** | Normal (no alternate screen) | Pick a host, ssh to it, exit back to the shell. |
| `sshc -m` / `sshc --manage` | **Manage** | Alternate screen | Add / edit / delete hosts, manage tags, see probe state. |

Both modes read the same host data (`~/.ssh/config` + `Include`d files)
and use the same fuzzy filter (nucleo). They differ in lifecycle and
key bindings.

## Inline mode

`sshc` (no arguments) opens a ratatui `Viewport::Inline(N)` below your
shell prompt — no alternate-screen takeover. After you select a host
and press Enter, the viewport is cleared, raw mode is released, and
`ssh <alias>` runs in the same shell. When ssh exits, the process
exits with ssh's status code. The inline UI is not re-entered.

### Viewport size

`(terminal_height - 5).clamp(8, 15)` rows. If the terminal is taller
than 20 rows you see 15 rows of host list; if it's tight you get
fewer. Below 12 rows total, `sshc` prints a one-line stderr notice and
falls back to manage mode (which uses the alternate screen).

### Key bindings

| Key | Action |
|---|---|
| any printable char | append to fuzzy filter (immediate filter) |
| Backspace | delete one filter char |
| `↑` / `↓` | navigate |
| `j` / `k` | navigate, but only when the filter is empty |
| `r` | reconnect to `state.memory.last_connected_alias`, but only when the filter is empty |
| Enter | ssh to the selected host, then exit |
| Esc | clear filter if non-empty; exit if filter is empty |
| Ctrl+C | exit without spawning ssh |

`j`, `k`, `r` lose their navigation/reconnect semantics as soon as you
have started typing — this matches fzf's muscle memory.

### Process exit code

Inline mode propagates ssh's exit code:

| ssh outcome | `$?` |
|---|---|
| Clean disconnect (`exit 0`) | `0` |
| Quit / Cancel before ssh | `0` |
| `Connect failed` (e.g. unreachable) | low byte of the original code, typically `255` |
| `Failed` (remote command exit) | low byte of the original code |
| `Crashed` / `Unknown termination` | `1` |

## Manage mode

`sshc -m` (or `--manage`) opens the alternate-screen TUI introduced in
v0.3. Features:

- Five-column responsive table: Alias | Account | Host | Port | Status.
  Columns hide in priority order (Account → Port → Host) as the panel
  narrows. Below 60×10 the binary prints "terminal too small".
- Centered dynamic panel sized to the widest data row, clamped to
  50–110 columns wide and 10–32 rows tall.
- Modal subsystem for forms and confirmations.

### Key bindings

| Key | Action |
|---|---|
| `↑` / `↓`, `j` / `k` | navigate |
| `/` | enter fuzzy filter mode |
| `/ @<tag>` | tag-only filter (e.g. `@prod`) |
| Enter | open the modify form for sshc.conf-managed hosts; for external hosts, jump to `$EDITOR` at the host's line |
| `s` | ssh to the selected host (round-trip; TUI suspends and resumes) |
| `r` | reconnect to the last-connected host |
| `a` | add a host (modal form) |
| `d` | delete the selected host (confirmation modal) |
| `t` | edit tags on the selected host |
| `e` | open `$EDITOR` at the selected host's line |
| `?` | help modal |
| Esc | exit filter mode / cancel modal / quit |
| `q` | quit |

Inside a form: Tab / Shift+Tab move between fields, Enter submits (or
advances), Esc cancels, Ctrl+U clears the active field.

### First-run setup (manage mode only)

On the first launch of manage mode `sshc` checks whether your
`~/.ssh/config` already contains an
`Include ~/.ssh/config.d/sshc.conf` directive. If not, a confirmation
modal appears:

- `y` — append the `Include` line to `~/.ssh/config` (with a dated
  `.bak.sshc-YYYYMMDD` backup) and create
  `~/.ssh/config.d/sshc.conf` (mode 0600) under
  `~/.ssh/config.d/` (mode 0700).
- `n` — record the decision in `~/.config/sshc/state.toml`
  (`declined_include_injection = true`). Manage mode then runs
  read-only: browsing and connecting work; `a` / `d` / `t` /
  Enter-on-managed-host surface a status-bar message and become
  no-ops.

Inline mode does not run this setup flow. It reads whatever hosts are
visible to your existing `~/.ssh/config`; if you have never run manage
mode, `sshc.conf` simply doesn't exist yet.

### Host CRUD

- **Add** (`a`): opens a form with six fields — Alias (required),
  HostName (required), User, Port, IdentityFile, Tags (comma-separated).
  The new block is appended to `sshc.conf` via `flock(LOCK_EX_NB)` +
  tempfile + rename.
- **Modify** (Enter on a sshc.conf-managed host): the same form
  pre-populated. Saved through the same atomic write.
- **Delete** (`d`): yes/no confirmation. Removes the host from the
  in-memory list and rewrites `sshc.conf`.
- **Edit tags** (`t`): single-field form for comma-separated tags.

Hosts whose `source_file` is not `sshc.conf` (i.e. hosts defined
directly in `~/.ssh/config` or in other `Include`d files) cannot be
modified by these operations. Pressing Enter on such a row jumps to
`$EDITOR` at the host's line; `d` / `t` surface "this host lives
outside sshc.conf — press e to edit source".

### Tags

Stored as `# @tags: a, b, c` comments immediately above each `Host`
block. Tag values are lowercased and deduped on parse. The filter
supports two syntaxes:

- bare query — fuzzy match against alias, hostname, and tag substrings.
- `@<tag>` — only hosts whose any tag contains `<tag>`.

### Status column

The Status column is two characters, `<probe> <marker>` (with a space
between for visual separation):

| Probe glyph | Meaning |
|---|---|
| `●` (green) | TCP connect succeeded |
| `○` (red) | TCP connect failed |
| `◌` (yellow) | probe in flight |
| (space) | unknown / not probed yet |

| Marker | Meaning |
|---|---|
| `★` (yellow) | the most-recently-connected host (`r` will reconnect here) |
| `·` (dim) | host is defined outside `sshc.conf` — read-only via the TUI |
| (space) | sshc.conf-managed, no special state |

The two halves carry independent meanings (one is automatic
reachability, the other is user-action / source state).

### Probe pool

A background thread pool (`min(8, host_count)` workers) issues TCP
connect probes against each host's `HostName:Port` with a 1-second
timeout per attempt. Results are surfaced through the probe glyph.
Probes never appear in inline mode — the pool is skipped to keep
single-shot launches fast.

## Files written by sshc

- `~/.ssh/config.d/sshc.conf` (mode 0600). Contains every host added
  via `a` in manage mode. Banner at the top:
  `# Managed by sshc. Manual edits inside Host blocks may be overwritten on next save.`
- `~/.ssh/config.d/` (mode 0700). Created if absent.
- `~/.ssh/config` is mutated **once**, when you accept the first-run
  Include prompt. A single `Include ~/.ssh/config.d/sshc.conf` line is
  appended after a dated `.bak.sshc-YYYYMMDD` backup. Manage mode
  never touches your hand-written entries beyond that.
- `$XDG_CONFIG_HOME/sshc/state.toml` (fallback
  `~/.config/sshc/state.toml`). Tracks whether the user accepted the
  Include injection (`include_check_done`,
  `declined_include_injection`) and the last-connected alias
  (`memory.last_connected_alias`). Schema:

  ```toml
  version = 1

  [setup]
  include_check_done = true
  declined_include_injection = false

  [memory]
  last_connected_alias = "web-1"
  ```

## Architecture

```
src/
├── main.rs              thin dispatch: parse_mode → run::inline or manage
├── run.rs               inline() / manage() bootstrapping helpers
├── inline_app.rs        InlineApp (lean fzf-style state machine)
├── app.rs               manage-mode App (modes, probe, state, forms)
├── config/
│   ├── model.rs         Host struct + nucleo fuzzy_score
│   ├── parser.rs        hand-rolled SSH config parser (Include traversal)
│   └── tags.rs          # @tags: parsing
├── error.rs             AppError + per-domain errors
├── exec/
│   ├── ssh.rs           ssh spawn+wait + SshResult classifier
│   └── editor.rs        $EDITOR launcher
├── probe/               TCP connect worker pool (manage mode only)
├── setup/               first-run flow + permissions (manage mode only)
├── state/               state.toml (TOML serde)
├── storage/             sshc.conf flock + atomic write + Include injector
├── tui/
│   ├── lifecycle.rs     TerminalGuard with ScreenMode { Alternate, Inline(N) }
│   ├── runtime.rs       manage-mode event loop + ssh round-trip
│   └── inline_runtime.rs inline-mode event loop + ssh single-shot
└── ui/                  render, layout, list, status bar, modal, forms
```

Module-boundary regression greps `R-G1..R-G9` (documented in
`docs/TESTING.md` §2) enforce constraints like "app.rs must not touch
the terminal", "inline_app must not depend on the modal/probe/forms
subsystems", and "main.rs ≤ 80 non-comment lines".

## Testing

```sh
cargo test --release                         # 190 tests
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

Headless layout preview (no TUI takeover, color/style lost):

```sh
cargo run --release --example render_preview    # manage panel at 100x30 etc.
cargo run --release --example inline_prototype  # inline viewport smoke
```

See `docs/TESTING.md` for the per-release manual checklist (terminal
restoration, ssh round-trip, panic safety, first-run flow, etc.).

## Migration from sshs (pre-rename releases)

The project was published under the name `sshs` from v0.1.0 through
v0.4.0. That name conflicts with an unrelated already-published CLI,
so the binary and file paths were renamed to `sshc`. If you ran a
pre-rename release, migrate manually:

```sh
mv ~/.ssh/config.d/sshs.conf ~/.ssh/config.d/sshc.conf
mv ~/.config/sshs ~/.config/sshc
# Then edit ~/.ssh/config: replace the line
#   Include ~/.ssh/config.d/sshs.conf
# with
#   Include ~/.ssh/config.d/sshc.conf
```

The state.toml schema is unchanged; only the parent directory moved.

## Limitations

- Linux and macOS only. No Windows support.
- Probe glyphs require a UTF-8 locale; non-UTF-8 terminals will show
  replacement characters.
- The probe column is manage-mode only. Inline mode skips the worker
  pool to keep single-shot launches fast.
- The tag column is omitted in inline mode (space-constrained).
- The `e` editor jump uses the `+<line>` flag; `vi` / `vim` / `nvim` /
  `nano` honor it. Other editors open the file but may ignore the line
  specifier.
- Fuzzy search is suited for ≤ ~500 hosts. Larger lists may feel
  sluggish.

## License

MIT. See [LICENSE](LICENSE).
