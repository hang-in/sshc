# sshc 디버깅 플랜 (for glm-5.1)

> **수신자**: 다른 터미널 탭에서 같은 워킹 디렉터리(`/Users/d9ng/privateProject/sshc`)를 보고 있는 glm-5.1.
> **작성자**: Claude (Opus 4.7). 본 문서는 코드 리뷰 결과를 바탕으로 작성됨.
> **작업 대상 브랜치**: `master` (현재 HEAD: `975e4f9`)
> **목표**: 명세 대비 다운그레이드/누락된 동작 4건 + 테스트 보강을 우선순위에 따라 수정.

---

## 0. 프로젝트 요약 (컨텍스트 동기화용)

- **이름**: `sshc` — `~/.ssh/config`를 읽어 호스트를 TUI로 보여주고 선택 시 `ssh <alias>`로 접속하는 Rust CLI.
- **주요 모듈**:
  - `src/config/parser.rs` — SSH config 직접 파서, Include/순환 탐지/glob 지원
  - `src/config/model.rs` — `Host` 구조체 + fuzzy match
  - `src/app.rs` — TUI 상태/키바인딩
  - `src/ui/` — ratatui 렌더링 (layout, list, mod)
  - `src/exec/ssh.rs` — `exec("ssh", alias)`로 프로세스 교체
  - `src/exec/editor.rs` — `$EDITOR` + `+<line>` 플래그
  - `src/main.rs` — 이벤트 루프 + 패닉 훅 + 터미널 복원
- **의존성**: `ratatui 0.29`, `crossterm 0.28`, `nucleo 0.5`, `dirs 6`, `anyhow 1`, `log/env_logger`
- **테스트**: 14 unit + 6 integration + 10 parser = 30개, 모두 통과 (master 기준).
- **빌드**: `cargo build --release` 0.57초로 OK.

---

## 1. 발견된 문제 (5건, 우선순위 순)

각 항목은 **(A) 문제 / (B) 근거 / (C) 수정안 / (D) 검증** 형식으로 정리.

---

### 🔴 P0-1. Fuzzy 매칭이 명세 위반 (substring fallback)

**(A) 문제**
- `Cargo.toml`에 `nucleo = "0.5"`가 선언돼 있으나 `src/` 어디서도 import하지 않음.
- 실제 매칭은 단순 case-insensitive substring.
- 명세상 "wbsrv" → "web-server" 같은 비연속 fuzzy 매칭이 가능해야 함.

**(B) 근거 (파일:라인)**
- `Cargo.toml:14` — `nucleo = "0.5"` 선언만 존재
- `src/config/model.rs:33-43`:
  ```rust
  pub fn fuzzy_match(&self, query: &str) -> bool {
      if query.is_empty() { return true; }
      let query = query.to_lowercase();
      self.alias.to_lowercase().contains(&query)
          || self.hostname.as_ref().is_some_and(|h| h.to_lowercase().contains(&query))
  }
  ```
- `src/config/model.rs:32` 주석에 "inline substring matching is sufficient"로 자인.

**(C) 수정안**
1. `src/config/model.rs`에 nucleo 기반 매칭으로 교체:
   ```rust
   use nucleo::{Matcher, Config, pattern::{Pattern, CaseMatching, Normalization}};
   ```
   `Host`에 메서드 시그니처는 유지하되 내부적으로 `nucleo::Matcher`로 점수 계산.
2. 점수가 양수면 true, 0이면 false. 또는 `score(&self, query, matcher) -> Option<u32>`로 바꿔 정렬에도 활용.
3. `src/app.rs:91-102` `apply_filter`에서 `Matcher`를 매 호출마다 만들지 않도록 `App`에 보관(`Box<Matcher>` 또는 `RefCell<Matcher>`).
4. 정렬 옵션: fuzzy 점수 내림차순으로 `filtered` 정렬하면 UX 개선.

