# sshc v0.12.0 — Architect Brief

> Companion docs:
> - `PLAN_V0.12.md` — round breakdown
> - `BRIEF_V11.md §9` — anti-features carry-over + v0.11 deferral list
> - 직전 BRIEF: `BRIEF_V11.md`
> - TODO seed: `BRIEF_V12_TODO.md` (gitignored)

## 1. Context

v0.11.0(2026-06-26)은 *measure-don't-predict* 원칙으로 -22~-30%의
크로스플랫폼 artifact 회수를 달성하며 v0.10 G2의 잘못된 사이즈
예측을 *훌쩍 넘어* 정정했다. 단일 Cargo.toml 라인 변경
(`env_logger = { default-features = false }`)이 regex chain
547 KiB을 통째로 dep 그래프에서 끊어낸 결과.

v0.12는 다시 **UX 작업으로 회귀**한다. v0.11이 닦은 작은 surface
위에서 v0.10 G1(`ForwardingListModal`)이 만든 *list editor* 패턴을
한 번 더 활용한다:

1. v0.10 G1의 ListModal은 OpenSSH의 `LocalForward` /
   `RemoteForward` / `DynamicForward` 다중 directive를 Vec<String>로
   잡았지만 — *같은 다중 directive 문제*가 `IdentityFile`에도 있다.
   OpenSSH는 호스트에 여러 키를 등록하고 *순서대로 시도*하는 흐름을
   허용한다. v0.7.1에서 `↑/↓`로 사이클하는 단일 picker는 그 흐름의
   한 갈래만 모델링했다.
2. v0.10 G1의 modal은 add/edit/delete만 — 다중 entry를 가진 사용자가
   *순서*를 조정할 길이 없다. OpenSSH는 entry 순서를 의미 있게
   취급한다.
3. v0.10 G5의 `S` sort axis는 session-only로 일부러 남겼다. v0.11
   세션에서 사용자가 dogfood한 결과 *fresh sshc는 늘 AliasAlpha로
   돌아가는* 행동이 surprise였다. v0.12에서 닫는다.

설계 결정: **`ListEditModal` 일반화**(사용자 확정). 현재
`ForwardingListModal`을 `ListEditModal`로 rename하고 kind를
`ListKind` enum으로 추출한다 — `ListKind::Forwarding(ForwardingKind)`
+ `ListKind::IdentityFile`. 행동 자체는 R1에서 변경 0; R2 이후가
새 surface를 추가하는 단계.

Anti-features (`BRIEF_V10 §9.3 + V11 carry-over`)는 그대로
유지. v0.12에서 완화되는 안티피처는 없다.

## 2. v0.12 Goals

| # | Goal | Definition |
|---|---|---|
| G1 | IdentityFile multi-value | `Host::identity_file: Option<PathBuf>` → `Vec<PathBuf>`. 파서가 occurrence별 push, serializer는 entry별 `IdentityFile` 라인 발행. 폼의 IdentityFile 행은 *summary cell*로 전환; Enter → `ListEditModal` (kind = `IdentityFile`). v0.7.1의 `↑/↓` candidate picker는 modal 안의 edit mode에서 재현된다(modal이 candidate Vec<PathBuf>를 받음). |
| G2 | ListEditModal: reorder via Shift+↑/↓ | browse mode에서 entry 순서를 위/아래로 한 칸씩 이동. 마지막 entry에서 Shift+↓ no-op, 첫 entry에서 Shift+↑ no-op. add-row(`+ add`)에서는 Shift 키 무시. OpenSSH의 declaration 순서가 의미 있는 directive(IdentityFile, ProxyJump-like)에 필요. |
| G3 | Sort axis state persistence | `SortAxis`를 `state.toml::MemorySection::sort_axis`로 직렬화. App::new에서 load, App::cycle_sort_axis에서 save. v0.10 G5의 session-only를 닫는다 — 사용자가 v0.11 dogfood로 명시적으로 회귀로 보고한 동작. |

## 3. Goal-별 설계

### 3.1 G1 — IdentityFile multi-value

#### 3.1.1 Storage 변경

```rust
// src/config/model.rs
pub struct Host {
    ...
-    pub identity_file: Option<PathBuf>,
+    pub identity_file: Vec<PathBuf>,
    ...
}
```

`BlockState::identity_file`도 `Vec<PathBuf>`. parser 마치ALL 분기 push.

#### 3.1.2 Serializer

```rust
for path in &host.identity_file {
    out.push_str(&format!("    IdentityFile {}\n", path.display()));
}
```

빈 Vec → 라인 발행 안 함 (현재 `Some/None` 분기와 동일 의미).

#### 3.1.3 폼 통합

