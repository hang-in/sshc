# sshc v0.8.0 — Architect Brief

> Companion docs:
> - `PLAN_V0.8.md` — round breakdown (작성 예정)
> - `BRIEF_V7.md` §9 — Windows 잔여 deferral 항목
> - `BRIEF_V6.md` §9.2 anti-features — 그대로 carry-over
> - 직전 BRIEF: `BRIEF_V7.md`

## 1. Context

v0.7.0 (2026-05-20)에서 `x86_64-pc-windows-msvc` 네이티브 빌드와
PowerShell 설치 경로가 추가되며 sshc는 macOS / Linux / Windows를
동등한 1차 타겟으로 다루기 시작했다. v0.7.1 패치(2026-05-20)는
manage-mode `i` 메시지의 멱등성 분기와 add-host 폼 레이아웃 붕괴,
IdentityFile ↑/↓ 선택기를 정리하며 일상 사용 사이클을 안정화했다.

v0.7.2 / v0.7.3 핫픽스(2026-05-20)는 v0.7 단계에서 사실상 Windows
manage 모드 저장을 막고 있던 두 개의 직렬 버그를 닫았다 — (1)
IdentityFile 검증의 forbidden 문자 목록이 `\`를 포함해 Windows 경로
입력을 통째로 거절(0.7.2), (2) `storage::with_locked_write`의 lock
handle이 `fs::rename` 시점까지 살아 있어 Windows `MoveFileW`가
sharing violation으로 실패하고 사용자에게는 "그냥 저장이 안 됨"으로
보임(0.7.3). 두 버그가 동시에 깔려 있어서 0.7.0/0.7.1 출시 검증에서
는 첫 번째에서 막혀 두 번째에 도달조차 못 했고, 실제로 Windows에서
호스트 한 줄을 저장한 사용자는 v0.7.3가 처음이다. v0.8은 이 baseline
위에서 시작한다.

v0.7 단계에서 명시적으로 미룬 두 가지 항목이 v0.8의 출발점이다:

1. **Windows에서의 ssh-agent 상태 표시** — v0.7의 doctor는
   `SSH_AUTH_SOCK   PASS  Windows: not applicable (use Windows
   OpenSSH agent or Pageant)`에서 멈춰 있다. 실제로 에이전트가
   살아있는지는 보지 않는다.
2. **Manage 모드에서 외부(`~/.ssh/config`) 호스트 → `sshc.conf`로
   끌어오기** — 현재는 외부 호스트를 선택하면 `$EDITOR`로 떨어진다.
   사용자 결정(2026-05-20): "한 번에 가져와서 sshc 폼으로 편집할 수
   있게 하되, 원본 `~/.ssh/config` 엔트리는 사용자가 직접 정리한다."

여기에 v0.8 사이클에서 새로 합의된 항목 한 개가 더 붙는다:

3. **`sshc --doctor`에서 최신 버전 비교** — Homebrew는 사용자가
   `brew upgrade`를 돌리지 않으면 알아서 따라오지 않고, cargo /
   PowerShell installer 사용자는 더더욱 그렇다. v0.7.0 → 0.7.1 →
   0.7.2 → 0.7.3가 같은 날 4번 쌓여도 사용자가 모를 수 있다. doctor
   에 GitHub Releases `/repos/hang-in/sshc/releases/latest` 1회 호출
   기반의 버전 비교 줄 한 개를 추가한다 (anti-feature 4와의 정합은
   §3.3 참조).

v0.8은 **기능 확장 라운드**다. 플랫폼은 v0.7에서 닫았다 — 새 OS는
없다. 새로운 데일리 흐름 세 개를 깔끔하게 더한다.

## 2. v0.8 Goals

| # | Goal | Definition |
|---|---|---|
| G1 | doctor가 Windows에서 실제 에이전트 상태를 보고한다 | Pageant named pipe (`\\.\pipe\pageant`) 또는 Microsoft OpenSSH 에이전트 (`\\.\pipe\openssh-ssh-agent`)의 존재 여부를 점검. 존재하면 `PASS`, 둘 다 없으면 `WARN`. 식별자(identity) 열거는 없음. |
| G2 | manage 모드에서 외부 호스트를 sshc.conf로 promote | 외부 호스트가 선택된 상태에서 `M` 키 → 그 호스트의 필드를 모두 prefill한 add/modify 폼이 열림. 저장 시 `sshc.conf`에 새 엔트리가 기록되고, 원본 `~/.ssh/config` 엔트리는 **건드리지 않는다** (anti-feature 1 유지). 사용자에게는 "기존 엔트리는 직접 삭제하세요" 안내 메시지가 1회 표시된다. |
| G3 | doctor에서 최신 버전 비교 | `sshc --doctor`의 6개 체크에 한 줄 추가. GitHub Releases `latest` 엔드포인트를 *해당 명령 호출 시에만* 1회 호출, 현재 바이너리 버전과 비교. 같으면 `PASS  0.x.y (latest)`, 새 버전 있으면 `WARN  0.x.y (latest is 0.x.z)`. 평상시 `sshc` / `sshc -m` 시작 경로에는 네트워크 호출이 **없다** — anti-feature 4 정신을 깨지 않는다. 오프라인 / 호출 실패 시 `WARN  could not reach github (offline?)`로 진행 차단 없이 종료. |

Anti-features (`BRIEF_V7.md §9.2`)는 그대로 carry-over한다. v0.8에서
완화되는 안티피처는 없다 — 특히 anti-feature 1("Self-built SSH
client") 및 "사용자가 직접 작성한 `~/.ssh/config`는 절대 sshc가
수정하지 않는다" 정책은 G2 설계의 1순위 제약이다.

## 3. Goal-별 설계

### 3.1 G1 — Windows agent socket discovery

#### 3.1.1 분기 매트릭스 (`src/doctor.rs::check_ssh_auth_sock`)

| Platform | `SSH_AUTH_SOCK` set & non-empty | Behavior |
|---|---|---|
| Unix | yes | `PASS  set ({short_path})` (v0.7 유지) |
| Unix | no | `WARN  not set — ssh-agent identities won't be available` (v0.7 유지) |
| Windows | (env var는 무시) | `\\.\pipe\openssh-ssh-agent` 존재 시 `PASS  Windows OpenSSH agent pipe present` |
| Windows | (env var는 무시) | `\\.\pipe\pageant` 존재 시 `PASS  Pageant pipe present` |
| Windows | (env var는 무시) | 둘 다 검출되지 않음 → `WARN  no agent pipe found (start Windows OpenSSH agent service or Pageant)` |