**대안 (간이판)**: nucleo 도입이 복잡하면 `nucleo` 의존성을 제거하고 substring 그대로 두되, 명세를 substring으로 갱신. 단, glm-5.1은 **nucleo 도입을 1순위로 시도**할 것.

**(D) 검증**
- `src/config/model.rs`에 신규 `#[cfg(test)] mod tests` 추가:
  - `fuzzy_match("wbsrv", host_alias="web-server") == true`
  - `fuzzy_match("xyz", ...) == false`
  - `fuzzy_match("", anything) == true`
  - 케이스 무시: `fuzzy_match("WEB", "web") == true`
- `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt --check` 통과.

---

### 🔴 P0-2. `Match` 디렉티브 미처리로 인한 잘못된 귀속

**(A) 문제**
- `parse_config_content`의 keyword arm에 `"match"`가 없음. 결과적으로 `Match` 블록 본문의 `HostName`/`User`가 **직전 `Host` 블록에 잘못 누적**됨.

**(B) 근거**
- `src/config/parser.rs:81-155` `match keyword.to_lowercase().as_str()` arm 목록: `host`, `hostname`, `user`, `port`, `identityfile`, `include`. `match` 없음.
- `_ if in_host_block` arm(`parser.rs:149-151`)이 "ignore but keep block active"라서 Match 직후 HostName이 그대로 직전 Host에 귀속됨.

**(C) 수정안**
`src/config/parser.rs:81`의 match에 `"match"` arm 추가:
```rust
"match" => {
    // Flush 직전 host block, Match 블록 본문은 무시
    if in_host_block && !current_aliases.is_empty() {
        flush_block(&mut hosts, &mut current_aliases, &current_hostname,
                    &current_user, current_port, &current_identity_file,
                    current_line_start, source_file);
    }
    in_host_block = false;
    current_aliases.clear();
    current_hostname = None;
    current_user = None;
    current_port = None;
    current_identity_file = None;
}
```
중복 flush 로직이 두 곳(파일 끝, Host arm)에 있으므로 **`flush_block` 헬퍼로 추출**할 것.

**(D) 검증**
- 신규 fixture `tests/fixtures/match_directive.config`:
  ```
  Host web
      HostName web.example.com

  Match host db
      HostName should-not-leak.example.com
      User leaked

  Host db
      HostName 192.0.2.1
  ```
- 테스트: `db.hostname == "192.0.2.1"` (Match가 web을 오염시키지 않음), `web.user`/`web.hostname` 변경 없음.

---

### 🔴 P0-3. 터미널 복원 race 가능성 (drop 순서 + panic hook 이중 출력)

**(A) 문제 3가지**
1. `ssh_connect` 호출 시점에 `terminal` 변수가 아직 살아있음 → 향후 Drop이 추가될 경우 exec 후 누락.
2. panic hook이 `eprintln!(panic_info)` + `default_hook(panic_info)` 둘 다 호출 → **panic info가 두 번 출력**.
3. `cmd.status()` 결과를 `let _status = cmd.status();`로 통째로 무시 (`main.rs:54`) → 에디터가 터미널을 망친 채 죽어도 그대로 TUI 재진입.

**(B) 근거**
- `src/main.rs:41-49`:
  ```rust
  restore_terminal(&mut terminal)?;
  result?;
  if app.should_connect {
      if let Some(host) = app.selected_host() {
          ssh_connect(&host.alias)?;
      }
  }
  ```
  `terminal`은 `main()` 끝까지 stack에 남아있음. exec 전 명시적 drop 없음.
- `src/main.rs:118-129`:
  ```rust
  panic::set_hook(Box::new(move |panic_info| {
      let _ = disable_raw_mode();
      let _ = execute!(io::stdout(), LeaveAlternateScreen);
      let _ = execute!(io::stdout(), crossterm::cursor::Show);
      eprintln!("{}", panic_info);   // <-- 첫 번째 출력
      default_hook(panic_info);       // <-- 두 번째 출력 (default가 또 stderr에 씀)
  }));
  ```
