# sshc v0.9.0 — Architect Brief

> Companion docs:
> - `PLAN_V0.9.md` — round breakdown
> - `BRIEF_V8.md` §9 — Windows / G3 잔여 deferral 항목
> - `BRIEF_V6.md` §9.2 anti-features — 그대로 carry-over
> - 직전 BRIEF: `BRIEF_V8.md`
> - 외부 비교 자료: `Adembc/lazyssh` (Go, 3.6k⭐) — UX 참고

## 1. Context

v0.8 사이클은 macOS / Linux / Windows 1차 지원과 manage-mode 두 가지
신규 동선(`M` promote, doctor update check)을 마무리했다. 사이클 끝에
v0.8.4 hotfix가 v0.4부터 잠재해 있던 `inject_include`의 conditional
include 트랩(append된 Include가 마지막 Host stanza의 child가 되어
모든 sshc-managed 호스트가 invisible)을 닫았다.

v0.9는 두 축으로 간다:

1. **운영 견고성 강화 (doctor + UX 잔여)** — v0.8 cycle에서 디버깅
   사이클을 길게 만들었던 묵묵한 실패 mode들을 doctor + status_message로
   surface한다. CRLF, nested-Include, modal-redraw가 status를 덮는
   문제.
2. **UX 비교 검토 후 선택적 도입** — `Adembc/lazyssh`(가장 활발한
   Go 구현, 3.6k⭐)와의 비교에서 발견한 *anti-feature와 충돌하지 않는*
   세 가지를 가져온다: `c` copy ssh command, Forwarding 폼 섹션,
   TCP reachability 체크. anti-feature 1+2를 깨는 SCP/키 배포는
   *명시적으로 거부*한다 — sshc의 정체성과 정면 충돌.

플랫폼은 v0.8에서 닫았다 — Windows ARM64 한 가지가 v0.8 carryover
로 남아 있고, 사이즈 회수(`attohttpc`/`minreq` 평가)도 운영 견고성
관점에서 v0.9에 정리한다.

Anti-features (`BRIEF_V8.md §9.2`)는 그대로 carry-over한다. v0.9에서
완화되는 안티피처는 없다.

## 2. v0.9 Goals

| # | Goal | Definition |
|---|---|---|
| G1 | doctor가 ~/.ssh/config의 CRLF를 감지 | macOS/Linux에서 Windows-origin config 복사 후 흔히 발생. OpenSSH가 alias 토큰에 `\r`을 포함시켜 모든 매칭이 묵묵히 깨짐. doctor에 `[WARN] line endings  CRLF detected — OpenSSH treats \r as part of alias tokens; run \`tr -d '\\r' < … > …\`` 추가. read-only — anti-feature 1 무관. |
| G2 | doctor가 sshc-managed Include의 nested 여부를 감지 | v0.8.4 이전 install이거나 사용자가 직접 ~/.ssh/config를 편집한 후 Include 위치가 깨진 경우. doctor가 main config를 line-by-line으로 훑어 `Include …/sshc.conf`가 *마지막 Host/Match directive 다음*에 등장하는지 확인. nested면 `[WARN] Include scope  nested inside Host '<alias>' — sshc hosts only fire for that alias; add 'Match all' above or re-inject via 'i'`. |
| G3 | status_message가 form-close redraw에 묻히지 않게 sticky | `apply_form`이 Err 시 status_message를 set하지만 form modal 닫힘 직후 list redraw가 그것을 덮어 사용자에게 안 보임 (v0.7-v0.8 cycle의 Windows save fail에서 정확히 surface). StatusMessage에 `kind: Error/Info` 필드 추가, Error는 *다음 keystroke까지 sticky*. Info는 v0.6 transient 동작 유지. |
| G4 | `c` 키 — 선택된 호스트의 ssh 명령을 클립보드에 복사 | `lazyssh` UX 참고. `ssh <user>@<hostname> -p <port> -i <identity>` 같은 한 줄 출력. 클립보드 라이브러리: `arboard` (cross-platform, X11/Wayland/macOS/Windows). 복사 후 status_message로 `copied: ssh d9ng@…`. |
| G5 | Forwarding 폼 섹션 — LocalForward/RemoteForward/DynamicForward 전용 입력 | 현재 sshc는 ProxyJump/LocalForward 같은 directive를 `Options:` 자유 입력에 몰아넣는다. 일상적으로 forwarding 쓰는 사용자에게는 어색한 UX. lazyssh의 *tabbed form*에서 영감: sshc는 *섹션 헤더 + 3개 추가 필드*(LocalForward, RemoteForward, DynamicForward) — tab 도입 없이 단일 폼의 *행 수만 늘림*. validate는 OpenSSH가 받는 형식(`[bind:]port:host:hostport`) 정도만 정규식. |
| G6 | TCP reachability 체크 (`g` 또는 별도 키) | `ssh -G`로 hostname/port resolve 후 그 endpoint에 TCP connect를 2초 안에 시도. 성공 → `✓ TCP reach: hostname:port (320 ms)`, 실패 → `✗ TCP unreachable: …`. **anti-feature 1과의 정합**: 실제 SSH handshake가 아닌 *연결 가능성 만* 본다 — 자체 SSH client가 아니므로 OK. lazyssh의 `g` ping과 의미적 분리: `v`는 *config resolve*, `g`는 *reachability*. |
| G7 | 사이즈 회수 평가 | v0.8.0 ureq+rustls 도입으로 +2.1MB. attohttpc + rustls-native-roots, minreq, ureq+native-tls 재시도 — 셋을 *cargo bloat*로 측정하고 최저 후보로 교체. 목표: v0.8.4 대비 -500KB 이상. 회수 실패 시 BRIEF로 명시하고 그대로 ship. |
| G8 | Windows ARM64 cargo-dist 타겟 추가 | `dist-workspace.toml`에 `aarch64-pc-windows-msvc` 한 줄 추가. 실제 빌드는 cargo-dist Windows-ARM64 runner. CI에 ARM64 runner 가용성 확인이 prerequisite — *2026-06 기준 GitHub Actions에 native ARM64 Windows 러너 있음*. 빌드 검증은 cargo-dist run 시점에 처음. |