둘 다 살아 있으면 `PASS  Windows OpenSSH agent + Pageant present`로
한 줄로 합친다. 점검 이름은 `SSH_AUTH_SOCK`을 유지하되 Windows
브랜치에서는 `agent` 라벨로 detail에 명시 — 라벨 자체는 doctor가
열거하는 6개 체크의 키 식별자이므로 외부 자동화가 grep할 수 있게
변경하지 않는다.

#### 3.1.2 Named pipe 존재 검사

`std::path::Path::new(r"\\.\pipe\openssh-ssh-agent").exists()`는
named pipe에 대해 신뢰할 수 없다 — `CreateFileW` + `CloseHandle`
시퀀스가 정공법이다.

후보 두 가지:

(a) `windows-sys`의 `CreateFileW` / `CloseHandle`로 직접 호출. 이미
v0.7에서 `windows-sys 0.59` 의존성이 들어가 있고 `LockFileEx`를
부르고 있으니 추가 의존성 없음.

(b) 더 가벼운 `std::fs::OpenOptions::new().read(true).open(path)`로
시도하고 `Err(NotFound)` 분기를 false로 처리. Windows에서 named pipe
경로에 대한 표준 라이브러리 동작은 OpenSSH 에이전트 측의 ACL에
의존적이므로 권장하지 않음.

**v0.8 선택**: (a). `src/doctor.rs`에 `#[cfg(windows)] fn
windows_agent_pipe_present(path: &str) -> bool`를 둔다. 단순히
`CreateFileW(path, GENERIC_READ, 0, NULL, OPEN_EXISTING,
FILE_ATTRIBUTE_NORMAL, NULL)` 한 번 부르고 `INVALID_HANDLE_VALUE`인지
검사한 뒤 핸들을 닫는다.

#### 3.1.3 테스트 전략

- Unix: 추가 테스트 없음. 기존 `check_ssh_auth_sock` 분기는 그대로.
- Windows: 단위 테스트로 named pipe 존재 검출 함수만 mockable한
  형태로 분리하고, 실제 통합 검증은 R-G9 매뉴얼 매트릭스에서 수동
  (`Get-Service ssh-agent` start/stop 토글) 으로 확인.

