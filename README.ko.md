# sshc

> 터미널에서 SSH 호스트를 빠르게 찾아 접속하고, 필요한 만큼만 안전하게 관리하는 작은 Rust TUI.

[![release](https://img.shields.io/github/v/release/hang-in/sshc?sort=semver)](https://github.com/hang-in/sshc/releases)
[![downloads](https://img.shields.io/github/downloads/hang-in/sshc/total)](https://github.com/hang-in/sshc/releases)
[![brew tap](https://img.shields.io/badge/brew-hang--in%2Ftap%2Fsshc-blue)](https://github.com/hang-in/homebrew-tap)
[![license](https://img.shields.io/badge/license-MIT-green)](LICENSE)
[![rust](https://img.shields.io/badge/rust-1.85%2B-orange)](https://www.rust-lang.org)
[![platform](https://img.shields.io/badge/platform-macOS%20%7C%20Linux-lightgrey)](https://github.com/hang-in/sshc/releases)

```sh
brew install hang-in/tap/sshc
```

> English: [README.md](README.md)

## 데모

![인라인 모드에서 호스트를 골라 ssh 접속하고 셸로 돌아오는 흐름](docs/demos/demo.gif)

## 왜 sshc인가?

`~/.ssh/config`에 호스트가 30개를 넘어가는 순간, 어떤 alias가 어느
서버였는지 머리로 기억하기 시작합니다. 직접 파일을 열어 추가하고
고치는 일도 점점 귀찮아지고요. ssh 자동완성은 alias 이름만 보여줄 뿐
태그나 맥락은 알려주지 않습니다.

sshc는 셸 프롬프트 바로 아래에 picker를 띄워서, 검색해서 고른 다음
Enter 한 번이면 접속까지 끝나도록 해 줍니다.

단일 바이너리, 데몬 없음. 최신 릴리스 사이즈: macOS arm64 ~810 KB,
Linux x64 ~1.16 MB, Windows x64 ~1.10 MB.

## 빠른 시작

```sh
brew install hang-in/tap/sshc
sshc
```

별도 설정은 필요 없습니다. 기존 `~/.ssh/config`와 거기서 `Include`된
파일들을 그대로 읽기만 합니다. 호스트를 직접 추가하거나 고치고
싶으면 `sshc -m`. alias를 이미 알고 있다면 `sshc <alias>`로 picker
없이 바로 ssh — 셸 별칭이나 스크립트에 묶어 쓰기 좋습니다.

## 사용 모드

| 명령 | 모드 | 설명 |
|---|---|---|
| `sshc` | 인라인 | 셸 위에서 호스트 picker를 띄우고, 선택하면 ssh 접속한 뒤 다시 셸로 돌아옵니다. 화면이 전환되지 않습니다. |
| `sshc <alias>` | 다이렉트 | picker를 건너뛰고 바로 `ssh <alias>`. 모르는 alias면 stderr에 안내 후 종료 코드 1. |
| `sshc -m` | 관리 | 전체 화면 TUI. 호스트 추가/수정/삭제, 태그, 상태 확인, `$EDITOR`로 직접 편집까지. |
| `sshc --doctor` | 진단 | 환경 점검만 (수정은 하지 않음): `~/.ssh` 권한, sshc.conf 존재, Include 줄, `ssh` 바이너리, `SSH_AUTH_SOCK`(Windows에서는 OpenSSH/Pageant named pipe), 최신 릴리스 확인. v0.9부터 `~/.ssh/config`의 CRLF 줄종결자와 sshc Include가 다른 Host stanza 내부에 nested된 케이스도 경고합니다. v0.10부터 `ProxyCommand`가 가리키는 실행 파일이 PATH에 없으면 경고합니다 (host 수 묶음). 무언가 깨졌을 때만 비정상 종료. 네트워크 호출이 싫으면 `SSHC_NO_UPDATE_CHECK=1`로 업데이트 점검만 건너뜁니다. |

인라인 모드는 스크롤백을 건드리지 않고, 관리 모드는 편집에 어울리는
전체 화면이라 따로 플래그를 둡니다.

## 키 바인딩

### 인라인

인라인은 모드 기반입니다. 기본은 탐색 모드(`j/k`/`↑/↓`로 이동),
`/`를 누르면 검색 모드로 들어가고 Esc로 빠져나옵니다. 관리 모드에서
`f`로 pin한 호스트와 최근에 접속한 호스트는 알아서 위로 올라오기
때문에, 평소엔 `sshc` 열고 Enter 한 번이면 끝나는 흐름이 됩니다.

| 키 | 동작 |
|---|---|
| ↑/↓, j/k | 호스트 이동 |
| `/` | 검색 모드 진입 |
| Enter | 선택한 호스트로 ssh 접속 후 종료 |
| q, Esc | 종료 (탐색 모드일 때) |
| Ctrl+C | 어디서든 즉시 종료 |
| (검색 중) 글자 입력 | 필터 쿼리에 추가 |
| (검색 중) Backspace | 필터에서 한 글자 지움 |
| (검색 중) Esc | 검색만 닫고 picker는 유지 |

### 관리

| 키 | 동작 |
|---|---|
| ↑/↓, j/k | 호스트 이동 |
| `/` | 필터 모드 진입 (`@태그` 형태로 태그 전용 필터) |
| Enter | sshc.conf 안의 호스트면 수정 폼, 외부 호스트면 `$EDITOR`로 점프 |
| s | 선택 호스트로 ssh |
| a | 호스트 새로 추가 |
| d | 삭제 (확인 모달) |
| t | 태그 편집 |
| f | 즐겨찾기(★) 토글 |
| v | `ssh -G <alias>` 해석 결과를 모달로 보여줌 (ssh가 실제로 어떻게 파싱했는지 — Include / Match 순서 디버깅에 유용) |
| c | 선택 호스트의 `ssh user@host -p port -i key` 한 줄 명령을 클립보드에 복사 (시스템 클립보드 실패 시 OSC 52 escape로 fallback — `SSHC_NO_OSC52`로 비활성화 가능) |
| g | TCP reachability 체크 — 해석된 hostname:port로 TCP 연결만 시도해 도달성/지연을 알려줌 |
| r | 모든 호스트의 TCP reachability를 한 번에 다시 검사. sshc는 시작할 때 + 편집 직후에만 자동 probe를 돌립니다(상시 대시보드가 아니라 picker라서); `r`는 그 사이의 명시적 새로고침. |
| e | 해당 호스트 줄에서 `$EDITOR` 열기 |
| M | 외부(`~/.ssh/config`) 호스트를 `sshc.conf`로 끌어오기 (원본 엔트리는 그대로 두므로 중복 `ssh -G` 매치가 싫다면 사용자가 직접 삭제) |
| S | 호스트 목록 정렬축을 순환 (별칭 → 최근접속 → 도달성). v0.12부터 `state.toml`에 저장돼 다음 세션이 직전 설정 그대로 시작합니다. |
| i | `~/.ssh/config`에 Include 줄 주입 |
| ? | 도움말 모달 |
| q, Esc | 종료 / 취소 |

폼 안에서는: Tab/Shift+Tab으로 필드 이동, Enter로 제출, Esc로 취소,
Ctrl+U로 현재 필드 비우기. v0.9에서 Tags와 Options 사이에 별도
`Forwarding` 섹션이 생겨 `LocalForward` / `RemoteForward` /
`DynamicForward`를 typed 필드로 입력할 수 있게 됐고, v0.10에서
forwarding 세 필드가 *list editor*로 동작하게 바뀌었습니다.

v0.12부터 같은 모달이 `IdentityFile` 행에도 적용됩니다 — OpenSSH가
호스트당 여러 키를 순서대로 시도하는 방식을 sshc도 list로 표현합니다.
v0.7.1의 `~/.ssh/*` 후보 키 picker(↑/↓로 detected 키 사이를 사이클)는
모달의 edit 모드 안으로 이동했습니다.

list-edit 모달 키맵:

| 키 | 동작 |
|---|---|
| ↑/↓ | 커서 이동 |
| Enter | 선택 항목 편집 (`+ add` 행에서는 빈 항목으로 시작) |
| d | 선택 항목 삭제 |
| Shift+↑/↓ | 선택 항목을 한 칸 위/아래로 재정렬 (v0.12 G2 — OpenSSH는 declaration 순서를 의미 있게 취급) |
| ↑/↓ (편집 중, IdentityFile 전용) | discovered `~/.ssh/*` 키 사이를 사이클 |
| Esc | 모달 닫기 (편집 중이면 편집 취소만) |

## 설치

### Homebrew (macOS + Linux)

```sh
brew install hang-in/tap/sshc
```

이후 `brew upgrade sshc`로 새 버전이 알아서 따라옵니다.

### 소스에서 cargo install

```sh
cargo install --git https://github.com/hang-in/sshc --tag v0.13.1
```

### 소스 빌드

```sh
git clone https://github.com/hang-in/sshc
cd sshc
cargo install --path .
```

Rust 1.85 이상 툴체인이 필요합니다.

### Windows

v0.7.0부터 네이티브 Windows 빌드를 함께 배포합니다. PowerShell
설치 스크립트 한 줄이면 끝나고, 직접 압축 파일을 받고 싶으면
[Releases](https://github.com/hang-in/sshc/releases) 페이지에서
Intel/AMD64는 `sshc-x86_64-pc-windows-msvc.zip`, Windows on ARM
(Snapdragon X / Surface Pro X / Dev Kit 2023)은
`sshc-aarch64-pc-windows-msvc.zip`을 내려받으면 됩니다. 두 ARM/x64
바이너리 모두 v0.8부터 배포됩니다.

```powershell
# PowerShell — %CARGO_HOME%\bin에 설치하고 PATH도 업데이트합니다
irm https://github.com/hang-in/sshc/releases/latest/download/sshc-installer.ps1 | iex
```

Windows 10 1809 이상 / Windows 11과 OpenSSH 클라이언트
(`설정 → 앱 → 선택적 기능 → OpenSSH Client`)가 필요합니다. 파일
권한 검사는 Windows ACL이 Unix 모드 비트와 다른 모델이라
0600/0700 확인을 생략하고 상위 디렉터리 상속에 맡깁니다.
ssh-agent 연동은 환경이 제공하는 것을 그대로 사용합니다 — Windows
OpenSSH agent든 Pageant든 — `SSH_AUTH_SOCK`은 보지 않습니다.
v0.8부터 `sshc --doctor`는 두 에이전트의 named pipe
(`\\.\pipe\openssh-ssh-agent`, `\\.\pipe\pageant`)를 직접 확인하므로,
`ssh-agent` 서비스가 멈춰 있으면 이전의 "not applicable" 안내가 아닌
`WARN`이 표시됩니다.

WSL2도 v0.6 때와 동일하게 그대로 동작합니다. Linux 빌드는 계속
배포되고, WSL 배포판 안에서는 macOS/Linux와 똑같이 동작합니다.

## 설정과 파일

sshc는 사용자가 손으로 쓴 `~/.ssh/config`는 절대 건드리지 않는 것을
원칙으로 합니다. 한 줄 `Include` 추가만 예외입니다.

- 관리 모드에서 추가한 호스트는 `~/.ssh/config.d/sshc.conf`(권한
  0600)에 따로 기록됩니다. 파일 첫 줄에 자동 주석이 붙어,
  손으로 편집하면 다음 저장 때 덮어쓰일 수 있다는 점을 표시합니다.
- 관리 모드 첫 실행 때 `~/.ssh/config` 끝에
  `Include ~/.ssh/config.d/sshc.conf`를 추가할지 묻습니다. 수락하면
  `.bak.sshc-YYYYMMDD` 백업을 만든 뒤 한 줄만 덧붙입니다. 거절하면
  관리 모드는 읽기 전용으로 동작합니다 (탐색·접속은 그대로).
- Include 선택 결과와 최근 접속 이력 같은 상태는
  `~/.config/sshc/state.toml`에 보관합니다.
- 태그는 각 `Host` 블록 바로 위에 `# @tags: a, b, c` 주석으로 들어가고,
  picker에서 `@<태그>`로 필터링할 수 있습니다 (예: `@prod`).

## 비슷한 도구와의 차이

`fzf` 스니펫을 직접 짜는 것보다는 친절하고, `storm`처럼
`~/.ssh/config`을 직접 손대는 도구보다는 안전하며, 본격 SSH GUI보다는
가벼운 도구입니다. **사용자가 손으로 작성한 `~/.ssh/config`은
sshc가 손대지 않는다는 약속**이 핵심입니다.

- **그냥 `ssh <alias>`**: 여전히 정답입니다. sshc는 그 위에 picker
  한 겹 얹는 도구일 뿐, ssh의 동작 자체는 바뀌지 않습니다.
- **[storm](https://github.com/emre/storm)**: Python으로 만든 CLI인데
  `~/.ssh/config`을 직접 편집합니다. sshc는 별도 파일(sshc.conf)에
  쓰고, `~/.ssh/config`에는 Include 한 줄만 추가합니다.

## 라이선스

MIT. 자세한 내용은 [LICENSE](LICENSE).

## 기여

이슈와 PR 모두 환영합니다. 24시간 안에 답 드리는 걸 목표로 하고
있습니다.