- `src/main.rs:53-54`:
  ```rust
  let mut cmd = build_editor_command(&host.source_file, host.line_start);
  let _status = cmd.status();
  ```
- crossterm 권장 순서: `LeaveAlternateScreen` → `disable_raw_mode`. 현재는 역순.

**(C) 수정안**
1. `src/main.rs`의 `ssh_connect` 호출 직전에 `drop(terminal);` 명시.
2. panic hook에서 `eprintln!("{}", panic_info)` 제거 (default_hook이 이미 출력함).
3. panic hook에서 `LeaveAlternateScreen` → `disable_raw_mode` 순서로 정렬.
4. `cmd.status()` 결과를 검사: 비정상 종료 시 `restore_terminal`을 다시 한 번 호출하고 로그.
5. `restore_terminal` 함수 자체도 동일 순서(`LeaveAlternateScreen` → `disable_raw_mode`)로 정렬할지 검토. 현재 `disable_raw_mode` 먼저(`main.rs:86-91`).

**(D) 검증**
- 자동화 어려움(터미널 상태). 다음 수동 시나리오:
  1. `kill -SEGV <pid>` 또는 `panic!()` 임시 삽입 후 터미널 복원 확인.
  2. 에디터를 일부러 `EDITOR=false sshc`로 실행해 즉시 실패 시 TUI 재진입이 깔끔한지 확인.
- 자동 테스트: panic hook 분리해서 함수형으로 단위 테스트 추가(터미널 호출은 mock 또는 feature gate).

---

### 🟡 P1. 파서 엣지 케이스 누락 (따옴표, 인라인 주석)

**(A) 문제**
- `split_directive`가 `HostName "my host"`의 따옴표를 그대로 hostname에 포함시킴.
- `HostName web.example.com # prod box` 같은 인라인 주석을 hostname에 포함시킴.
- 빈 Host 블록(`Host foo`만 있고 자식 디렉티브 0개)이 `hostname: None`인 채 push됨 — 의도 확인 필요.

**(B) 근거**
- `src/config/parser.rs:178-203` `split_directive`:
  - 첫 공백/`=`까지를 keyword로 자르고 나머지는 trim만. 따옴표 핸들링 없음.
- `src/config/parser.rs:71` `if line.is_empty() || line.starts_with('#')` — 라인 시작 `#`만 검사.
- `src/config/parser.rs:84,159` — 빈 블록 flush 가드는 `!current_aliases.is_empty()`만 보고 hostname 유무는 안 봄.

**(C) 수정안**
1. `split_directive` 개선:
   - value가 `"`로 시작하면 다음 `"`까지가 value, 이후는 인라인 주석 가능.
   - value 안에 ` #`(공백 + 샵)이 있으면 그 앞까지만 value. (SSH 공식은 인라인 주석 미지원이지만 흔한 실수, optional.)
2. 빈 Host 블록은 명세 확인 후 결정. 일단 그대로 두되 `Display`에서 `<no hostname>`은 유지.

**(D) 검증**
- `parser_test.rs`에 단위 케이스 추가:
  - `split_directive(r#"HostName "my host""#)` → `("HostName", "my host")` (따옴표 제거)
  - `split_directive("HostName a.com # comment")` → `("HostName", "a.com")` 또는 명세대로
- 신규 fixture `tests/fixtures/quoted.config` 추가.

---

### 🟡 P1. 테스트 커버리지 보강

**(A) 문제**
- `Host::fuzzy_match` 단위 테스트가 0개 (`src/config/model.rs`에 `#[cfg(test)]` 모듈 자체 없음).
- Include 순환 탐지(A→B→A) 직접 테스트 없음.
- `malformed.config` 테스트가 존재 여부만 확인, hostname 오염 여부 미검증 (`parser_test.rs:110-115`).
- `tests/integration_test.rs`가 사실상 단위 테스트 중복.

