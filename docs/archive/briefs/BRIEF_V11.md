# sshc v0.11.0 — Architect Brief

> Companion docs:
> - `PLAN_V0.11.md` — round breakdown
> - `BRIEF_V10.md` §9 — anti-features carry-over + v0.10 deferral list
> - 직전 BRIEF: `BRIEF_V10.md`
> - TODO seed: `BRIEF_V11_TODO.md` (gitignored)

## 1. Context

v0.10.0(2026-06-26)은 5개 Goals를 한 사이클에 닫았지만 G2(arboard
image-data 비활성화)의 사이즈 회수는 *예측이 빗나갔다*. PLAN/commit
메시지가 "-600~-800KB"를 약속했는데 cargo-dist 실측에서는 *모든
플랫폼 사이즈가 증가* — Linux x64는 +164KB. R6 commit이 측정 없이
숫자를 쓴 게 원인. GitHub Release notes에 erratum을 prepend하고
master history는 비파괴 보존한 상태로 사이클을 닫았다.

v0.11은 **그 빚을 갚는 라운드**다. 큰 surface 변경 없이 v0.10에서
실패한 사이즈 회수를 *측정 기반*으로 다시 시도한다. v0.11 시작
시점에 `cargo bloat`로 실제 top consumer를 식별했다:

| Rank | Crate | .text |
|---:|---|---:|
| 1 | std | 456.6 KiB |
| 2 | sshc 본체 | 355.4 KiB |
| 3 | **regex_automata** | **264.1 KiB** |
| 4 | **regex_syntax** | **169.2 KiB** |
| 5 | toml_edit | 157.8 KiB |
| 6 | ureq | 117.7 KiB |
| 7 | **aho_corasick** | **114.0 KiB** |
| 8 | ratatui | 71.8 KiB |
| 9 | nucleo_matcher | 69.2 KiB |
| 10 | url | 43.8 KiB |

`regex_automata + regex_syntax + aho_corasick = 547 KiB`(.text의
26.7%). sshc는 regex를 직접 import하지 않는다. `cargo tree
--invert regex-automata`로 출처를 추적하면:

```
regex-automata 0.4.14
└── regex 1.12.3
    └── env_filter 1.0.1
        └── env_logger 0.11.10
```

sshc의 `env_logger` 사용처는 `main.rs:7`의 `env_logger::init()` 한 줄.
`log::warn!`/`error!` 호출은 7곳 — 모두 TUI raw mode 환경이라 stdout이
표시되지 않고 *사실상 silent*. RUST_LOG로 디버깅한 적은 거의 없다.

v0.11 G1은 **`env_logger`의 regex 의존을 끊는다**. default features
중 `regex`를 빼면 RUST_LOG가 *prefix 매칭만* 지원하게 된다 — sshc의
모든 log 호출은 `sshc::` prefix 안에 있으므로 사용성 영향 0.

Anti-features (`BRIEF_V10 §9.3`)는 그대로 carry-over. v0.11에서
완화되는 안티피처는 없다.

## 2. v0.11 Goal

| # | Goal | Definition |
|---|---|---|
| G1 | env_logger default-off + measured size recovery | `Cargo.toml`에서 `env_logger = "0.11"` → `env_logger = { version = "0.11", default-features = false }`. `auto-color` / `humantime` / `regex` 세 features 모두 off. 이로 인해 `regex` / `regex_automata` / `regex_syntax` / `aho_corasick`이 transitive에서 제거되어야 한다. *예측 숫자는 commit 메시지에 쓰지 않는다* — 측정값만. R2에서 cargo bloat 재실행으로 실제 결과 확인 후 그 숫자만 CHANGELOG에 기록. 추가 회수 후보(toml_edit, nucleo 등)는 R1 결과를 보고 평가. |

v0.10 R6의 교훈이 G1 정의 자체에 들어간다 — *측정 후 보고*가
정의의 일부.

## 3. Goal-별 설계

### 3.1 G1 — env_logger default-off

#### 3.1.1 변경

`Cargo.toml`:
```toml
# Before
env_logger = "0.11"

# After
env_logger = { version = "0.11", default-features = false }
```

#### 3.1.2 영향

- `RUST_LOG=sshc=debug sshc -m` 같은 *prefix 매칭*은 그대로 동작.
- `RUST_LOG="sshc::config::.*"` 같은 *wildcard / regex 매칭*은 동작
  안 함. sshc 코드는 이 형식을 가정한 적 없고 README에도 명시한
  적 없다.
- `auto-color` 비활성화: env_logger의 컬러 출력 비활성화. TUI 모드
  에서 어차피 stderr / stdout 출력 안 보임. 사용성 영향 0.
- `humantime` 비활성화: timestamp format이 RFC 3339에서 단순
  format으로 변경. RUST_LOG 사용자 본 적 없으므로 영향 0.

#### 3.1.3 측정 흐름 (R1 + R2)

1. R0 baseline: `target/release/sshc` 사이즈 + `cargo bloat --crates -n 15`
   결과 기록. (작성 시점 macOS arm64: 3,982,280 bytes.)
2. R1: Cargo.toml 변경 + 빌드 + 새 사이즈 + 새 bloat 결과 기록.
3. R2 (조건부): R1 결과가 -200KB 미만이면 *추가 후보 평가* — 어떤
   crate가 남아 있는지 보고 다음 단계 결정. 예: `toml_edit`을 더
   가벼운 toml parser로 교체할 수 있는지, `nucleo`를 `sublime_fuzzy`로
   교체할 수 있는지 등.