Anti-features (`BRIEF_V8.md §9.2`)는 그대로 carry-over한다 — 특히
anti-feature 1 (Self-built SSH client) + 2 (Secret / key management)
는 lazyssh가 가려는 SCP / 키 배포 방향과 *정면 충돌*. v0.9는 lazyssh의
UX 영감만 차용하고 그 두 방향은 *명시적으로 거부*한다 (§9.1 참조).

## 3. Goal-별 설계

### 3.1 G1 — doctor CRLF 감지

#### 3.1.1 동작

`src/doctor.rs`에 새 체크 `check_main_config_line_endings()` 추가.
~/.ssh/config을 read하고:

- 첫 100 라인 안에서 `\r\n`이 한 번이라도 발견되면 → `WARN`.
- 발견 안 되면 → 체크 자체를 *생략* (PASS 라인을 추가하지 않음).

```
[WARN] line endings  CRLF detected in ~/.ssh/config — OpenSSH
                     treats '\r' as part of alias tokens; run
                     `tr -d '\r' < ~/.ssh/config > .tmp && mv …`
```

#### 3.1.2 정합성

- *read-only*. anti-feature 1 무관.
- doctor의 다른 체크와 동일 패턴 (Status::Warn).
- run() 반환값 영향 없음 (WARN은 exit-0).

### 3.2 G2 — doctor nested-Include 감지

#### 3.2.1 동작

`check_include_scope()` 추가. ~/.ssh/config을 line scan:

1. sshc-managed Include 라인의 line number 확인 (`is_include_present`
   로직 재사용 + 라인 번호 추출).
2. 그 위로 거슬러 올라가 마지막 `Host` 또는 `Match` directive를 찾음.
3. 마지막 directive가 `Host …` (any pattern except `*`)인 경우 →
   nested. `Match all` 또는 `Host *`인 경우 → unconditional.

```
[WARN] Include scope  nested inside Host 'foo' (line 74) — sshc
                      hosts only fire for that alias; add
                      'Match all' directly above the Include or
                      re-inject via 'sshc -m' -> 'i' on v0.8.4+
```

#### 3.2.2 보충 — v0.8.4 carryover와의 관계