기존 v0.7.1 picker:
- `fields[IDENTITY_INDEX]`가 path string 한 개를 보유
- `↑/↓`로 `identity_candidates: Vec<PathBuf>` 순회

v0.12 변경:
- `fields[IDENTITY_INDEX]`는 *summary cell* (Vec 첫 entry + `+N more`)
- `↑/↓`는 active_index가 IDENTITY_INDEX일 때 *어떤 동작도 안 함*
  (modal 안으로 이동)
- Enter on IDENTITY_INDEX → `ListEditModal::new(ListKind::IdentityFile, vec, candidates)`
- modal의 edit mode에서 `↑/↓`가 v0.7.1 picker 동작 재현
  (candidate 사이클)

`HostForm` struct에 `identity_file_entries: Vec<String>` 추가
(forwarding 패턴과 동일). `build_host`에서 String → PathBuf로 변환,
`from_host`에서 PathBuf → String. v0.7.0~v0.7.2의 `\` 백슬래시
허용 경로 검증은 modal validate로 이동.

#### 3.1.4 Fixture sweep

모든 Host fixture에서 `identity_file: Some(PathBuf::from(...))` →
`identity_file: vec![PathBuf::from(...)]`, `None` → `Vec::new()`.
v0.10 R1과 동일 패턴 (8 fixture).

### 3.2 G2 — Reorder via Shift+↑/↓

`ListEditModal::handle_key`의 browse 모드 분기에 추가:

```rust
KeyCode::Up if key.modifiers.contains(KeyModifiers::SHIFT)
    && self.selected > 0
    && self.selected < self.entries.len() =>
{
    self.entries.swap(self.selected, self.selected - 1);
    self.selected -= 1;
    ListOutcome::Stay
}
KeyCode::Down if key.modifiers.contains(KeyModifiers::SHIFT)
    && self.selected + 1 < self.entries.len() =>
{
    self.entries.swap(self.selected, self.selected + 1);
    self.selected += 1;
    ListOutcome::Stay
}
```

`+ add` 행에서는 Shift+↑/↓를 무시 (selected == entries.len() 조건).

단위 테스트 2개: top entry shift-up no-op, 중간 entry shift-down 후
순서 검증.

### 3.3 G3 — Sort axis state persistence

#### 3.3.1 schema

```rust
// src/state/schema.rs
pub struct MemorySection {
    ...
+    #[serde(default)]
+    pub sort_axis: SortAxisPersisted,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortAxisPersisted {
    #[default]
    Alias,
    Recent,
    Reachability,
}
```

`SortAxis` enum이 `app::mod`에 있고 `SortAxisPersisted`가
`state::schema`에 있는 게 R-G6(state 모듈은 ratatui/crossterm 무관).
변환 함수 두 개.

#### 3.3.2 load/save

`App::new`:
```rust
sort_axis: SortAxis::from_persisted(state.memory.sort_axis),
```

`App::cycle_sort_axis`:
```rust
self.sort_axis = self.sort_axis.next();
self.state.memory.sort_axis = self.sort_axis.to_persisted();
let _ = self.state.save();  // best-effort, status도 그대로 emit
self.apply_filter();
...
```

state save 실패는 status에 *별도 에러로 surface하지 않는다* — 이미
`sorted by ...` info를 보여줬는데 옆에 에러를 띄우면 사용자에게
noise. 다음 정상 save에서 자연 복구.

#### 3.3.3 migration

`#[serde(default)]` 덕에 기존 state.toml에 `sort_axis`가 없어도
`Alias`로 디폴트. 별도 migration 작업 없음.

## 4. Module-boundary R-G gates

신규 R-G 게이트 없음. v0.6에서 굳어진 9개 게이트 그대로.

특히 주의:
- R-G6: `state/*.rs`에는 ratatui/crossterm import 금지.
  `SortAxisPersisted`는 state에 두고 `SortAxis`는 app에 두는 분리가
  R-G6을 유지하기 위한 것.
- R-G8: `ui/forms/*` + `ui/modal.rs`에 fs / Command 금지.
  IdentityFile candidate 디스커버리는 `app::forms::discover_identity_files`
  (v0.8 R0 hoist)가 책임 — v0.12 변경 없음.

## 5. New dependencies

없음. v0.12는 v0.10 G1 surface 확장 + state.toml schema 한 필드
추가만.

## 6. Risks

| Risk | Mitigation |
|---|---|
| `IdentityFile` Vec 도입으로 v0.11 binary가 v0.12 sshc.conf를 읽을 때 두 번째 이후 entry가 cascade되어 `extra`에 들어가는 비호환. (v0.10 G1과 동일 trade-off.) | CHANGELOG에 명시. v0.11→v0.12 forward 호환은 보장, v0.12→v0.11 backward는 single entry만 보존. |
| 사용자가 v0.7.1 picker의 `↑/↓` 사이클을 form *최상위*에서 기대 | Enter on IdentityFile 행 → modal 안에서 동일 사이클. README/CHANGELOG에 명시. |
| Shift+↑/↓ 충돌 — 일부 터미널이 키 codes를 다르게 보냄 | crossterm의 KeyModifiers::SHIFT는 신뢰 가능. 의심되면 K/J도 reorder alias로 추가 (v0.12 안 함). |
| state.toml save 실패 | best-effort. UI에 별도 에러 안 띄움 — sort axis는 cosmetic이라 사용자 흐름 끊을 가치 없음. |
| ListEditModal rename이 v0.10 호환성 깨뜨림 (외부 사용자 — 없음) | sshc는 lib 형태로 외부 호출되지 않음. R1에서 안전. |

## 7. Definition of Done

- [ ] `cargo fmt --check` 클린.
- [ ] `cargo clippy --all-targets -- -D warnings` 클린 (host).
- [ ] `cargo clippy --all-targets --target x86_64-pc-windows-msvc -- -D warnings`
      클린.
- [ ] `cargo test --release` 클린 — lib + integration.
- [ ] R-G1..R-G9 클린.
- [ ] IdentityFile multi round-trip 테스트 (parser → serializer → parser).
- [ ] Forwarding reorder 단위 테스트 2개.
- [ ] Sort axis persistence 단위 테스트 (state load + cycle save + reload).
- [ ] README + README.ko: IdentityFile multi 모달 한 문단, S sort 영구 보존 한 줄.
- [ ] CHANGELOG `[0.12.0]` 엔트리.
- [ ] cargo-dist 단일 dispatch + 6 플랫폼 사이즈 v0.11.0 대비 측정 (예측 X).
- [ ] `main.rs`는 여전히 ≤ 10 LOC (R-G0).

## 8. Round breakdown (PLAN_V0.12 시드)

| R | 내용 | 게이트 |
|---|---|---|
| R0 | git pull + R-G + size baseline (DONE this session) | 통과 |
| R1 | ListEditModal rename + ListKind enum 추출. ForwardingKind는 ListKind::Forwarding(_)로 wrap. 행동 변경 0. | 모든 기존 forwarding 테스트 그대로 통과 |
| R2 | G1 R2-A: Host::identity_file Option → Vec. 파서/serializer 갱신, fixture sweep. 폼은 단일 path 보존 (다음 라운드에서 modal로). | round-trip 테스트 + R-G |
| R3 | G1 R2-B: 폼 통합. IDENTITY_INDEX 행은 summary cell, Enter → ListEditModal(IdentityFile). 기존 v0.7.1 picker는 modal 내부로 이동. | 모달 통합 단위 테스트 |
| R4 | G2: ListEditModal에 Shift+↑/↓ reorder + 2 단위 테스트 | reorder 테스트 PASS |
| R5 | G3: state.toml schema에 sort_axis 추가 + load/save 통합 + 단위 테스트 | persistence 테스트 PASS |
| R6 | docs (README + README.ko + CHANGELOG), Cargo.toml 0.11.0→0.12.0, tag + push | cargo-dist 단일 dispatch + 6 플랫폼 사이즈 측정 후 보고 |

## 9. Out of scope

### 9.1 v0.13+로 미루는 항목

- **추가 사이즈 회수**: `toml_edit` (165 KiB), `ureq` (118 KiB)
  swap — 사용자 피드백 우선순위 변하지 않으면 v0.13 후보.
- **Forwarding entries on preview panel** — 작은 cosmetic.
- **doctor: variable-substituted ProxyCommand sanity** — `%h`/`%p`
  expansion 후 path 재검사.
- **doctor: OSC 52 + tmux note** — `TMUX` env 감지해 hint.
- **K/J keys as reorder alias** — Shift+↑/↓로 R4에서 충분.

### 9.2 Project-wide anti-features (BRIEF_V10 §9.3 carry-over)

1. Self-built SSH client.
2. Secret / key management.
3. Team-shared catalogs.
4. Web UI / daemon / always-on process.
5. Complete OpenSSH `config(5)` parser.

Plus v0.9 / v0.10 / v0.11 reaffirmations (모두 유지):
- No SCP / file transfer.
- No `ssh-copy-id` integration.
- No parallel ssh / multi-host dispatch.
- No automatic update *download*.
- No identity enumeration.
- No always-on update check.

v0.12에서 어느 것도 완화하지 않는다.

## End of Brief.
