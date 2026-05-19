# sshs

A TUI for browsing, connecting to, and managing SSH hosts defined in
`~/.ssh/config` (and `~/.ssh/config.d/sshs.conf` for sshs-managed entries).
~/.ssh/config 호스트를 탐색·연결·관리하는 TUI 애플리케이션.

## Features / 기능

- **Browse**: list all non-wildcard `Host` entries from `~/.ssh/config`
  and any `Include`d files, with a 5-column responsive table
  (Alias | Account | Host | Port | Status).
- **Filter**: fuzzy search by alias / hostname, or tag-prefix search
  (`@prod`). Default filter also matches tags as a fallback.
- **Connect**: one-key SSH (`Enter`) or reconnect to the most recent
  host (`r`). `★` marks the last-connected host.
- **Edit**: `e` opens `$EDITOR` at the exact line of the selected host.
- **Manage** (new in v0.3): `a` add, `m` modify, `d` delete, `t` edit
  tags — all via modal forms backed by `~/.ssh/config.d/sshs.conf`.
- **Probe** (new in v0.3): background TCP connect checks surface
  reachability in the Status column.
- **Tags** (new in v0.3): per-host tags stored as `# @tags: a, b`
  comments; rendered as `[t1,t2]` cyan prefix on the Alias column.
- **Source-aware**: hosts outside sshs.conf are marked `·` and
  protected from in-TUI edits — press `e` to edit the source file.
- **Terminal-safe**: raw mode + alt screen restored on exit, error, or
  panic. Works on Linux and macOS.

v0.3에서 호스트 추가/수정/삭제(`a`/`m`/`d`)와 태그(`t`),
백그라운드 프로브, sshs.conf Include 자동 설정이 추가되었습니다.

## Install / 설치

```sh
cargo install --path .
```

## Usage / 사용법

```sh
sshs
```

On first launch sshs offers to add an
`Include ~/.ssh/config.d/sshs.conf` line to your `~/.ssh/config`
(with a dated backup). Decline and sshs runs read-only.

처음 실행 시 `~/.ssh/config`에 `Include` 줄 추가를 제안합니다 (백업 자동 생성).
거절하면 sshs.conf 쓰기 기능은 비활성화되고 탐색·연결만 가능합니다.

### Keybindings / 키바인딩

| Key | Action | 동작 |
|-----|--------|------|
| `↑` / `k` | Move selection up | 위로 이동 |
| `↓` / `j` | Move selection down | 아래로 이동 |
| `/` | Enter fuzzy filter mode (`@tag` for tag filter) | 퍼지/태그 필터 |
| `Enter` | Connect to selected host | SSH 연결 |
| `r` | Reconnect to last host | 마지막 호스트 재연결 |
| `a` | Add host (modal form) | 호스트 추가 |
| `m` | Modify selected host | 선택 호스트 수정 |
| `d` | Delete selected host (with confirm) | 선택 호스트 삭제 |
| `t` | Edit tags on selected host | 태그 편집 |
| `e` | Open `$EDITOR` at host's line | 에디터로 열기 |
| `?` | Show help | 도움말 |
| `Esc` | Exit filter / cancel modal / Quit | 필터 종료/모달 취소/종료 |
| `q` | Quit | 종료 |

Inside a form modal: `Tab` / `Shift+Tab` move between fields,
`Enter` submits (or advances), `Esc` cancels, `Ctrl+U` clears
the active field.

### Status column / 상태 컬럼

The 2-character Status column encodes `<probe><marker>`:

- Marker `★`: last-connected host
- Marker `·`: host lives outside `sshs.conf` (read-only via TUI)
- Probe glyph: reachability (added in v0.3 — current visualization
  reserves the slot; visible glyphs land with full wiring)

### sshs.conf / sshs.conf 파일

v0.3 introduces a managed file at `~/.ssh/config.d/sshs.conf` (mode
`0600`). Hosts you add via `a` are written there with a banner
warning that manual edits inside `Host` blocks may be overwritten on
the next save. Your hand-written `~/.ssh/config` is never modified
beyond a single `Include` line (with a dated `.bak` backup).

`a` 키로 추가한 호스트는 `~/.ssh/config.d/sshs.conf`에 저장됩니다.
sshs는 메인 `~/.ssh/config`를 직접 건드리지 않고 `Include` 한 줄만 추가합니다
(백업 자동 생성).

### State file / 상태 파일

`~/.config/sshs/state.toml` (or `$XDG_CONFIG_HOME/sshs/state.toml`)
remembers:

- whether the user accepted or declined the Include injection
- the last-connected alias (for `r` reconnect across sessions)

## Architecture / 아키텍처

```
src/
├── main.rs            — bootstrap + first-run setup + AppAction dispatch
├── app.rs             — state machine (List + Modal modes, probe, state)
├── config/
│   ├── model.rs       — Host struct (alias, hostname, port, tags, source)
│   ├── parser.rs      — Hand-rolled SSH config parser (line-aware)
│   └── tags.rs        — # @tags: parse + render + normalize
├── error.rs           — AppError, StorageError, SetupError, ProbeError
├── exec/              — ssh spawn + $EDITOR
├── probe/             — TCP connect worker pool + generation guard
├── setup/             — first-run flow (scaffolding + permission gates)
├── state/             — state.toml (TOML serde)
├── storage/           — sshs.conf flock + atomic write + Include injector
├── tui/               — terminal lifecycle + event loop runtime
└── ui/                — render, layout, list, status bar, modal, forms
```

## Testing / 테스트

```sh
cargo test --release                     # 162 tests
cargo clippy --all-targets -- -D warnings
cargo fmt --check
cargo build --release
```

See `docs/TESTING.md` for the full automated/manual checklist and the
R-G1..R-G8 module-boundary regression greps.

## Limitations / 제한사항

- Unix-only (Linux + macOS). No Windows support.
- Probe glyph visualization will populate as wiring lands; the column
  is reserved.
- The `e` editor jump uses the `+<line>` flag; non-vi/vim/nvim/nano
  editors open the file but may ignore the line specifier.
- Fuzzy search uses nucleo; sufficient for ≤ 500 hosts.

  Linux/macOS 전용, 프로브 글리프는 후속 와이어링 시점에 채워집니다.
  500호스트 미만에서 nucleo 퍼지 검색이 충분합니다.

## License / 라이선스

MIT
