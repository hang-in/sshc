# sshc

SSH 설정에 정의된 호스트를 탐색·연결·관리하는 터미널 UI. `~/.ssh/config`
와 거기서 `Include`된 파일들을 모두 읽고, 관리 모드에서는 신규 호스트를
별도 파일 `~/.ssh/config.d/sshc.conf`에 기록해서 사용자가 손으로 작성한
설정은 건드리지 않습니다.

> English version: [README.md](README.md)

- **상태**: v0.4.0 — Linux + macOS, MIT.
- **기술 스택**: Rust, ratatui 0.29, crossterm 0.28, nucleo 0.5, nix flock,
  serde/toml.
- **테스트 규모**: 자동화 테스트 190개 (라이브러리 단위 147개 + 통합 43개).

## 설치

### Homebrew (macOS / Linux)

```sh
brew install hang-in/tap/sshc
```

tap 포뮬러가 갱신되면 `brew upgrade sshc`로 다음 릴리스를 받습니다.

### 소스 빌드

```sh
git clone https://github.com/hang-in/sshc
cd sshc
cargo install --path .
```

Rust 1.85+ 툴체인이 필요합니다. `cargo build --release` 후 바이너리는
`target/release/sshc`에 생깁니다.

## 데모

<!-- brew 설치 경로가 살아나면 GIF를 여기에 삽입하세요.            -->
<!-- 예시: `brew install hang-in/tap/sshc` → `sshc` (인라인 모드) -->
<!-- → 퍼지 필터 → Enter → ssh 세션 → 종료 → 셸 프롬프트.         -->

## 두 가지 모드

`sshc`는 커맨드라인 플래그로 두 모드 중 하나를 골라 실행합니다.

| 명령 | 모드 | 터미널 | 용도 |
|---|---|---|---|
| `sshc` | **인라인** | 일반 화면 (alt 화면 미사용) | 호스트 선택 → ssh → 셸로 복귀. |
| `sshc -m` / `sshc --manage` | **관리** | Alternate screen | 추가/수정/삭제, 태그 편집, probe 상태 확인. |

두 모드 모두 같은 호스트 데이터(`~/.ssh/config` + `Include` 파일들)를
읽고 같은 퍼지 필터(nucleo)를 씁니다. 차이는 터미널 생명주기와 키
바인딩에 있습니다.

## 인라인 모드

`sshc` (인자 없음)는 ratatui `Viewport::Inline(N)`을 셸 프롬프트 아래에
열어둡니다 — alternate screen으로 전환하지 않습니다. 호스트를 선택해
Enter를 누르면 뷰포트가 비워지고 raw 모드가 해제된 다음 같은 셸에서
`ssh <alias>`가 실행됩니다. ssh가 끝나면 프로세스가 ssh의 종료 코드로
종료됩니다. 인라인 UI는 다시 열리지 않습니다.

### 뷰포트 크기

`(터미널_높이 - 5).clamp(8, 15)` 줄입니다. 터미널이 20줄 이상이면
호스트 목록을 15줄까지, 좁으면 그보다 작게 보여줍니다. 전체 12줄
미만이면 `sshc`는 stderr에 한 줄짜리 안내를 출력하고 관리 모드(alt
화면)로 폴백합니다.

### 키 바인딩

| 키 | 동작 |
|---|---|
| 출력 가능한 모든 문자 | 퍼지 필터에 즉시 추가 (즉시 필터) |
| Backspace | 필터에서 한 글자 삭제 |
| `↑` / `↓` | 이동 |
| `j` / `k` | 필터가 비었을 때만 이동 |
| `r` | 필터가 비었을 때만 `state.memory.last_connected_alias`로 재접속 |
| Enter | 선택된 호스트로 ssh 후 종료 |
| Esc | 필터가 있으면 지움, 없으면 종료 |
| Ctrl+C | ssh 없이 즉시 종료 |

