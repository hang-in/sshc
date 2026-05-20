# sshc

> Rust로 만든 빠른 TUI로 SSH 호스트를 관리·연결합니다.

[![brew tap](https://img.shields.io/badge/brew-hang--in%2Ftap%2Fsshc-blue)](https://github.com/hang-in/homebrew-tap)
[![license](https://img.shields.io/badge/license-MIT-green)](LICENSE)

```sh
brew install hang-in/tap/sshc
```

> English: [README.md](README.md)

## 데모

<!-- GIF goes here. -->

## 왜 sshc인가?

`~/.ssh/config`에 항목이 30개를 넘어가면 어떤 alias가 어느 서버인지
기억하기 어렵습니다. 손으로 추가·수정하는 것도 지겹고, ssh
자동완성은 태그·문맥을 보여주지 못합니다. sshc는 셸 프롬프트 바로
아래에 fzf 형태의 picker를 열어 — 입력 → Enter → 접속, 끝입니다.

## 빠른 시작

```sh
brew install hang-in/tap/sshc
sshc
```

이게 전부입니다. sshc는 기존 `~/.ssh/config`와 거기에서 `Include`된
파일들을 그대로 읽습니다 — 별도 설정 없습니다. 추가/수정/삭제 + 태그가
필요하면 `sshc -m`.

## 두 가지 모드

| 명령 | 모드 | 하는 일 |
|---|---|---|
| `sshc` | 인라인 | fzf 스타일 picker → ssh → 셸 복귀. Alt 화면 미사용. |
| `sshc -m` | 관리 | 풀 TUI: 추가/수정/삭제, 태그, probe 글리프, `$EDITOR` 점프. |

인라인 모드는 스크롤백을 그대로 둡니다. 관리 모드는 편집에 적합한
풀스크린 TUI라서 별도 플래그 뒤에 둡니다.

## 키 바인딩

### 인라인

| 키 | 동작 |
|---|---|
| (모든 글자) | 즉시 필터 (fzf 스타일) |
| Backspace | 필터에서 한 글자 삭제 |
| ↑ / ↓ | 이동 |
| j / k | 필터가 비었을 때만 이동 |
| r | 필터가 비었을 때만 마지막 호스트 재접속 |
| Enter | ssh → 종료 |
| Esc | 필터가 있으면 지움, 없으면 종료 |
| Ctrl+C | 종료 |

### 관리

| 키 | 동작 |
|---|---|
| ↑/↓, j/k | 이동 |
| `/` | 필터 모드 진입 (`@태그`는 태그 전용) |
| Enter | 관리 호스트: 수정 폼 / 외부 호스트: `$EDITOR` 점프 |
| s | ssh |
| r | 마지막 호스트 재접속 |
| a | 호스트 추가 |
| d | 삭제 (확인 모달) |
| t | 태그 편집 |
| e | 해당 호스트 줄에서 `$EDITOR` 열기 |
| ? | 도움말 모달 |
| q, Esc | 종료 / 취소 |

폼 안: Tab/Shift+Tab 필드 이동, Enter 제출, Esc 취소, Ctrl+U 활성 필드
비우기.

## 설치

### Homebrew (macOS + Linux)

```sh
brew install hang-in/tap/sshc
```

`brew upgrade sshc`로 새 릴리스가 자동 반영됩니다.

### git에서 cargo install

```sh
cargo install --git https://github.com/hang-in/sshc --tag v0.4.1
```

### 소스 빌드

```sh
git clone https://github.com/hang-in/sshc
cd sshc
cargo install --path .
```

Rust 1.85+ 툴체인이 필요합니다.

## 설정

sshc는 사용자가 손으로 작성한 `~/.ssh/config`를 한 줄 `Include`
이상으로는 건드리지 않습니다.

- `sshc -m`에서 추가한 호스트는 `~/.ssh/config.d/sshc.conf` (0600)에
  씁니다. 파일 상단:
  `# Managed by sshc. Manual edits inside Host blocks may be overwritten on next save.`
- 관리 모드 첫 실행 시 `~/.ssh/config` 끝에
  `Include ~/.ssh/config.d/sshc.conf`를 추가할지 묻습니다 (수락 시
  `.bak.sshc-YYYYMMDD` 백업 자동 생성). 거절하면 관리 모드는 read-only
  (탐색·접속은 동작).
- 상태(Include 결정, 마지막 접속 alias)는 `~/.config/sshc/state.toml`에
  저장됩니다.
- 태그는 각 `Host` 블록 바로 위 줄의 `# @tags: a, b, c` 주석으로
  저장됩니다. `@<태그>`로 필터링 (예: `@prod`).

## 다른 도구와의 비교

- **그냥 `ssh <alias>`**: 변함없는 source of truth. sshc는 기존 설정을
  읽기만 합니다 — 현재 사용 방식이 바뀌지 않습니다.
- **[storm](https://github.com/emre/storm)**: Python CLI로 `~/.ssh/config`를
  직접 편집. sshc는 별도 파일을 두고 단 한 줄의 Include만 주입합니다.

## 라이선스

MIT. [LICENSE](LICENSE) 참고.

## 기여

이슈 / PR 환영합니다. 24시간 안에 응답하는 걸 목표로 합니다.