### 3.2 G2 — Promote external host → sshc.conf

#### 3.2.1 사용자 흐름

1. 사용자가 manage 모드 (`sshc -m`) 진입.
2. 외부(`~/.ssh/config`) 출처의 호스트를 선택. 현재 v0.7.1까지는
   `Enter`가 `$EDITOR`로 fallback (input.rs:194 `AppAction::EditConfig`).
3. 새 단축키 **`M`** 또는 **`Shift+m`** — 외부 호스트일 때만 활성화.
   관리되는 호스트(`sshc.conf`)에서 누르면 status bar에
   `'<alias>' already managed by sshc.conf` 1회 출력 후 무시.
4. `M`이 활성화되면 기존 add/modify 폼이 외부 호스트의 모든 필드
   (Host alias, HostName, User, Port, IdentityFile, ProxyJump,
   ForwardAgent, 기타 v0.6/v0.7 노출 필드 — `config::model`에서
   덤프 가능한 모든 필드)로 prefill된 상태로 열린다.
5. 저장 (`Ctrl+S` 등 현 폼 단축키) 시:
   - `sshc.conf`에 새 호스트 엔트리가 기록된다 (writer의 잠금/멱등
     기존 코드 재사용).
   - **원본 `~/.ssh/config` 엔트리는 sshc가 절대 손대지 않는다.**
6. status bar에:
   `'<alias>' promoted to sshc.conf — original ~/.ssh/config entry left intact, delete it manually if duplicate ssh -G output bothers you`

   메시지가 길지만 한 번이고, manage-mode `i` 멱등 메시지 패턴
   (v0.7.1)과 동등한 수준의 명시성으로 의도. 한 줄이 status bar 폭을
   넘으면 줄임 처리(`StatusMessage::new` 기존 동작) 허용.

#### 3.2.2 충돌 케이스

- 같은 alias가 이미 `sshc.conf`에 있는 경우: 폼 prefill 시점에 감지
  → status bar `'<alias>' already exists in sshc.conf — promote
  aborted` 출력하고 폼은 열지 않는다.
- alias가 와일드카드(`*`, `?`, 패턴) 인 경우: v0.8에서는 promote
  거부. status bar `wildcard alias '<alias>' cannot be promoted —
  sshc only manages explicit aliases`. (anti-feature 5: complete
  `config(5)` parser 안 한다는 입장과 일관.)
- `Include`된 외부 파일에서 온 호스트: 출처가 `~/.ssh/config`가
  아니더라도 "external" 분류이면 동일한 흐름. promote 결과는 항상
  `sshc.conf`로만 간다.

#### 3.2.3 영향 받는 모듈

| 파일 | 변경 |
|---|---|
| `src/app/input.rs` | `M` 키 바인딩 추가. external 분기에서만 활성화. activate_selected와 동일 패턴의 `promote_selected()` 메서드. |
| `src/app/forms.rs` | `open_modify_form`에 "원본 source != sshc.conf인 외부 호스트로부터의 promote" 모드 진입점 추가. 저장 경로는 신규 add와 동일 (writer → sshc.conf). |
| `src/app/mod.rs` | 폼 상태에 `promote_origin: Option<PathBuf>` 추가 (저장 시 status 메시지에 출처 표시용). |
| `src/ui/status_bar.rs` | promote 완료 / 중단 / 충돌 메시지 분기. |
| `src/ui/list.rs` 또는 footer hint | external 호스트가 선택되어 있을 때 `Enter open in $EDITOR • M promote to sshc.conf` 안내. |
| `src/config/model.rs` | (확인 필요) 호스트의 모든 필드를 폼 prefill 가능한 형태로 노출하는 게 이미 되어 있는지 — 없는 필드가 있으면 추가. |

`src/storage/` 와 `src/setup/` 는 손대지 않는다 — promote는 결국
"add 폼을 prefill된 상태로 열기"와 같으므로 저장 경로는 v0.7.1까지의
add 경로 그대로다.

#### 3.2.4 테스트

- 단위:
  - `forms::prepare_promote(host)` → prefill된 폼 상태 검증.
  - alias 충돌 / wildcard 거부 분기 각각.
- 통합 (`tests/round_trip_test.rs` 패턴):
  - 외부 호스트 1개 + sshc.conf 비어있음 → promote → sshc.conf에
    해당 alias 등장, 원본 `~/.ssh/config` byte-level 동일 보장.
