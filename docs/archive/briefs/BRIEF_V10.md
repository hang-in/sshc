# sshc v0.10.0 — Architect Brief

> Companion docs:
> - `PLAN_V0.10.md` — round breakdown
> - `BRIEF_V9.md` §9 — anti-features carry-over + v0.9 deferral list
> - 직전 BRIEF: `BRIEF_V9.md`
> - TODO seed: `BRIEF_V10_TODO.md` (작성 시점 메모, gitignored)

## 1. Context

v0.9.0(2026-06-26)은 8개 Goals(doctor CRLF/nested-Include, sticky
status, `c` copy ssh, Forwarding 폼 섹션, `g` TCP reach, native-tls
사이즈 회수, Windows ARM64)를 한 사이클에 닫았고, 그 중 R7
ureq+native-tls explicit wire가 *PLAN 목표 -400KB의 5배인 -2.18MB*
를 회수하면서 macOS arm64 release를 3.76MB까지 줄였다. ring 의존성
이 빠지면서 v0.8 R6부터 막혀 있던 `cargo check --target
x86_64-pc-windows-msvc` macOS host 크로스컴파일도 다시 살아났다.

v0.10은 **표면 다듬기 + 사이즈 2차 회수**의 라운드다. 큰 새 기능
은 한 개(G1 multi-forwarding 흐름)이고 나머지는 v0.9 surface가
*살짝 부족했던 부분*을 채운다:

1. v0.9 G5의 Forwarding 폼은 typed `Option<String>` 한 자리 — OpenSSH
   는 같은 directive 여러 번 허용한다. 멀티 forward 사용자는 첫
   번째만 폼에서 보고 나머지는 `extra`(free-form Options)로 떨어진다.
   round-trip 보존은 되지만 *UI에서 편집*은 불가능. G1이 닫는다.
2. v0.9 R4의 arboard는 image/tiff transitive를 끌어와 ~+600KB. PNG
   디코더가 sshc에 필요 없다 — 더 작은 클립보드 라이브러리로 교체.
3. v0.9 R4의 `c` copy는 Wayland-without-display 환경에서 fail하면
   Error 사티키만 띄우고 끝. OSC 52 fallback이면 그런 환경에서도
   동작.
4. `ProxyCommand`를 쓰는 호스트가 흔한데 그 명령이 PATH에서 안
   찾아지면 `ssh`는 묵묵히 fail. doctor가 한 줄로 surface.
5. lazyssh의 sort 기능 — 100+ 호스트 사용자에게 가치. `s`는 ssh로
   점유했으니 `S`(Shift+s)로.

anti-features (`BRIEF_V9 §9.3`)는 그대로. v0.10에서 완화되는
안티피처 없다.

## 2. v0.10 Goals