`j` / `k` / `r`은 사용자가 한 글자라도 입력한 시점부터 일반 필터 문자로
바뀝니다 — fzf의 손버릇과 일치합니다.

### 프로세스 종료 코드

인라인 모드는 ssh의 종료 코드를 그대로 전달합니다.

| ssh 결과 | `$?` |
|---|---|
| 정상 종료 (`exit 0`) | `0` |
| ssh 호출 전 종료/취소 | `0` |
| `Connect failed` (예: 도달 불가) | 원래 코드의 하위 바이트, 보통 `255` |
| `Failed` (원격 명령 종료) | 원래 코드의 하위 바이트 |
| `Crashed` / `Unknown termination` | `1` |

## 관리 모드

`sshc -m` (또는 `--manage`)는 v0.3에서 도입된 alt 화면 TUI를 엽니다.

- 5컬럼 반응형 테이블: Alias | Account | Host | Port | Status.
  패널이 좁아지면 우선순위(Account → Port → Host)대로 숨겨집니다.
  60×10 미만이면 "터미널이 너무 작다" 메시지를 띄웁니다.
- 데이터 폭에 맞춘 동적 패널 — 가로 50–110 / 세로 10–32 사이로 클램프.
- 폼·확인용 모달 시스템.

### 키 바인딩

| 키 | 동작 |
|---|---|
| `↑` / `↓`, `j` / `k` | 이동 |
| `/` | 퍼지 필터 모드 진입 |
| `/ @<태그>` | 태그 전용 필터 (예: `@prod`) |
| Enter | sshc.conf 관리 호스트면 수정 폼; 외부 호스트면 `$EDITOR`로 해당 줄 점프 |
| `s` | 선택 호스트로 ssh (TUI 일시중단 후 복귀) |
| `r` | 마지막 접속 호스트로 재접속 |
| `a` | 호스트 추가 (모달 폼) |
| `d` | 선택 호스트 삭제 (확인 모달) |
| `t` | 태그 편집 |
| `e` | 선택 호스트의 줄에서 `$EDITOR` 열기 |
| `?` | 도움말 모달 |
| Esc | 필터 종료 / 모달 취소 / 종료 |
| `q` | 종료 |

폼 안: Tab/Shift+Tab 필드 이동, Enter 제출(또는 다음 필드), Esc 취소,
Ctrl+U 활성 필드 비우기.

### 첫 실행 설정 (관리 모드 전용)

관리 모드를 처음 실행하면 `sshc`는 `~/.ssh/config`에
`Include ~/.ssh/config.d/sshc.conf`가 이미 있는지 확인합니다. 없으면
확인 모달이 뜹니다.

- `y` — `~/.ssh/config`에 `Include` 줄을 추가하고
  (`.bak.sshc-YYYYMMDD` 형식의 백업 자동 생성),
  `~/.ssh/config.d/sshc.conf`를 생성합니다(0600). 부모 디렉토리
  `~/.ssh/config.d/`도 없으면 만들고(0700) 만듭니다.
- `n` — 결정을 `~/.config/sshc/state.toml`에 기록합니다
  (`declined_include_injection = true`). 이 상태에서 관리 모드는
  read-only로 동작 — 탐색·연결은 되지만 `a` / `d` / `t` /
  관리 호스트에서의 Enter는 상태바 안내만 띄우는 no-op이 됩니다.

인라인 모드는 이 설정 흐름을 실행하지 않습니다. 기존 `~/.ssh/config`에
보이는 호스트만 읽으며, 관리 모드를 한 번도 안 돌렸다면 `sshc.conf`는
아예 없는 상태입니다.

### 호스트 CRUD

- **추가** (`a`): 6개 필드 폼이 열립니다 — Alias (필수), HostName (필수),
  User, Port, IdentityFile, Tags (콤마 구분). 새 블록은 `flock(LOCK_EX_NB)`
  + 임시파일 + rename으로 `sshc.conf`에 원자적으로 추가됩니다.