4. R3: 모든 측정값을 CHANGELOG에 *그대로* 기록. 예측 숫자 없음.

#### 3.1.4 가능한 R2 후보들 (R1 결과를 보고 결정)

| Candidate | Approach | Expected savings (예측 X — bloat 후 측정) |
|---|---|---|
| `toml = "0.8"` → `toml = "0.5"` 또는 `basic-toml` | state.toml은 단순 — feature 적게 | TBD |
| `nucleo` → `sublime_fuzzy` 또는 직접 fuzzy | matcher size 비교 | TBD |
| `ureq` → `attohttpc` 또는 `minreq` | v0.8 R7에서 시도 — native-tls는 잘 됐음 | TBD |
| `ratatui` widget feature 줄이기 | Table widget만 쓰는데 다 들어옴 | TBD |

이 후보들은 *R1 결과 + 사용자 합의* 후에만 진입한다.

## 4. Module-boundary R-G gates

신규 R-G 게이트 없음. v0.6에서 굳어진 9개 게이트 그대로 유효.

## 5. New dependencies

없음. G1은 *의존성 features 축소*이지 추가가 아니다.

## 6. Risks

| Risk | Mitigation |
|---|---|
| `env_logger` default-off가 wildcard / regex RUST_LOG에 의존한 사용자에게 회귀로 작동 | README에 prefix-only matching이 됐다고 명시. 또는 escape hatch로 `regex` feature를 별도 enable하는 빌드 옵션을 *문서화*. |
| 사이즈 회수가 v0.10 G2처럼 또 빗나감 | *측정 후 보고*가 G1 정의의 일부. 예측 숫자 commit 금지. R1 끝에 cargo bloat 다시 돌려서 변화량을 직접 본다. |
| `env_logger` 0.11에서 default-features=false 빌드 자체가 실패하는 케이스 | 빌드 fail 시 다음 가장 보수적 후보: `regex` feature만 빼고 다른 default는 유지. `env_logger = { version = "0.11", default-features = false, features = ["auto-color", "humantime"] }`. |

## 7. Definition of Done

- [ ] `cargo fmt --check` 클린.
- [ ] `cargo clippy --all-targets -- -D warnings` 클린 (host).
- [ ] `cargo clippy --all-targets --target x86_64-pc-windows-msvc -- -D warnings`
      클린.
- [ ] `cargo test --release` 클린 — 223 lib + 3 integration tests.
- [ ] R-G1..R-G9 클린.
- [ ] R1 후 cargo bloat 결과가 `regex_automata` / `regex_syntax` /
      `aho_corasick`을 top 10에서 제거했음을 확인.
- [ ] release 사이즈 측정값이 CHANGELOG에 *그대로* 기록 (예측 없음).
- [ ] cargo-dist Linux x64 artifact 사이즈가 v0.10.0의 1,515 KB
      대비 *감소*했음을 확인 (감소량은 측정 후 명시).
- [ ] README는 사용자에게 보이는 변경이 없으면 *수정 없음*. (G1은
      내부 deps만 — keymap / 폼 / doctor surface는 안 만짐.)
- [ ] CHANGELOG `[0.11.0]` 엔트리.
- [ ] `main.rs`는 여전히 ≤ 10 LOC (R-G0).

## 8. Round breakdown (PLAN_V0.11 시드)

| R | 내용 | 게이트 |
|---|---|---|
| R0 | git pull + R-G + cargo bloat baseline (DONE this session) | 통과 + bloat 측정 결과 BRIEF에 인용 |
| R1 | G1: `env_logger` default-features=false 적용 + cargo bloat 재실행 + 사이즈 측정 | regex_automata 등이 top 10에서 빠짐 + bloat 결과 commit 메시지에 *측정값으로* 기록 |
| R2 | (조건부) R1 결과가 -200KB 미만이면 추가 후보 1개 평가 + 시도 | 측정값 commit 메시지에 |
| R3 | 문서 (CHANGELOG, Cargo.toml 0.10.0→0.11.0), 측정값 그대로 기록, tag + push | cargo-dist 단일 dispatch + Linux 사이즈 v0.10.0 대비 감소 |

## 9. Out of scope

### 9.1 v0.12+로 미루는 항목

- **sort axis state persistence** (`BRIEF_V11_TODO §2.2`) — v0.10
  G5의 session-only 동작이 정확히 옳은지 사용자 피드백 보고 결정.
- **IdentityFile multi-value** (`BRIEF_V11_TODO §2.3`) — v0.10
  ForwardingListModal 패턴 재사용 가능. v0.12 후보.
- **Forwarding list reorder (↑/↓)** (`BRIEF_V11_TODO §2.4`).
- **doctor: SSHC_NO_OSC52 + Wayland 조합 sanity** (`BRIEF_V11_TODO §2.5`).

### 9.2 Project-wide anti-features (BRIEF_V10 §9.3 carry-over)

1. Self-built SSH client.
2. Secret / key management (passwords, keyfile content).
3. Team-shared catalogs, permissions, audit logs.
4. Web UI / daemon / always-on process.
5. Complete OpenSSH `config(5)` parser.

플러스 v0.9 / v0.10 reaffirmations (모두 유지):
- No SCP / file transfer.
- No `ssh-copy-id` integration.
- No parallel ssh / multi-host dispatch.
- No automatic update *download*.
- No identity enumeration.
- No always-on update check.

v0.11에서 어느 것도 완화하지 않는다.

## End of Brief.