| # | Goal | Definition |
|---|---|---|
| G1 | Multi-forwarding entries with a dedicated list modal | `Host`의 `local_forward / remote_forward / dynamic_forward`를 `Option<String>` → `Vec<String>`으로 변환. 호스트 폼의 각 forwarding 필드 안에서 Enter → 별도 `ForwardingListModal`이 열려 각 entry를 한 줄씩 추가/편집/삭제. 폼은 첫 entry + 개수 요약("(3)")만 표시. 저장 시 모든 entry를 sshc.conf에 별도 라인으로 emit. 이미 v0.9 G5가 last-wins + extra cascade로 round-trip을 보존했으니 데이터 모델 확장만으로 backward-compat. |
| G2 | Clipboard backend swap (arboard → smaller) | arboard 3.x는 `image` + `tiff` decoder를 transitive로 끌어와 sshc 사이즈에 +600KB. 우리는 `set_text` 한 번만 쓴다. 후보: `clipboard-anywhere`, `copypasta`, 또는 platform-cfg 분기 직접 구현(macOS NSPasteboard via objc2, Windows OpenClipboard via windows-sys, Linux X11 + Wayland minimal). 목표: v0.9.0 대비 -300KB 이상. |
| G3 | OSC 52 clipboard fallback | G2가 어떤 후보로 가든 *최후 fallback*으로 OSC 52 escape sequence를 stdout에 emit. kitty / iTerm2 / foot / alacritty / wezterm 모두 honor. tmux 안에서는 `set -g set-clipboard on` 필요(README에 명시). G2 backend가 ErrorContentNotAvailable / Unsupported / 실패 시 OSC 52로 fall through. 사용자에게는 *어떤 path가 성공했는지* status에 짧게 표기(`copied (osc52)`). |
| G4 | doctor: ProxyCommand sanity | sshc.conf + Include 체인의 모든 호스트를 훑어 `ProxyCommand` directive에서 첫 token(실행 파일)을 추출한다. 시스템 PATH에서 찾을 수 없으면 `[WARN] proxy commands  '<token>' not on PATH (used by N host(s))`. 여러 호스트가 같은 proxy를 쓰면 count로 묶어 한 줄. read-only — anti-feature 1 무관. |
| G5 | `S` sort key (manage mode only) | `s`는 ssh, `S`(Shift+s)는 sort. 정렬 축 cycle: alphabetical alias → recent-use desc → ProbeState (Open first, Failed last) → (다시 alphabetical). status에 현재 정렬 표시("sorted by recent"). State persistence 없음 — 세션마다 reset(첫 라운드는 단순화, 사용자 피드백 보고 v0.11에서 persist 결정). |

Anti-features (`BRIEF_V9.md §9.3`)는 그대로 carry-over. v0.10에서
완화되는 안티피처는 없다.

## 3. Goal-별 설계

### 3.1 G1 — Multi-forwarding entries with list modal

#### 3.1.1 모델 변경

`src/config/model.rs`:
```rust
pub struct Host {
    // ...
    pub local_forward: Vec<String>,      // was Option<String>
    pub remote_forward: Vec<String>,     // was Option<String>
    pub dynamic_forward: Vec<String>,    // was Option<String>
}
```

`src/config/parser.rs`의 last-wins 분기(`block.local_forward.replace(...)`)
가 `push()`로 바뀐다. 이전 값이 `extra`로 떨어지는 cascade 코드는
제거 — 진짜 multi 지원이 들어오니까.

`src/storage/serializer.rs`의 emit은 Vec를 순회해 *각 entry를
별도 라인*으로 출력. 빈 Vec은 emit 안 함.

#### 3.1.2 Form 변경

`HostForm` 자체는 *기존 String 한 필드*를 *display summary*로 유지:
- 비어있으면 `[]`
- 1개면 그 entry 그대로 `[8080 localhost:80]`
- 2개 이상이면 `[8080 ... +2 more]`

해당 필드가 active일 때 Enter → `ForwardingListModal` 띄움.
`FormState::handle_key`의 Enter 분기에 추가.

#### 3.1.3 ForwardingListModal

새 모듈 `src/ui/forms/forwarding_list.rs`:
```rust
pub struct ForwardingListModal {
    title: &'static str,       // "LocalForward" / "RemoteForward" / "DynamicForward"
    entries: Vec<String>,
    selected: usize,
    editing: Option<String>,   // Some when typing into a row
    validator: fn(&str) -> bool,  // looks_like_local_remote_forward etc.
}
```

키맵:
- ↑/↓: 선택 이동
- Enter: 편집 모드 진입 (또는 `+` 자리에서 새 entry 추가)
- d: 선택된 entry 삭제 (확인 없음 — Esc로 취소 가능)
- Esc: editing이면 편집 취소, 아니면 modal 닫고 부모 폼으로 복귀

부모 폼은 modal 결과를 받아 fields[i]의 display summary를 갱신.

#### 3.1.4 통합

`FormPayload::Host`의 3개 String 필드 → `Vec<String>`. apply_form
match arm + build_host 시그니처도 따라 바뀐다.

#### 3.1.5 Backward-compat