- **수정** (sshc.conf 관리 호스트 위에서 Enter): 같은 폼이 채워진 채로
  열립니다. 동일한 원자적 쓰기로 저장.
- **삭제** (`d`): yes/no 확인. 메모리 목록에서 제거 후 `sshc.conf` 재작성.
- **태그 편집** (`t`): 단일 필드 폼(콤마 구분).

`source_file`이 `sshc.conf`가 아닌 호스트(즉 `~/.ssh/config`에 직접
정의됐거나 다른 `Include` 파일에 있는 호스트)는 이 명령들로 수정할 수
없습니다. Enter는 `$EDITOR`로 해당 줄 점프, `d` / `t`는 상태바에
"외부 source — e로 편집" 안내가 뜹니다.

### 태그

각 `Host` 블록 바로 위 줄의 `# @tags: a, b, c` 주석으로 저장됩니다.
파싱 시 소문자화 + 중복 제거. 필터는 두 가지 문법을 지원합니다.

- 일반 쿼리 — alias / hostname / 태그 부분 문자열로 퍼지 매칭.
- `@<태그>` — 해당 태그가 부분 일치하는 호스트만.

### Status 컬럼

Status 컬럼은 두 글자 `<probe> <marker>`입니다 (시각적 분리를 위한
공백 1칸 포함).

| Probe 글리프 | 의미 |
|---|---|
| `●` (녹색) | TCP connect 성공 |
| `○` (적색) | TCP connect 실패 |
| `◌` (노란색) | probe in-flight |
| (공백) | unknown / 아직 측정 안 됨 |

| Marker | 의미 |
|---|---|
| `★` (노란색) | 가장 최근에 접속한 호스트 (`r` 재접속 대상) |
| `·` (dim) | `sshc.conf` 외부에 정의된 호스트 — TUI에서 read-only |
| (공백) | sshc.conf 관리 호스트, 특별 상태 없음 |

두 글자는 의미가 독립적입니다(왼쪽은 자동 도달성, 오른쪽은
사용자 행동 / 소스 상태).

### Probe 풀

백그라운드 스레드 풀(워커 수 `min(8, 호스트 수)`)이 각 호스트의
`HostName:Port`에 대해 1초 타임아웃으로 TCP connect 프로브를 수행하고,
결과를 probe 글리프로 표시합니다. 인라인 모드에서는 풀을 띄우지 않습니다
— 단발 실행 속도를 우선합니다.

## sshc가 쓰는 파일

- `~/.ssh/config.d/sshc.conf` (0600). 관리 모드에서 `a`로 추가한 호스트가
  들어갑니다. 파일 상단 배너:
  `# Managed by sshc. Manual edits inside Host blocks may be overwritten on next save.`
- `~/.ssh/config.d/` (0700). 없으면 생성.
- `~/.ssh/config`은 단 한 번만 수정됩니다 — 첫 실행 시 Include 프롬프트에
  `y`로 응답할 때, `.bak.sshc-YYYYMMDD` 백업을 만든 뒤
  `Include ~/.ssh/config.d/sshc.conf` 한 줄을 추가합니다. 그 외에는 사용자가
  쓴 항목을 절대 건드리지 않습니다.
- `$XDG_CONFIG_HOME/sshc/state.toml` (없으면
  `~/.config/sshc/state.toml`). Include 주입 수락 여부
  (`include_check_done`, `declined_include_injection`)와 마지막 접속 alias
  (`memory.last_connected_alias`)를 기록합니다. 스키마:

  ```toml
  version = 1

  [setup]
  include_check_done = true
  declined_include_injection = false

  [memory]
  last_connected_alias = "web-1"
  ```

## 아키텍처