- R-G 매트릭스 추가 없이 기존 R-G1..R-G9 통과 + 새 통합 테스트로
  cover.

### 3.3 G3 — doctor 업데이트 체크

#### 3.3.1 정합성 확인 (anti-feature 4)

anti-feature 4는 "Web UI / daemon / always-on process"다. 핵심은
*사용자가 의도하지 않은* 백그라운드 네트워크 / 프로세스 유지. v0.8
G3는:

- 호출 경로가 **doctor 한 가지로 한정** — 평상시 `sshc`, `sshc -m`,
  `sshc <alias>` 어디서도 네트워크 호출이 발생하지 않는다.
- doctor는 *원래* 외부 상태(ssh 바이너리, ~/.ssh 권한, Include 라인)
  를 조사하는 명령이다. "최신 릴리스가 있는지"도 같은 부류의
  외부 상태이므로 doctor의 책임 안에 자연스럽게 들어간다.
- 한 번 호출에 HTTP 한 번. 캐시 / 백그라운드 / 데몬 / 항상-켜진
  프로세스 일체 없음.

이 셋이 모두 충족되므로 anti-feature 4는 깨지지 않는다. 만약
v0.9+에서 "시작 시 자동 체크"가 거론되면 그 때는 다시 §3.3.1 기준
으로 검토한다. 현 단계에서는 *그쪽으로 가지 않는다*.

#### 3.3.2 출력 매트릭스

| 상황 | Status | detail |
|---|---|---|
| 응답 OK, latest == current | PASS | `0.8.0 (latest)` |
| 응답 OK, latest > current | WARN | `0.8.0 (latest is 0.x.y — see https://github.com/hang-in/sshc/releases/latest)` |
| 응답 OK, latest < current (개발 빌드) | PASS | `0.9.0-dev (ahead of latest 0.8.0)` |
| 네트워크 실패 / 타임아웃 / 4xx / 5xx | WARN | `could not reach github (offline?)` |
| 파싱 실패 (응답 형식 변경) | WARN | `unexpected response from GitHub releases` |

체크 이름은 `update`로 고정. `--doctor`가 출력하는 6개 체크의
마지막 줄에 추가.

#### 3.3.3 구현 메모

- HTTP 클라이언트: `ureq` 한 개. 사유:
  - `reqwest`는 `tokio` + `hyper`를 끌고 와서 sshc 같은 동기 TUI
    바이너리에는 무겁다.
  - `ureq`는 동기, ~120KB stripped, 의존성이 얕다 (`rustls` 또는
    `native-tls` 양자택일).
  - `native-tls`로 가면 시스템 TLS(SecureTransport / SChannel /
    OpenSSL)를 그대로 쓰므로 추가 인증서 번들 불필요.
- JSON 파싱: GitHub `latest` 응답의 `tag_name` 한 필드만 본다.
  `serde_json`을 끌지 않고 직접 정규식 또는 `tag_name` substring
  탐색으로 가져온다 (응답이 항상 객체이므로 안전).
- 타임아웃: connect 2s + read 3s. 도합 5초 이내. doctor가 5초 더
  걸려도 사용자에게 큰 무게는 아니지만 그 이상은 답답하다.
- 버전 비교: `tag_name` 앞의 `v`를 strip 후 `env!("CARGO_PKG_VERSION")`
  와 lexicographic 비교 가능. SemVer pre-release(`0.8.0-dev`,
  `0.8.0-rc.1`)는 등장 시점에 다시 본다 (현재까지 sshc는 dot-triple만
  쓴다). 외부 `semver` 크레이트는 도입하지 않는다.
- 환경변수 escape hatch: `SSHC_NO_UPDATE_CHECK=1`이 설정되면 G3
  체크 자체를 건너뛰고 `PASS  update check skipped (SSHC_NO_UPDATE_CHECK)`.
  사내망 / 폐쇄망 사용자가 매번 `WARN`을 보지 않도록 한다.

#### 3.3.4 테스트

- 단위:
  - 버전 비교 함수 (`compare_versions("0.7.3", "0.8.0") == Greater`
    등) — 5개 분기.
  - `tag_name` 추출 함수 — GitHub 실제 응답 샘플 fixture로.
- 통합: 네트워크 호출은 mocking. `mockito` 또는 직접 `127.0.0.1`로
  `std::net::TcpListener` 띄워 응답 fixture 반환.