기존 sshc.conf에 single forwarding이 있던 경우, parser가 Vec[0]에
push. v0.9 → v0.10 read에서 깨질 일 없음. v0.10이 *여러 라인*을
emit해도 v0.9 parser는 last-wins로 마지막 것만 가져가고 나머지는
extra로 떨어지므로 forward-compat도 깨끗하지는 않지만 *깨지지는
않는다* (필드 일부 손실).

### 3.2 G2 — Clipboard backend swap

#### 3.2.1 후보 평가 (R6에서 cargo bloat 측정 후 결정)

| Candidate | Approach | Expected delta |
|---|---|---|
| `copypasta` | wraps arboard + clipboard | similar to current |
| `clipboard-anywhere` | smaller wrapper | ~-300KB |
| 직접 platform-cfg 구현 | macOS objc2, Win32 OpenClipboard, Linux X11/Wayland minimal | ~-500KB but more code (~150 LOC) |
| `arboard` with feature gates off | `arboard = { default-features = false, features = ["wayland-data-control"] }` 같은 옵션이 있다면 | -200KB? unknown |

R6에서 *cargo bloat + 빌드 사이즈* 순서로 측정해 최저값 후보 채택.
실패 시 arboard 유지하되 BRIEF/CHANGELOG에 측정값 기록.

#### 3.2.2 호환 layer

`src/exec/clipboard.rs` (new): 단일 함수 `pub fn copy_to_clipboard(text: &str) -> Result<ClipboardBackend, ClipboardError>`. `ClipboardBackend`는
`System / Osc52` 같은 enum — G3가 이걸로 fall-through 표시.

### 3.3 G3 — OSC 52 fallback

#### 3.3.1 동작

`copy_to_clipboard`가 system clipboard에 실패하면 OSC 52 emit:
```
ESC ] 52 ; c ; <base64 of text> ESC \
```

stdout으로 출력하면 terminal emulator가 가로채 클립보드에 설정.
TTY 모드라 raw output이 user에게 보이지 않는다.

#### 3.3.2 제약 명시

README에 추가:
- 작동 환경: kitty / iTerm2 / foot / alacritty / wezterm (대부분 기본 on).
- tmux 안에서는 `set -g set-clipboard on` 필요.
- SSH 세션 안에서 sshc를 돌릴 때 server-side에서 클라이언트
  클립보드로 setting이 통하는지는 *클라이언트 emulator에 의존*.

#### 3.3.3 status 메시지

```
copied: ssh d9ng@host -p 2232 -i …
copied: ssh d9ng@host -p 2232 -i … (osc52)
```

backend 표시는 *system이 성공이면 생략*, *fallback일 때만* 표시.

### 3.4 G4 — doctor ProxyCommand sanity

#### 3.4.1 흐름

`src/doctor.rs`에 신규 체크 `check_proxy_commands`:

1. `crate::config::parser::parse_config` 결과를 받아 모든 host의
   `extra` 라인에서 `ProxyCommand <token...>` 찾기.
2. 첫 token 추출 (실행 파일 이름). 변수 치환(`%h`, `%p`, `%r`)이
   *실행 파일 자체*에 있으면 검증 불가 → skip.
3. PATH에서 찾기: `std::env::split_paths(&std::env::var_os("PATH")?)` +
   각 디렉토리 join + `is_file()` + (Unix) executable bit. Windows는
   `.exe` / `.cmd` / `.bat` suffix까지.
4. 못 찾으면 `(token, count)`에 누적.
5. 출력: `[WARN] proxy commands  '<token>' not on PATH (used by N hosts)`.

#### 3.4.2 모든 PASS일 때

체크 자체 생략 (CRLF 체크와 동일한 패턴). 깨끗하면 doctor 출력에
*추가 줄 없음*.

### 3.5 G5 — `S` sort key

#### 3.5.1 정렬 축

```rust
enum SortAxis {
    AliasAlpha,
    RecentDesc,
    ProbeStateOpenFirst,
}
```

`App`에 `sort_axis: SortAxis` 필드 추가 (default `AliasAlpha`).

