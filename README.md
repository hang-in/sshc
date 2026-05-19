# sshs

A minimal TUI for managing and connecting to SSH hosts defined in `~/.ssh/config`.
~/.ssh/config에 정의된 SSH 호스트를 관리하고 연결하는 최소 TUI 애플리케이션.

## Features / 기능

- List all non-wildcard `Host` entries from `~/.ssh/config`
- Fuzzy filter hosts by alias or hostname
- One-key SSH connection (replaces current process via `exec()`)
- Open `$EDITOR` at the exact line of a host block
- `Include` directive support with circular detection and depth limit
- Terminal state always restored — even on panic
- ~/.ssh/config가 없어도 빈 상태로 안전하게 동작

  ~/.ssh/config의 와일드카드가 아닌 `Host` 항목을 나열하고, 퍼지 필터로 검색한 뒤
  Enter 한 번으로 SSH 연결하거나 e 키로 에디터를 해당 줄에서 바로 열 수 있습니다.

## Install / 설치

```sh
cargo install --path .
```

## Usage / 사용법

```sh
sshs
```

Opens a centered TUI listing all SSH hosts from `~/.ssh/config`.
화면 중앙에 `~/.ssh/config`의 모든 SSH 호스트가 나열됩니다.

### Keybindings / 키바인딩

| Key | Action | 동작 |
|-----|--------|------|
| `↑` / `k` | Move selection up | 위로 이동 |
| `↓` / `j` | Move selection down | 아래로 이동 |
| `/` | Enter fuzzy filter mode | 퍼지 필터 모드 진입 |
| `Enter` | Connect to selected host | 선택한 호스트에 SSH 연결 |
| `e` | Open `$EDITOR` at host config block | 해당 호스트 설정 줄에서 에디터 열기 |
| `Esc` | Exit filter mode / Quit | 필터 모드 종료 / 종료 |
| `q` | Quit | 종료 |

### Editor Jump / 에디터 줄 이동

When pressing `e`, the editor opens `~/.ssh/config` at the line where the selected `Host` block starts.
The `+<line>` flag works with `vi`, `vim`, `nvim`, and `nano`.
Other editors (e.g., `code`, `emacs`) will open the file but may ignore the line specifier.

`e`를 누르면 `~/.ssh/config`를 선택한 `Host` 블록의 시작 줄에서 엽니다.
`+<줄번호>` 플래그는 `vi`, `vim`, `nvim`, `nano`에서 작동하며,
`code`, `emacs` 등 다른 에디터는 파일만 열고 줄 이동은 무시할 수 있습니다.

## Architecture / 아키텍처

```
src/
├── main.rs          — Event loop, panic hook, terminal restore
├── app.rs           — App state, keybindings, fuzzy filter
├── config/
│   ├── model.rs     — Host struct + Display/fuzzy_match
│   └── parser.rs    — Hand-rolled SSH config parser (line-aware)
├── exec/
│   ├── ssh.rs       — SSH exec() (Unix-only)
│   └── editor.rs    — $EDITOR +line jump
└── ui/
    ├── layout.rs    — Centered panel calculation
    ├── list.rs      — Host list widget
    └── mod.rs        — Render integration
```

## Testing / 테스트

```sh
cargo test                              # 30 tests (unit + integration + parser fixtures)
cargo clippy --all-targets -- -D warnings
cargo fmt --check
cargo build --release
```

## Limitations / 제한사항

- Unix-only — uses `exec()` to replace the process with SSH (`#[cfg(unix)]` guard)
- Read-only — edit mode delegates to `$EDITOR`, no direct file writes
- Fuzzy search is inline substring match (sufficient for <500 hosts)
- No `Host *` or wildcard patterns in the list view

  Unix 전용(`exec()` 사용), 읽기 전용(편집은 `$EDITOR`에 위임),
  퍼지 검색은 인라인 서브스트링 매치(500호스트 미만에 충분),
  와일드카드 `Host *` 패턴은 목록에서 제외됩니다.

## License / 라이선스

MIT