- 매뉴얼: `SSHC_NO_UPDATE_CHECK=1 sshc --doctor`, 인터넷 끊고
  `sshc --doctor` 각각.

## 4. Module-boundary R-G gates

신규 R-G 게이트 없음. v0.6에서 굳어진 9개 게이트가 그대로 유효하다:

- R-G7 (manage 모드 키 바인딩 표): `M` 추가 시 표 동기화.
- R-G2 (writer 단일 진입점): promote 저장은 기존 add 경로 재사용 —
  새 writer 진입점 만들지 말 것.
- R-G8 (anti-feature 1: `~/.ssh/config` 비파괴): G2의 핵심 제약.

`docs/TESTING.md §2` 매트릭스는 v0.7.0과 동일하게 굴린다.

## 5. New dependencies

G1, G2는 추가 의존성 없음 — v0.7의 `windows-sys 0.59`로 G1 충족,
G2는 순수 in-process / 기존 storage 경로.

G3는 다음 둘이 들어온다 (양쪽 모두 무조건 의존성, target-gated가
아님 — Unix / Windows 양쪽에서 doctor 가 실행되므로):

| Crate | Purpose | Version | Notes |
|---|---|---|---|
| `ureq` | 동기 HTTP 클라이언트 (GitHub `releases/latest` 호출) | `2.10` (실측 2.12) | default features 그대로 — rustls + webpki-roots. 처음에는 `native-tls`로 빌드 시도했지만 ureq 2.12에서 native-tls만 enable해도 런타임 TLS backend가 잡히지 않아 단념. |
| (none) | JSON 파싱은 자체 처리 | — | `serde_json` 도입 없이 `tag_name` substring 추출. |

**실측 사이즈 영향** (macOS arm64 release):
- baseline v0.7.3: 3,150,128 bytes (~3.0MB)
- v0.8 R6 후: 5,246,752 bytes (~5.0MB) — +2.1MB 증가.

PLAN_V0.8 §7 DoD에 적힌 "+500KB 이내" 한도는 native-tls 경로를
가정한 추정이라 실제와 어긋났다. v0.8은 이 사이즈를 받아들이고
넘어간다. SSHC_NO_UPDATE_CHECK escape hatch가 있고, 절대값
(~5MB)은 동급 TUI 도구 분포(htop ~7MB, kubectl ~50MB) 안이라
사용자 가치를 깨는 수준은 아니다. v0.9에서 `attohttpc` / `minreq`
+ rustls-native-roots 조합으로 사이즈 회수 시도 — Risks 표에
명시 (§6).

## 6. Risks

