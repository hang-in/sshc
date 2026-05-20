# sshc

A TUI for browsing, connecting to, and managing SSH hosts defined in
`~/.ssh/config` (and `~/.ssh/config.d/sshc.conf` for sshc-managed entries).
~/.ssh/config 호스트를 탐색·연결·관리하는 TUI.

## What's new in v0.4

The default command is now an **inline (fzf-style) host browser** that
opens BELOW your shell prompt — no alternate screen takeover. Type to
filter, `Enter` to ssh, `Esc` to cancel. When ssh exits, the shell
prompt returns; the inline TUI does NOT re-open.

The full management TUI (CRUD, tags, probes, edit) is one flag away:
`sshc -m` (or `--manage`).

v0.4에서 기본 `sshc`는 **인라인 호스트 선택기**(셸 프롬프트 아래에 인라인
뷰포트). 타이핑으로 즉시 필터, Enter로 ssh. ssh가 끝나면 셸로 복귀 —
TUI는 재진입하지 않습니다. 전체 관리 TUI는 `sshc -m`로 진입.

## Features

### Inline mode (default)
- Open a 15-line viewport below the shell prompt — no alternate screen.
- Immediate fuzzy filter (every keystroke updates the list).
- `Enter` selects + spawns ssh + exits back to the shell with ssh's
  status code.
- `r` reconnects to the most recent host (cross-session via
  `~/.config/sshc/state.toml`).
- Falls back to manage mode automatically if the terminal is shorter
  than 12 rows.

### Manage mode (`-m` / `--manage`)
- v0.3 alternate-screen TUI: 5-column responsive table, last-connected
  marker, source marker, modal subsystem.
- **Host manager**: `a` add, `d` delete (with confirm), `Enter` open
  edit form (or `$EDITOR` for external hosts), `t` edit tags.
- **`s` connect**: ssh-spawn on the selected host (v0.3 Enter behaviour
  moved to `s` so Enter can mean "open this for editing").
- **Tags**: stored as `# @tags: a, b` comments above each Host block.
  Filter with `@<tag>` or rely on the default fuzzy filter (also
  matches tags as a fallback).
- **First-run setup**: offers to add `Include ~/.ssh/config.d/sshc.conf`
  to `~/.ssh/config` (with `.bak.sshc-YYYYMMDD` backup). Decision is
  persisted to state.toml.
- **Probe column**: background TCP connect checks surface reachability
  (`●` open / `○` failed / `◌` inflight) in the Status column.

### Both modes
- Hand-rolled parser with `Include` directive support + circular
  detection.
- Terminal-safe: raw mode + alternate screen (manage only) restored on
  exit, error, or panic.
- Linux + macOS. Probe glyph requires a UTF-8 locale.

## Install

```sh
cargo install --path .
```

## Usage

```sh
sshc          # Inline mode (default in v0.4)
sshc -m       # Full management TUI
sshc --manage # Same as -m
```

On first run of manage mode, sshc offers to add an `Include` line to
your `~/.ssh/config` (with a dated backup). Decline and the manager
runs read-only (browse + connect only).

### Inline keybindings

| Key | Action |
|-----|--------|
| any printable char | append to fuzzy filter |
| `↑`/`↓`, `j`/`k`* | navigate |
| `Backspace` | delete one filter char |
| `Esc` | clear filter (or exit if filter empty) |
| `Ctrl+C` | exit (no ssh) |
| `Enter` | ssh to selected host (then exit) |
| `r`* | reconnect to last host (`state.memory.last_connected_alias`) |

*`j` / `k` / `r` only navigate / reconnect when the filter is empty.
Once you start typing, they become ordinary filter characters (fzf
muscle memory).

### Manage keybindings

| Key | Action |
|-----|--------|
| `↑`/`↓`, `j`/`k` | navigate |
| `/` | enter fuzzy filter mode (`@tag` for tag filter) |
| `Enter` | open edit form (sshc.conf hosts) / `$EDITOR` (external) |
| `s` | ssh connect to selected host |
| `r` | reconnect to last host |
| `a` / `d` / `t` | add / delete / edit tags |
| `e` | open `$EDITOR` at host's line |
| `?` | help modal |
| `Esc` | exit filter / cancel modal / quit |
| `q` | quit |

Inside a form modal: `Tab` / `Shift+Tab` move between fields, `Enter`
submits (or advances), `Esc` cancels, `Ctrl+U` clears the active field.

### Status column (manage mode)

The 2-character Status column encodes `<probe><marker>`:

- Probe glyph: reachability (`●` open / `○` failed / `◌` inflight / ` `
  unknown).
- Marker: `★` last-connected host; `·` external-source host
  (read-only via TUI — use `e` or `Enter`); space otherwise.

### sshc.conf

Inline mode reads any host visible to `~/.ssh/config`.
Manage mode writes new hosts to `~/.ssh/config.d/sshc.conf` (mode
0600). Your hand-written `~/.ssh/config` is never modified beyond a
single `Include` line (with a dated `.bak` backup).

### State file

`$XDG_CONFIG_HOME/sshc/state.toml` (fallback `~/.config/sshc/state.toml`)
remembers:

- whether the user accepted/declined the `Include` injection
- the last-connected alias (used by `r` in both modes)

## Architecture

```
src/
├── main.rs              — thin dispatch: parse_mode → run::inline or manage
├── run.rs               — inline() / manage() helpers
├── inline_app.rs        — InlineApp (lean fzf-style state machine)
├── app.rs               — manage-mode App (modes, probe, state, forms)
├── config/{model,parser,tags}.rs
├── error.rs             — AppError + per-domain errors
├── exec/                — ssh spawn + $EDITOR
├── probe/               — TCP connect worker pool (manage mode only)
├── setup/               — first-run flow + permissions (manage mode only)
├── state/               — state.toml (TOML serde)
├── storage/             — sshc.conf flock + atomic write + Include injector
├── tui/
│   ├── lifecycle.rs     — TerminalGuard with ScreenMode { Alternate, Inline(N) }
│   ├── runtime.rs       — manage event loop + ssh round-trip
│   └── inline_runtime.rs — inline event loop + ssh single-shot
└── ui/                  — render, layout, list, status bar, modal, forms
```

## Testing

```sh
cargo test --release                     # 190 tests
cargo clippy --all-targets -- -D warnings
cargo fmt --check
cargo build --release
```

See `docs/TESTING.md` for the full automated/manual checklist and the
R-G1..R-G9 module-boundary regression greps.

Headless layout preview (no TUI takeover):

```sh
cargo run --release --example render_preview   # manage-mode panel
cargo run --release --example inline_prototype # inline viewport
```

## Limitations

- Unix-only (Linux + macOS). No Windows support.
- Probe column is manage-mode only. Inline mode skips probes for a
  fast single-shot launch (no worker pool spin-up).
- Tag column is omitted in inline mode for compactness.
- `e` editor jump uses the `+<line>` flag; non-vi/vim/nvim/nano
  editors open the file but may ignore the line specifier.
- Fuzzy search uses nucleo; sufficient for ≤ 500 hosts.

## License

MIT