v0.8.4가 *새로 inject되는* Include만 fix. 기존 install들은 fix 안 됨.
G2는 그 사용자들을 doctor 한 줄로 surface — *자동 수정은 하지 않는다*
(anti-feature 1).

### 3.3 G3 — status_message sticky on Error

#### 3.3.1 현 동작

`StatusMessage::new("…")`로 만든 메시지는 모두 *transient* —
v0.6에서 3초 timer + 다음 redraw 시 갱신 가능. `apply_form`이
`Err(StorageError::…)`을 받았을 때도 같은 transient를 set하고
form modal 닫힘. 닫힘 redraw가 status_bar를 다시 그리고, 새 메시지가
없으면 *transient도 즉시 사라지는 경로*가 있음 (정확한 위치는
`src/ui/status_bar.rs` 또는 `src/app/mod.rs`의 message expiry).

v0.8 cycle 디버깅이 길어진 핵심 — `with_locked_write`가 Err 반환,
`apply_form`이 status set, form 닫히면서 사용자가 메시지를 못 봄.

#### 3.3.2 설계

```rust
pub struct StatusMessage {
    text: String,
    kind: StatusKind,        // 새 필드
    created_at: Instant,
}

pub enum StatusKind {
    Info,    // v0.6 transient 동작 — 3초 후 자동 사라짐
    Error,   // 다음 keystroke까지 sticky
}
```

- `StatusMessage::error(text)` 생성자 추가.
- `apply_form`의 Err 분기에서 `StatusMessage::error(…)` 사용.
- `is_visible()` 또는 expiry 검사에서 `kind == Error`면 *시간 검사
  생략*.
- 다음 키스트로크 처리 시 `Error` 메시지는 *명시적으로 clear*
  (status_bar 새 메시지 set 또는 사용자 명시 dismiss).

#### 3.3.3 영향 범위

- 기존 호출자 (~/.ssh/config 권한, persist 실패, validation 등)
  중 *진짜 Err 메시지*만 `StatusMessage::error(…)`로 전환. 안내성
  메시지(`'<alias>' pinned` 등)는 그대로 transient.
- v0.6의 `f`/`v`/`i` 등 정상 동작 메시지는 변경 없음.

### 3.4 G4 — `c` copy ssh command

#### 3.4.1 동작

manage 모드에서 호스트 선택 후 `c` → `ssh -G <alias>` 출력에서
hostname/port/user/identityfile 추출 → 한 줄 명령 구성:

```
ssh d9ng@yongseek.iptime.org -p 2232 -i ~/.ssh/id_ed25519
```

`arboard::Clipboard::new()?.set_text(&cmd)?` → 클립보드에 복사.
status: `copied: ssh d9ng@yongseek.iptime.org -p 2232 -i …`.

inline 모드: 키 무시 (인라인은 read-only browser, R-G9).

#### 3.4.2 의존성

`arboard = "3"` (cross-platform clipboard, X11/Wayland/macOS/Win).
~+150KB 사이즈 영향 — G7 사이즈 회수와 함께 측정.

#### 3.4.3 키맵 충돌 검사

v0.8.4 시점 manage-mode 키: `j/k/Enter/s/a/d/t/f/v/M/e/i/?/q/Esc`.
`c` 비어있음 — 안전.

### 3.5 G5 — Forwarding 폼 섹션

#### 3.5.1 폼 레이아웃

현 v0.8.4 host form (7 필드, one-row-per-field):
```
Alias  HostName  User  Port  IdentityFile  Tags  Options
```

v0.9는 **3 필드 추가 + 섹션 헤더**:
```
─── Identity ───
Alias  HostName  User  Port  IdentityFile  Tags

─── Forwarding ───
LocalForward  RemoteForward  DynamicForward

─── Advanced ───
Options
```

섹션 헤더는 입력 불가능한 dimmed 라벨. Tab/Shift+Tab은 섹션을
건너뛰며 입력 필드만 순회.

#### 3.5.2 직렬화

각 forwarding 필드가 비어있지 않으면 sshc.conf에 별도 라인으로:
```
LocalForward 8080 localhost:80
RemoteForward …
DynamicForward 1080
```

#### 3.5.3 validation