| Risk | Mitigation |
|---|---|
| Windows OpenSSH 에이전트 서비스가 stopped인데 named pipe만 잔존 — false PASS | `CreateFileW`가 `ERROR_FILE_NOT_FOUND` 또는 `ERROR_PIPE_NOT_CONNECTED`를 모두 미존재로 매핑. 살아있지 않으면 open에 실패 — 핸들 검사로 충분. |
| Pageant pipe 이름이 PuTTY 버전별로 다른 사례 | 0.78 이후로 `\\.\pipe\pageant`가 안정 — 그 외 이름은 supported 아님. doctor 메시지에 명시. |
| promote 후에도 `~/.ssh/config`에 원본이 남아 `ssh -G`가 중복 매치를 토해냄 | 의도된 동작. status 메시지에 그대로 명시. anti-feature 1을 깨면서 자동 삭제하지 않는다. |
| 외부 호스트가 `Include` 체인 깊숙이 있고 출처 파일이 sshc-쓰기 가능한 디렉터리인 경우 사용자가 "왜 안 지워주냐"고 오해 | manage UI 푸터의 `M` hint 옆에 `(original kept)` 짧은 단서를 함께 표기. |
| `M` 키가 기존 어떤 동작과 충돌 | v0.7.1 시점 manage-mode 키맵 확인: `i / v / Enter / f / d / r / q / ↑↓ / / / Tab`. 대문자 `M`은 비어 있음 — 안전. |
| Windows에서 PuTTY 미설치 환경의 Pageant 경로가 비어 있어도 doctor가 PASS — 사용자 혼란 | "no agent pipe" → `WARN`. Doctor의 다른 체크와 같이 `WARN`은 실행을 막지 않음 (`run()` 반환값 매트릭스 변경 없음). |
| G2가 form prefill 시 v0.7에서 추가된 IdentityFile 후보 선택기와 충돌 | promote는 폼 열기 직전에 IdentityFile 값을 명시 설정 — 후보 리스트는 사용자가 ↑/↓로 재선택 가능. 충돌 없음. |
| G3 doctor가 폐쇄망에서 매번 WARN을 띄워 사용자가 둔감해짐 | `SSHC_NO_UPDATE_CHECK=1` escape hatch로 PASS-skip 으로 전환. README에 명시. |
| G3 ureq 도입으로 `cargo install`이 OpenSSL/SChannel 헤더를 요구해 새 환경에서 실패 | R6 실측 결과 native-tls 경로가 ureq 2.12에서 TLS backend를 잡지 못해 단념하고 default(rustls + webpki-roots)로 갔다 → 시스템 라이브러리 의존성 0, 어떤 환경에서도 빌드 성공. 대가는 +2.1MB 사이즈. |
| G3로 인한 사이즈 증가가 후속 라운드에서 사용자 불만으로 돌아옴 | `attohttpc` / `minreq` + `rustls-native-roots` 조합으로 v0.9에서 사이즈 회수 시도. PLAN_V0.9 시드 항목으로 미루기. 절대값(~5MB)이 동급 TUI 분포 안이라 v0.8 단계에서는 차단 요소 아님. |
| 동일 태그 푸시에 cargo-dist Release 워크플로우가 중복 트리거되어 한쪽이 `release already exists`로 실패 (v0.7.2 시점 관측) | R0에 `.github/workflows/release.yml`의 `jobs.*` 위에 `concurrency: { group: release-${{ github.ref }}, cancel-in-progress: false }` 추가. 늦은 워크플로우는 큐잉되어 첫 번째가 끝난 뒤 동일 태그를 보고 no-op 종료. |

## 7. Definition of Done

- [ ] `cargo check --target x86_64-pc-windows-msvc` 클린.
- [ ] `cargo test` 클린 (Unix host + Windows 비-권한 경로).
- [ ] `cargo clippy --all-targets -- -D warnings` 양 플랫폼 클린.
- [ ] `cargo fmt --check` 클린.
- [ ] R-G1..R-G9 클린.
- [ ] 신규 통합 테스트: external → promote → `sshc.conf` 등장 +
      `~/.ssh/config` byte-equal 보존.
- [ ] doctor의 Windows 분기 매뉴얼 검증: 에이전트 서비스 stop/start
      각각에 대해 `PASS` ↔ `WARN` 토글.
- [ ] doctor의 update 체크 매뉴얼 검증: 정상 / 오프라인 /
      `SSHC_NO_UPDATE_CHECK=1` 세 경로.
- [ ] 버전 비교 함수 단위 테스트 5개 분기 모두 통과.
- [x] `cargo build --release` 사이즈 측정. ureq + rustls 도입으로
      ~+2.1MB 증가 — DoD의 "+500KB 이내" 추정은 native-tls 가정이라
      실측과 어긋났고, v0.8에서는 받아들이고 R6 커밋 메시지 + §5에
      사실 명시. v0.9 risk로 사이즈 회수 항목 등재.
- [ ] manage 모드 매뉴얼 검증: external 호스트 선택 → `M` →
      prefill된 폼 → 저장 → list가 sshc.conf 출처로 갱신, status
      메시지 출력, 원본 `~/.ssh/config` 미변경.
- [ ] manage 모드 매뉴얼: managed 호스트에서 `M` 누르면 무해한 안내
      메시지만 출력.
- [ ] `.github/workflows/release.yml`에 concurrency 가드가 있고,
      v0.8.0 push 시 워크플로우가 한 번만 떴다는 사실을 `gh run list`
      로 확인.
- [ ] README + README.ko §"manage 단축키", §"doctor 출력" 갱신.
- [ ] CHANGELOG `[0.8.0]` 엔트리 (Added: Windows agent pipe
      detection; Added: manage-mode `M` promote external host;
      Added: doctor update check + `SSHC_NO_UPDATE_CHECK`).
- [ ] `main.rs`는 여전히 ≤ 10 LOC (R-G0).

## 8. Round breakdown (PLAN_V0.8 시드)