**(B) 근거**
- `grep "#\[cfg(test)\]" src/config/model.rs` → 0건.
- `tests/parser_test.rs:110-115`:
  ```rust
  assert!(aliases.contains(&"web"));
  assert!(aliases.contains(&"db"));
  ```
  hostname/port가 어떻게 오염됐는지 검증 없음.

**(C) 수정안**
1. `src/config/model.rs`에 `fuzzy_match` 테스트 4-5개 추가 (P0-1과 묶어 진행).
2. 신규 fixture로 순환 include 케이스(`circular_a.config` ↔ `circular_b.config`) + 테스트.
3. `test_parse_malformed_recovers`에 `web.hostname == "web.example.com"`, `db.port == None` 등 구체 assertion 추가.
4. (선택) `src/ui/`에도 layout 계산 함수에 단위 테스트 추가.

**(D) 검증**
- `cargo test` 30 → 40+ 케이스로 증가, 전부 통과.
- `cargo tarpaulin --skip-clean` 등으로 coverage 측정(선택).

---

## 2. 작업 순서 권장

```
P0-1 (nucleo)          → 영향 범위: model.rs, app.rs, model 테스트
P0-2 (Match 디렉티브)  → 영향 범위: parser.rs, 신규 fixture, parser_test.rs
P0-3 (터미널 복원)     → 영향 범위: main.rs, exec/ssh.rs
P1   (파서 엣지)       → 영향 범위: parser.rs, 신규 fixture, parser_test.rs
P1   (테스트 보강)     → P0-1, P0-2와 자연스럽게 묶임
```

각 P0 항목 완료 후 반드시:
```
cargo build --release
cargo test
cargo clippy -- -D warnings
cargo fmt --check
```

---

## 3. 커밋 분리 권장

1. `fix(matcher): use nucleo for real fuzzy matching` (P0-1)
2. `fix(parser): handle Match directive to prevent leakage` (P0-2)
3. `fix(tty): ensure terminal restoration order and drop before exec` (P0-3)
4. `feat(parser): handle quoted values and inline comments` (P1)
5. `test: add coverage for fuzzy match, circular includes, malformed input` (P1)

각 커밋 메시지 본문에 근거 라인 인용(이 문서 §1 그대로 활용).

---

## 4. 알려진 비목표 (이번 작업에서 손대지 말 것)

- ratatui 0.29 → 최신 마이너 업그레이드: scope out.
- TUI 테마/단축키 커스터마이징: out of scope.
- `crates.io` 배포 / GitHub Actions CI: out of scope.
- README 갱신: scope 내 (P0-1 nucleo 도입 시 fuzzy 매칭 설명 갱신만).

---

## 5. 질문/확인이 필요한 사항 (glm-5.1 → 사용자 ping 시점)

다음 결정사항이 필요할 때만 사용자에게 묻기:

1. **P0-1 nucleo 도입 시**: fuzzy 점수로 정렬할 것인지(`filtered` 순서 변경) — UX 변경 동반.
2. **P1 따옴표 처리**: SSH 공식 문법(따옴표 지원, 인라인 주석 미지원) 기준으로 갈지, 관대한 처리(인라인 주석도 처리)로 갈지.
3. **빈 Host 블록**: skip할지 keep할지 명세 확인.

확인 없이 진행해도 되는 사항(default 결정):
- `Match` 블록 본문은 무시 (P0-2). SSH는 Match를 조건부 적용에 쓰지만, sshc는 정적 호스트 목록만 다루므로 본문 무시가 안전.
- panic hook 이중 출력 제거 (P0-3). 기능 변경 아님.

---

**완료 보고 형식 권장**: 각 항목 완료 시 커밋 해시 + `cargo test` 결과 라인 수 + clippy 경고 0 확인 한 줄로 보고.