각 필드별로 OpenSSH가 받는 *느슨한* 형식 정규식만:
- LocalForward: `^\d+(:\S+)?\s+\S+:\d+$`
- RemoteForward: 동일
- DynamicForward: `^\d+$` 또는 `^\d+:\d+$`

세부 검증은 ssh 자체에 맡김 (anti-feature 5: 완전한 OpenSSH parser
안 한다).

### 3.6 G6 — TCP reachability

#### 3.6.1 동작

manage 모드에서 호스트 선택 후 `g` →
1. 기존 `ssh -G <alias>` 캐시 hit/miss 확인.
2. hostname/port 추출.
3. `std::net::TcpStream::connect_timeout(addr, Duration::from_secs(2))`.
4. 성공: `✓ TCP reach: hostname:port (320 ms)` (Info).
5. 실패: `✗ TCP unreachable: hostname:port — <error short>` (Error sticky).

#### 3.6.2 의미 분리

- `v` = ssh -G dump (config resolve, 모든 옵션 보이기). v0.6 R6.
- `g` = TCP socket connect만. agent / key / banner 안 봄.
- `s` = 실제 ssh 연결 spawn. v0.4.

세 키 각각의 의미가 사용자에게 명확. README에 한 줄씩.

#### 3.6.3 anti-feature 1과의 정합

TCP connect는 *SSH protocol*과 무관 — 그냥 port reachability.
nc/telnet/Test-NetConnection이 하는 것과 동일 layer. sshc가 자체
SSH 구현으로 가는 게 아니므로 OK.

### 3.7 G7 — 사이즈 회수 평가

#### 3.7.1 baseline

- v0.8.4 macOS arm64 release: ~5.2MB (ureq + rustls).
- v0.7.3 baseline (pre-ureq): ~3.0MB.
- 차이: +2.1MB.

#### 3.7.2 후보 비교

세 가지 조합을 cargo bloat + 빌드 실측으로 비교:

| 후보 | TLS | 예상 사이즈 |
|---|---|---|
| `ureq` + `rustls` (현재) | rustls + webpki-roots | baseline |
| `ureq` + `rustls-tls-native-roots` | rustls + 시스템 trust | -300KB? |
| `attohttpc` + `tls-native` | system | -800KB? |
| `minreq` + `https-rustls` | rustls + webpki-roots | -1.2MB? |
| `ureq` + `native-tls` (재시도) | system | -1.5MB? (단 v0.8 R6 cycle에서 backend not configured 이슈 — 정확한 ureq agent config 필요) |

#### 3.7.3 결정 기준

- 가장 작은 후보를 선택하되 *cargo-dist 모든 타겟에서 빌드 성공*
  보장이 필요. ring 같은 native build를 끌어오면 macOS host
  cross-compile이 막힘 (v0.8 R6 학습).
- 회수 실패 (모든 후보가 비슷하거나 더 큼) 시 CHANGELOG에 명시하고
  현 ureq+rustls 유지.

### 3.8 G8 — Windows ARM64

#### 3.8.1 추가

`dist-workspace.toml`:
```toml
targets = [
    "aarch64-apple-darwin",
    "aarch64-pc-windows-msvc",   # 추가
    "aarch64-unknown-linux-gnu",
    "x86_64-apple-darwin",
    "x86_64-pc-windows-msvc",
    "x86_64-unknown-linux-gnu",
]
```

#### 3.8.2 검증

cargo-dist의 Windows ARM64 runner가 빌드 + zip + checksums까지
정상 produce. PowerShell installer가 ARM64 binary를 picked up
하는지 확인.

#### 3.8.3 risks

- ring (rustls 의존)의 ARM64 Windows MSVC 빌드 — windows-msvc x64
  는 cargo-dist에서 통과했으니 ARM64도 OK 가능성 큼. 첫 빌드에서
  실패하면 G7과 묶어 native-tls fallback 고려.

## 4. Module-boundary R-G gates

신규 R-G 게이트 없음. v0.6에서 굳어진 9개 게이트가 그대로 유효하다:

- R-G6 (storage/setup/probe/state가 crossterm/ratatui 만지지 말 것):
  G6의 TCP connect는 `src/exec/` 또는 `src/probe/` 신규 모듈에. UI
  layer에 끌어오지 말 것.