`apply_filter` 끝에 *현재 axis에 따라 filtered Vec를 sort*. 첫
줄에 안정 정렬(`stable_sort_by`)로 axis 외 ordering(예: pin)은 깨지지
않게.

#### 3.5.2 키맵

`S` → axis cycle (`AliasAlpha → RecentDesc → ProbeStateOpenFirst → ...`).
상태 메시지에 "sorted by recent" 등 짧게.

#### 3.5.3 state 영속성

이번 라운드는 **세션 한정** — `app.state.memory`에 저장 안 함.
v0.11에서 사용자 피드백 보고 결정.

## 4. Module-boundary R-G gates

신규 R-G 게이트 없음. v0.6에서 굳어진 9개 게이트 그대로 유효:

- R-G6: clipboard 직접 구현 시 `std::process` spawn은 storage가 아닌
  `exec/clipboard.rs`에 한정.
- R-G8: `ForwardingListModal`은 `ui/forms/forwarding_list.rs` —
  `std::fs` / `std::process` 안 만진다. parent form이 결과 받음.
- R-G9: G5 sort는 manage-mode App만 — `inline_app`는 read-only
  browser 그대로.

## 5. New dependencies

| Crate | Purpose | Version | Goal |
|---|---|---|---|
| (TBD R6) | clipboard backend 후보 | 측정 후 결정 | G2 |
| (none) | OSC 52은 std-only로 가능 (`base64`만 필요) | — | G3 |
| `base64` | OSC 52 payload encoding | `0.22` | G3 |

`base64`는 ~+30KB. G2 후보가 무엇이든 G2의 회수보다 작다.

## 6. Risks

| Risk | Mitigation |
|---|---|
| G1 ForwardingListModal이 좁은 터미널에서 modal-in-modal로 보임 (모달 위에 모달) | ratatui 자체는 모달 stack을 지원 — `App::mode`를 stack으로 확장하거나, list modal은 parent form을 *완전히 가린다*. 후자가 단순. v0.7.1의 form layout 학습으로 좁은 폭 처리는 가능. |
| G1 backward-compat: v0.10이 multi-line emit한 sshc.conf를 v0.9 사용자가 읽으면 *마지막 라인만* 잡힘 (조용한 데이터 손실) | v0.10이 *기존 single-line 호스트는 그대로 single-line emit*하고, multi-forwarding이 추가된 호스트만 multi-line. 사용자가 multi를 적극적으로 추가하지 않는 한 영향 없음. CHANGELOG에 명시. |
| G2가 platform-cfg 직접 구현 길로 가면 라운드 작업량 폭증 | R6 시작에서 cargo bloat로 *가장 작은 wrapper crate*가 충분히 작으면 그걸 쓰고 직접 구현은 v0.11로 미룬다. 직접 구현 line이 200 LOC 초과하면 라운드를 v0.10에서 빼낸다. |
| G3 OSC 52이 tmux에서 막혀 사용자에게 "왜 안 됨" 발생 | README + status 메시지에 `(osc52)` 표시 + tmux `set -g set-clipboard on` 안내. 또는 SSHC_NO_OSC52 escape hatch. |
| G4 ProxyCommand 토큰 추출이 quoted/escaped 값을 잘못 split | `shlex` 같은 dep 도입은 과함 — 단순 whitespace split + 첫 token. quoted 케이스는 skip(검증 불가 처리). |
| G5 sort가 favorites/pin과 충돌 (favorites는 항상 위) | favorite 정렬은 v0.6 R2부터 첫 정렬 키 — sort axis는 *favorites 아래* 그룹 안에서만 적용. 안정 정렬로 보장. |

## 7. Definition of Done

- [ ] `cargo fmt --check` 클린.
- [ ] `cargo clippy --all-targets -- -D warnings` 클린 (host).
- [ ] `cargo clippy --all-targets --target x86_64-pc-windows-msvc -- -D warnings`
      클린 (v0.9 R7부터 살아있는 cross check 유지).