| R | 내용 | 게이트 |
|---|---|---|
| R0 | 브랜치 확보, `git pull --ff-only`, R-G1..R-G9 재실행으로 baseline 확인. `.github/workflows/release.yml`에 `concurrency: { group: release-${{ github.ref }}, cancel-in-progress: false }` 추가. | 기존 매트릭스 통과 + workflow yaml lint |
| R1 | G1: `check_ssh_auth_sock` Windows 분기에 named pipe 검출 추가 + `windows_agent_pipe_present` 헬퍼 | Unix 분기 동작 무변경, Windows 단위 테스트 |
| R2 | G1: doctor 메시지 / detail 문자열 / README 반영 | 매뉴얼 PASS/WARN 토글 |
| R3 | G2: external 선택 상태에서 `M` 키 라우팅 + footer hint 갱신 | clippy / fmt, 안내 메시지 단위 테스트 |
| R4 | G2: 폼 prefill 경로 (`prepare_promote`) + 저장 시 sshc.conf 경로 + alias 충돌 / 와일드카드 거부 | 단위 + 통합 round_trip 테스트 |
| R5 | G2: status bar 메시지 3종 분기 (성공 / 충돌 / wildcard) + UI 푸터 동기화 | UI 단위 테스트 (status_message 매처) |
| R6 | G3: `ureq` 의존성 추가 + `check_latest_version()` + `SSHC_NO_UPDATE_CHECK` 분기 + 5개 단위 테스트 | 단위 테스트 + 매뉴얼 (정상 / 오프라인 / skip) |
| R7 | 문서 (README, README.ko, CHANGELOG `[0.8.0]`), 버전 bump `0.7.3 → 0.8.0` | Definition of Done 전 항목 |
| R8 | 릴리스 (tag `v0.8.0`, push, cargo-dist 트리거, Homebrew 탭 갱신 확인). R0의 concurrency 가드가 실제로 한 번만 띄우는지 `gh run list`로 검증. | GitHub Release 아티팩트 9종 + 단일 워크플로우 트리거 확인 |

## 9. Out of scope

### 9.1 v0.9+로 미루는 항목

- **Windows ARM64 (`aarch64-pc-windows-msvc`)** — 러너 부재로 v0.7
  단계에서 미룸. v0.8에서도 그대로. cargo-dist `targets` 한 줄
  추가건은 러너 확보 시 수분.
- **Windows ACL 검사 강화** — "private key file은 읽기 권한이
  최소화되어야 한다" 같은 정책의 Windows 등가물. v0.7 §9.1에서
  미룬 항목 그대로.
- **Identity 열거 / 에이전트 식별자 표시** — G1은 *presence만* 본다.
  `ssh-add -l` 호출이나 named pipe 위로 SSH 에이전트 프로토콜
  메시지를 직접 보내 식별자를 enumerate하는 짓은 anti-feature 2
  (Secret / key management) 와 anti-feature 1 (Self-built SSH client)
  양쪽에 걸려 있다. 하지 않는다.
- **원본 `~/.ssh/config` 자동 정리** — G2에서 promote 후에도 원본
  엔트리는 손대지 않는다. 자동 삭제는 anti-feature 1을 깬다. 사용자
  요청이 누적되면 v0.9+에서 별도 명령(`sshc clean --dry-run`)으로
  재검토하되, 기본은 항상 비파괴.
- **시작 시 자동 업데이트 체크** — G3는 *doctor 한정* 호출이다.
  매 `sshc` / `sshc -m` 실행마다 GitHub에 요청하는 건 anti-feature 4
  ("always-on process") 정신과 어긋난다. 매번 체크가 필요해진다면
  그 시점에 다시 §3.3.1 기준으로 재검토하되 현재는 doctor에 가둔다.
- **자동 다운로드 / 자동 업데이트** — G3는 "있다고 알린다"까지만
  한다. sshc 자신이 새 바이너리를 받고 설치하는 자동 업데이트
  메커니즘은 보안/책임 측면에서 별도 설계가 필요 — 도입한다면
  Homebrew / cargo-dist installer에 위임하는 게 정공법이고 sshc
  내부에서 처리하지 않는다.

### 9.2 Project-wide anti-features (BRIEF_V7 §9.2 carry-over)

1. Self-built SSH client.
2. Secret / key management (passwords, keyfile content).
3. Team-shared catalogs, permissions, audit logs.
4. Web UI / daemon / always-on process.
5. Complete OpenSSH `config(5)` parser.

v0.8에서 어느 것도 완화하지 않는다.

## End of Brief.