- R-G8 (ui/forms가 fs/process 만지지 말 것): G5 폼은 v0.8 R0의
  identity scan 패턴 그대로 — `app/forms.rs`에서 디스커버, `ui/forms`
  는 수신만.
- R-G9 (inline_app read-only): G4 `c` copy / G6 `g` reach는 manage
  모드 한정. inline_app 라우터에 추가하지 말 것.

## 5. New dependencies

| Crate | Purpose | Target | Version | Goal |
|---|---|---|---|---|
| `arboard` | 클립보드 (G4) | 무조건 | `3` | G4 |
| (G7 후보) | HTTP TLS 교체 | 무조건 | TBD | G7 |

`arboard`는 Wayland/X11/Cocoa/Win32 cfg 분기 자체에서 처리. sshc
코드에서 cfg 분기 없이 단일 호출.

## 6. Risks

| Risk | Mitigation |
|---|---|
| G3 sticky error가 *다른 정상 안내 메시지*까지 사용자가 보지 못하게 가림 | 다음 keystroke 단 한 번에 clear — 사용자가 키 누르면 자동 해소. 또는 status bar에 `(press any key to dismiss)` 짧은 hint. |
| G4 `arboard`가 Wayland 환경에서 깨짐 (X11/Wayland 동시 지원이 fragile) | 실패 시 status에 `clipboard unavailable — falling back to stdout` + stdout으로 echo. fail-safe. |
| G5 Forwarding 폼 추가로 폼 행수 7 → 10+. 좁은 터미널에서 modal 높이 초과 | v0.7.1에서 학습한 *one-row-per-field*는 유지. modal 높이가 부족하면 scrollable form (ratatui Paragraph scroll). 추가 작업 가능성 있음. |
| G6 TCP connect가 firewall/NAT 환경에서 false negative (실제 SSH는 가능한데 raw TCP는 막힘) | doctor / `v`와 분리 — `g`는 *TCP reach만 본다*고 README에 명시. 진짜 SSH는 `s`. |
| G7 사이즈 회수가 실측에서 미미 (-100KB 수준) | 그대로 진행하되 BRIEF/CHANGELOG에 측정값 정직히 기록. *사이즈 한도 자체를 v0.9에서 받아들임*. |
| G8 cargo-dist Windows ARM64 runner의 ring 빌드 실패 | G7과 묶어 native-tls 후보로 fallback. 또는 G8을 다음 라운드로 지연 (사용자 영향 적음). |
| G4/G5/G6가 동시에 들어가며 manage-mode key + form layout이 한꺼번에 크게 바뀜 | 라운드를 *기능별 commit 분리*하여 검증 가능. PLAN_V0.9 R4/R5/R6 명시. |

## 7. Definition of Done

- [ ] `cargo fmt --check` 클린.
- [ ] `cargo clippy --all-targets -- -D warnings` 클린 (host).
- [ ] `cargo test --release` 클린.
- [ ] R-G1..R-G9 클린.
- [ ] doctor 출력이 7→8~9 라인으로 늘어남. CRLF 케이스에서 WARN,
      nested-Include 케이스에서 WARN 매뉴얼 검증.
- [ ] status_message sticky on error: macOS에서 `apply_form` 강제
      실패 (e.g. sshc.conf 권한 변경) 후 메시지가 다음 키스트로크
      때까지 표시되는지 매뉴얼 검증.
- [ ] `c` 매뉴얼 검증: 호스트 선택 + `c` → 클립보드에 ssh 명령
      복사 + status 표시.
- [ ] `g` 매뉴얼 검증: 실제 호스트(reachable) + 가짜 호스트
      (unreachable) 각각.
- [ ] Forwarding 폼 매뉴얼: LocalForward 입력 → 저장 → sshc.conf에
      라인 등장 → `ssh -G alias` 확인.
- [ ] `cargo build --release` 사이즈 측정. v0.8.4 대비 delta 기록.
- [ ] cargo-dist Windows ARM64 빌드 통과 (or 명시적으로 deferred).
- [ ] README + README.ko의 키맵 표 갱신 (`c`, `g`, 새 폼 섹션).
- [ ] CHANGELOG `[0.9.0]` 엔트리.
- [ ] `main.rs`는 여전히 ≤ 10 LOC (R-G0).