- [ ] `cargo test --release` 클린.
- [ ] R-G1..R-G9 클린.
- [ ] G1: forwarding list modal 매뉴얼 검증 — 3개 entry 추가 → 저장
      → sshc.conf에 3 라인 등장 → 다시 폼 열어 3개 표시.
- [ ] G2: release 사이즈가 v0.9.0의 3.76MB 대비 -300KB 이내. 회수
      실패 시 측정값 CHANGELOG에 정직히 기록.
- [ ] G3: OSC 52 매뉴얼 검증 — 외부에 보이는 클립보드 backend
      disable 후 `c` → status에 `(osc52)` 표시 + 실제 클립보드 갱신
      확인 (iTerm2/kitty에서).
- [ ] G4: 매뉴얼 검증 — 가짜 ProxyCommand가 있는 호스트 1개 → doctor
      → WARN 표시. 정상이면 줄 자체 안 나옴.
- [ ] G5: 매뉴얼 검증 — 3개 호스트로 `S` 4번 누르면 sort axis 한
      바퀴 돌아 원위치.
- [ ] README + README.ko의 키맵 표 갱신 (`S` sort), 폼 섹션 갱신
      (forwarding 리스트 모달), doctor 섹션 갱신 (ProxyCommand
      체크 + OSC 52 안내).
- [ ] CHANGELOG `[0.10.0]` 엔트리.
- [ ] `main.rs`는 여전히 ≤ 10 LOC (R-G0).

## 8. Round breakdown (PLAN_V0.10 시드)

| R | 내용 | 게이트 |
|---|---|---|
| R0 | git pull + R-G + 사이즈 baseline 측정 (3.76MB) | 통과 — v0.10 시작 직전 완료 |
| R1 | G1 storage 모델 — Host 3 필드 Option→Vec + parser push + serializer 멀티-라인 emit + fixture 일괄 갱신 | parser round-trip 테스트 |
| R2 | G1 ForwardingListModal — `src/ui/forms/forwarding_list.rs` + 폼에서 Enter dispatch + FormPayload Vec 전환 | modal 단위 테스트 + 폼 통합 테스트 |
| R3 | G4 doctor ProxyCommand | parser fixture 단위 테스트 |
| R4 | G5 `S` sort key | sort axis cycle 단위 테스트 |
| R5 | G3 OSC 52 fallback — `src/exec/clipboard.rs` + base64 dep | encoded payload 단위 테스트 |
| R6 | G2 clipboard backend 평가 + 교체 | 사이즈 측정 결과 docs/ 기록 |
| R7 | 문서 + Cargo.toml 0.9.0 → 0.10.0 + tag + push | Definition of Done 전 항목 + cargo-dist 단일 dispatch + ARM64 artifact |

## 9. Out of scope

### 9.1 v0.11+로 미루는 항목

- **sort axis state 영속성** (G5 carryover) — 세션 한정이 충분한지
  v0.10 사용 피드백 후 결정.
- **forwarding list modal에서 ↑/↓ reorder** — 단순 add/edit/del만
  v0.10. 순서 바꾸기는 v0.11.
- **multi `IdentityFile`** — 같은 패턴이 IdentityFile에도 있지만
  scope 보호 차원에서 forwarding만 v0.10. v0.11 후보.
- **clipboard backend 직접 구현** (G2 carryover) — wrapper crate가
  충분히 작으면 v0.10에서 채택. 직접 구현은 v0.11.

### 9.2 Project-wide anti-features (BRIEF_V9 §9.3 carry-over)

1. Self-built SSH client.
2. Secret / key management (passwords, keyfile content).
3. Team-shared catalogs, permissions, audit logs.
4. Web UI / daemon / always-on process.
5. Complete OpenSSH `config(5)` parser.

플러스 v0.9-specific reaffirmations (모두 유지):
- No SCP / file transfer.
- No `ssh-copy-id` integration.
- No parallel ssh / multi-host dispatch.
- No automatic update *download* (doctor surfaces availability;
  user runs their own installer).
- No identity enumeration on a discovered agent.

v0.10에서 어느 것도 완화하지 않는다.

## End of Brief.