```
src/
├── main.rs              얇은 디스패치: parse_mode → run::inline 또는 manage
├── run.rs               inline() / manage() 부트스트랩
├── inline_app.rs        InlineApp (fzf 패턴의 lean 상태머신)
├── app.rs               관리 모드 App (모드, probe, state, 폼)
├── config/
│   ├── model.rs         Host struct + nucleo fuzzy_score
│   ├── parser.rs        손으로 작성한 SSH config 파서 (Include 재귀)
│   └── tags.rs          # @tags: 파싱
├── error.rs             AppError + 도메인별 에러
├── exec/
│   ├── ssh.rs           ssh spawn+wait + SshResult 분류기
│   └── editor.rs        $EDITOR 런처
├── probe/               TCP connect 워커 풀 (관리 모드 전용)
├── setup/               첫 실행 흐름 + 권한 (관리 모드 전용)
├── state/               state.toml (TOML serde)
├── storage/             sshc.conf flock + atomic write + Include injector
├── tui/
│   ├── lifecycle.rs     TerminalGuard with ScreenMode { Alternate, Inline(N) }
│   ├── runtime.rs       관리 모드 이벤트 루프 + ssh 라운드트립
│   └── inline_runtime.rs 인라인 모드 이벤트 루프 + ssh 단발 실행
└── ui/                  렌더, 레이아웃, 리스트, 상태바, 모달, 폼
```

모듈 경계 회귀 grep `R-G1..R-G9` (`docs/TESTING.md` §2)이 "app.rs는
터미널을 만지지 않는다", "inline_app은 modal/probe/forms 의존을 가지지
않는다", "main.rs는 ≤ 80 non-comment 줄" 같은 규칙을 강제합니다.

## 테스트

```sh
cargo test --release                         # 190 tests
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

헤드리스 레이아웃 미리보기(TUI를 띄우지 않음, 색·스타일은 손실):

```sh
cargo run --release --example render_preview    # 관리 패널 100x30 등
cargo run --release --example inline_prototype  # 인라인 뷰포트 스모크
```

릴리스마다 돌리는 수동 체크리스트는 `docs/TESTING.md` 참고
(터미널 복원, ssh 라운드트립, 패닉 안전, 첫 실행 흐름 등).

## sshs(개명 전)에서의 마이그레이션

이 프로젝트는 v0.1.0–v0.4.0 동안 `sshs`라는 이름으로 배포됐습니다.
같은 이름의 다른 CLI가 이미 존재해서 바이너리·파일 경로를 `sshc`로
개명했습니다. 개명 전 릴리스를 쓰셨다면 수동 마이그레이션이 필요합니다.

```sh
mv ~/.ssh/config.d/sshs.conf ~/.ssh/config.d/sshc.conf
mv ~/.config/sshs ~/.config/sshc
# 그리고 ~/.ssh/config 안의
#   Include ~/.ssh/config.d/sshs.conf
# 를
#   Include ~/.ssh/config.d/sshc.conf
# 로 수정
```

state.toml 스키마는 그대로입니다 — 부모 디렉토리 이름만 바뀌었습니다.

## 제한 사항

- Linux와 macOS만 지원합니다. Windows 미지원.
- Probe 글리프는 UTF-8 로케일이 필요합니다 — 비 UTF-8 터미널은 대체
  문자로 보입니다.
- Probe 컬럼은 관리 모드 전용입니다. 인라인 모드는 워커 풀을 띄우지
  않아 단발 실행이 빠릅니다.
- 인라인 모드에는 태그 컬럼이 빠져 있습니다 (공간 제약).
- `e` 에디터 점프는 `+<line>` 플래그를 사용합니다 — `vi` / `vim` /
  `nvim` / `nano`가 인식합니다. 다른 에디터는 파일은 열리지만 줄
  지정은 무시될 수 있습니다.
- 퍼지 검색은 ~500개 호스트까지 적합합니다. 더 많아지면 느려질 수
  있습니다.

## 라이선스

MIT. [LICENSE](LICENSE) 참고.