## 8. Round breakdown (PLAN_V0.9 시드)

| R | 내용 | 게이트 |
|---|---|---|
| R0 | git pull --ff-only, v0.8.4 baseline gate 재실행, 사이즈 baseline 측정 | R-G1..R-G9 통과 |
| R1 | G1: doctor CRLF 체크 | 단위 + 매뉴얼 (Windows-origin config 시뮬레이션) |
| R2 | G2: doctor nested-Include 체크 | 단위 (Host stanza로 끝나는 config fixture) |
| R3 | G3: StatusKind enum + sticky-on-error 분기 + apply_form 호출자 갱신 | 단위 (Error는 expiry 무관, Info는 transient) |
| R4 | G4: arboard 의존 추가 + `c` 키 + status_message | 단위 (build_ssh_command 출력) + 매뉴얼 |
| R5 | G5: Forwarding 섹션 (forms.rs build_host 갱신 + ui/forms/host_form section header + sshc.conf serializer extra 라인) | 단위 (forwarding 필드 round-trip) + 매뉴얼 |
| R6 | G6: `g` TCP connect | 단위 (TcpStream::connect_timeout localhost OK / closed port FAIL) + 매뉴얼 |
| R7 | G7: HTTP/TLS 후보 평가 + 선택 후보로 교체 (실패 시 현 ureq+rustls 유지하고 BRIEF 갱신) | cargo bloat 측정 결과 docs/ 기록 |
| R8 | G8: dist-workspace.toml ARM64 추가 | 빌드 검증은 R9 태그 push 시 |
| R9 | 문서 (README, README.ko, CHANGELOG [0.9.0]), Cargo.toml 0.8.4 → 0.9.0, tag + push | cargo-dist 모든 타겟 success + 단일 dispatch (concurrency 가드) |

## 9. Out of scope

### 9.1 v1.0+로 미루는 항목 (lazyssh가 가지만 sshc는 거부)

- **자체 SCP / 파일 전송** (lazyssh upcoming) — anti-feature 1
  (Self-built SSH client) 위반. sshc는 ssh-agent / OpenSSH가
  제공하는 흐름만 spawn한다. 외부 SCP를 사용자가 자기 셸에서
  직접 부르는 게 정공법.
- **SSH 키 배포 / ssh-copy-id 통합** — anti-feature 2 (Secret /
  key management). 키 파일 내용을 sshc가 만지지 않는다. 사용자가
  ssh-copy-id를 직접 실행한다.
- **다중 호스트 동시 동작** (parallel ssh) — anti-feature 1 +
  운영 위험. doctor + `g`로 *상태만* 보여주고, 실제 multi-host는
  pdsh / ansible 같은 전용 도구가 함.
- **자체 SSH protocol 구현** — anti-feature 1. sshc는 항상
  `ssh.exe` / `ssh` binary를 spawn.

### 9.2 v0.10+로 미루는 항목 (사용자 수요 우선순위 낮음)

- **자동 update 다운로드** (v0.8 BRIEF carryover) — G3 doctor의
  업데이트 알림 + 사용자 manual upgrade 흐름 유지.
- **시작 시 자동 업데이트 체크** (v0.8 BRIEF carryover) —
  anti-feature 4 (always-on process) 정신과 어긋남. doctor에 가둠.
- **Identity 열거** (v0.8 BRIEF carryover) — agent의 식별자
  enumerate. anti-feature 1+2.
- **원본 ~/.ssh/config 자동 정리 after promote** (v0.8 BRIEF
  carryover) — anti-feature 1.

### 9.3 Project-wide anti-features (BRIEF_V8 §9.2 carry-over)

1. Self-built SSH client.
2. Secret / key management (passwords, keyfile content).
3. Team-shared catalogs, permissions, audit logs.
4. Web UI / daemon / always-on process.
5. Complete OpenSSH `config(5)` parser.

v0.9에서 어느 것도 완화하지 않는다. lazyssh가 SCP / 키 배포 / 다중
호스트로 가는 길은 sshc가 *명시적으로 가지 않는 길*이며, v0.9의
모든 신규 기능(G1~G8)은 이 다섯 안티피처와 정합이다.

## End of Brief.
