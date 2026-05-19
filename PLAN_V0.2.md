# sshs v0.2.0 — Execution Plan

> Companion to `BRIEF_V2.md`. This document drives the actual implementation work:
> task units, parallel scheduling, per-task delegation payloads, and the architect's
> review checklist.
>
> Architect (this Claude session) is reviewer; implementer is `glm-5.1:cloud` via
> tunaLlama. Ollama Cloud allows 3 concurrent model sessions, so up to 3 task units
> may be delegated in parallel where dependencies allow.

---

## 1. Roles

| Role | Who | Responsibilities |
|---|---|---|
| Architect | this Claude session (Opus 4.7) | Owns BRIEF_V2.md. Reviews every delegated task output. Applies via Edit/Write. Runs verification gates. Makes commit/push decisions. |
| Implementer | glm-5.1:cloud via tunaLlama | Receives self-contained delegation payloads. Returns code/diffs. Does NOT touch git. |
| User | d9ng | Approves cross-task decisions, scope changes, and push moments. |

**Hard rule**: implementer outputs are advisory. The architect verifies cross-cutting
impact (call sites, signatures, regression-guard greps) before applying. No
implementer output is committed verbatim without review.

---

## 2. Task Inventory (from BRIEF_V2.md §13)

| ID | Title | LOC est. | Files | Depends | Risk |
|---|---|---|---|---|---|
| T1 | `error.rs` types | ~80 | `src/error.rs`, `src/lib.rs` (mod decl) | – | low |
| T2 | `tui/lifecycle.rs` + tests | ~180 | `src/tui/mod.rs`, `src/tui/lifecycle.rs`, `src/lib.rs` | – | high (panic + atomics) |
| T3 | `ui/status_bar.rs` + tests | ~60 | `src/ui/status_bar.rs`, `src/ui/mod.rs` | – | low |
| T4 | `exec/ssh.rs` rewrite | ~120 | `src/exec/ssh.rs` | T1 | medium (signal/exit) |
| T5 | `app.rs` extensions | ~150 | `src/app.rs` | T1, T3 | medium |
| T6 | `ui/list.rs` ★ marker | ~40 | `src/ui/list.rs`, `src/ui/layout.rs` | T3, T5 | low |
| T7 | mock_ssh + integration tests | ~120 | `tests/bin/mock_ssh.rs`, `Cargo.toml`, `tests/round_trip_test.rs` | T4 | medium |
| T8 | `main.rs` orchestration | ~80 | `src/main.rs` | T1, T2, T4, T5 | medium (race) |
| T9 | `docs/TESTING.md` | ~100 | `docs/TESTING.md` | T8 | low |

---

## 3. Parallel Schedule

```
                              ┌─ T1 (error.rs) ─┐
   R1 (3 in parallel) ───────┼─ T2 (lifecycle) ┼──┐
                              └─ T3 (status_bar)┘  │
                                                   │
   R2 (2 in parallel) ─── T4 (exec/ssh) ─── T5 (app.rs) ──┐
                                                          │
   R3 (2 in parallel) ─── T6 (ui/list) ─── T7 (mock+itest)┤
                                                          │
   R4 (serial) ─────────────── T8 (main.rs) ──────────────┤
                                                          │
   R5 (serial) ─────────────── T9 (TESTING.md) ────────── DONE
```

**R1 launches three concurrent delegations**. Architect reviews each as it returns,
applies, and verifies independently. R1 must fully complete (all three merged into
the working tree, build + clippy + fmt green) before R2 launches — because R2 tasks
import T1's `error.rs` types and reference T3's `StatusMessage`.

Per-round verification gates (run after applying all tasks in the round):

```bash
cargo build --release && cargo test && \
  cargo clippy --all-targets -- -D warnings && \
  cargo fmt --check
```

Plus the §9 BRIEF_V2 regression-guard greps.

---

## 4. Per-Task Delegation Payload Skeleton

Every delegation to glm-5.1:cloud follows this structure (template; concrete
payloads in §5 below):

```
TASK: <one-sentence outcome>

CONTEXT FILES YOU SHOULD READ FIRST (local paths — you have access):
- /Users/d9ng/privateProject/sshc/BRIEF_V2.md   (sections referenced below)
- <other local files relevant to this task>

CONTRACT (binding — see BRIEF_V2.md §<n>):
<copy the relevant CONTRACT block(s) verbatim>

INPUTS YOU MAY MODIFY:
- <file 1>
- <file 2>

INPUTS YOU MUST NOT TOUCH:
- <list of files outside this task's scope>

DELIVERABLES:
- Section 1: full content of <new file 1> (if creating new files)
- Section 2: full content of <new file 2>
- Section 3: unified diff for <existing file 1>
Separate sections by a single line of `===`.
No prose, no markdown fences, no commentary.

REGRESSION CHECKS (run mentally — architect will run them mechanically):
- grep -E "<forbidden pattern>" <file>   should produce no matches
- <other module boundary checks>
```

**Why "full content for new files, unified diff for edits"**: glm's diff context
matching is unreliable for edits (we observed mismatched hunks in P1 work). Full
files for new code, narrow diffs for surgical edits.

---

## 5. Concrete Delegation Payloads — R1 (parallel × 3)

Architect sends these as three concurrent `tuna_general_task` calls.

### 5.1 R1-T1 payload (error.rs)

```
TASK: Create src/error.rs defining typed error enums per the contract in
BRIEF_V2.md §6.2, and add `pub mod error;` to src/lib.rs.

CONTEXT FILES:
- /Users/d9ng/privateProject/sshc/BRIEF_V2.md  (read §6.2 CONTRACT)
- /Users/d9ng/privateProject/sshc/src/lib.rs

CONTRACT (verbatim from §6.2):
<copy §6.2 block>

INPUTS YOU MAY MODIFY:
- src/error.rs (NEW)
- src/lib.rs (add `pub mod error;` declaration only)

INPUTS YOU MUST NOT TOUCH: every other file.

DELIVERABLES:
- Section 1: full content of src/error.rs
- Section 2: unified diff for src/lib.rs (just adding `pub mod error;`)

REGRESSION CHECKS (architect runs):
- `cargo build --release` succeeds
- `grep "anyhow" src/error.rs` produces no matches (use std::io::Error only)
- All four error enums implement std::error::Error + Display
- AppError implements From<TerminalError>, From<SshError>, From<EditorError>, From<std::io::Error>
```

### 5.2 R1-T2 payload (tui/lifecycle.rs)

```
TASK: Create src/tui/mod.rs and src/tui/lifecycle.rs implementing the
TerminalGuard RAII type and install_panic_hook per BRIEF_V2.md §6.3.

CONTEXT FILES:
- BRIEF_V2.md §5 Q1, Q2; §6.3; §7 contracts; §11 notes 1-3
- src/main.rs (for reference on current crossterm usage — DO NOT modify)
- src/lib.rs

CONTRACT (verbatim §6.3):
<copy block>

ADDITIONAL CONTRACT (from §11):
- Set TERMINAL_ACTIVE = true AFTER successful enter; clear BEFORE leave.
- Drop must never panic — use `let _ = ...` on each crossterm call.
- Panic hook calls default_hook(panic_info), NOT eprintln!.

INPUTS YOU MAY MODIFY:
- src/tui/mod.rs (NEW; `pub mod lifecycle;` plus pub re-exports if needed)
- src/tui/lifecycle.rs (NEW)
- src/lib.rs (add `pub mod tui;`)

INPUTS YOU MUST NOT TOUCH: every other file. Do not modify main.rs — that's T8.

UNIT TESTS REQUIRED (in #[cfg(test)] mod tests inside lifecycle.rs):
- test_terminal_active_initial_false: confirm TERMINAL_ACTIVE starts false
- test_panic_hook_install_idempotent: install_panic_hook() twice doesn't double-wrap
  (hint: track via a separate AtomicBool guard inside the install function)
- test_drop_when_inactive_is_noop: a TerminalGuard whose suspend() was called drops
  without re-leaving (i.e. no double leave)

NOTE: real terminal calls (enable_raw_mode etc.) ARE called in tests because
crossterm is happy to no-op when no real TTY is present. If a CI without TTY
fails, gate the relevant assertions with `if std::io::stdout().is_terminal()`.

DELIVERABLES:
- Section 1: full content of src/tui/mod.rs
- Section 2: full content of src/tui/lifecycle.rs
- Section 3: unified diff for src/lib.rs

REGRESSION CHECKS:
- `cargo build --release` succeeds
- `grep -E "use .*config::model" src/tui/lifecycle.rs` — no matches
- `grep "eprintln" src/tui/lifecycle.rs` — no matches
- panic hook installation function is idempotent (verified by test)
```

### 5.3 R1-T3 payload (ui/status_bar.rs)

```
TASK: Create src/ui/status_bar.rs with StatusMessage type per BRIEF_V2.md §6.5,
and register it in src/ui/mod.rs.

CONTEXT FILES:
- BRIEF_V2.md §6.5, §11 note 6
- src/ui/mod.rs (current content)

CONTRACT (verbatim §6.5):
<copy block>

ADDITIONAL CONTRACT (from §11 note 6):
- Extract the visibility comparison into a helper that takes `now: Instant` as a parameter,
  so tests can assert without real-time sleeping. Public `is_visible(&self)` uses
  `Instant::now()`; private `is_visible_at(&self, now: Instant) -> bool` is the testable form.

INPUTS YOU MAY MODIFY:
- src/ui/status_bar.rs (NEW)
- src/ui/mod.rs (add `pub mod status_bar;`)

INPUTS YOU MUST NOT TOUCH: every other file.

UNIT TESTS REQUIRED:
- test_status_message_visible_immediately: new message returns true
- test_status_message_hidden_after_timeout: construct message, simulate "now = expires_at + 1ms"
  via is_visible_at, assert false
- test_status_message_text_accessor: text() returns input

DELIVERABLES:
- Section 1: full content of src/ui/status_bar.rs
- Section 2: unified diff for src/ui/mod.rs

REGRESSION CHECKS:
- `cargo build --release` succeeds
- no imports from crossterm, ratatui, config, exec
```

---

## 6. R2 / R3 / R4 / R5 payload outlines

Concrete payloads for R2–R5 are produced AFTER R1 lands, because they reference
exact types/signatures from R1. Outlines below — full payloads will be written
inline in the conversation when each round starts.

### R2-T4 (exec/ssh.rs) — depends on T1
- Rewrite ssh.rs per §6.4. Remove `CommandExt::exec`. Implement `ssh_run` returning
  `Result<SshResult, SshError>`. Implement `classify_exit_status` as a free function.
- 6 unit tests on `classify_exit_status` covering each `SshResult` variant.

### R2-T5 (app.rs) — depends on T1, T3
- Add `last_connected`, `status_message`, `pending_action` fields.
- Add `AppAction` enum.
- Add `on_ssh_finished`, `take_action`, `replace_hosts` (replaces `refresh_hosts`),
  `try_reconnect`, `dismiss_status_if_expired`.
- Selection-by-alias preservation logic.
- `r` key wiring (non-filter mode only).
- ~6 new unit tests.

### R3-T6 (ui/list.rs) — depends on T3, T5
- Render ★ prefix on rows where `host.alias == app.last_connected.as_deref()`.
- Render `app.status_message` in the layout (BRIEF_V2.md §6 says status bar lives
  in `ui/layout.rs` — implementer decides whether to extend `layout.rs` or render
  inline in `list.rs`. Architect prefers a separate row at the bottom of the layout
  managed by `layout.rs`).

### R3-T7 (mock_ssh + integration tests) — depends on T4
- Add `[[bin]] name = "mock_ssh" path = "tests/bin/mock_ssh.rs"` to Cargo.toml in
  `[[example]]` or a dev-only `[[bin]]` with `required-features` or just a build.rs
  that compiles it. Simpler: use `escargot` or just have the test build it via
  `cargo build --bin mock_ssh` at test setup. Implementer chooses the simplest
  portable approach — flag if it's not straightforward.
- 5 integration tests per §8.2.

### R4-T8 (main.rs) — depends on T1, T2, T4, T5
- Rewrite main.rs per §6.7. Must be ≤ 80 non-comment lines.
- All terminal calls go through `TerminalGuard`.
- All exit code interpretation delegated to `App::on_ssh_finished`.

### R5-T9 (TESTING.md) — depends on T8
- Manual test checklist from BRIEF_V2.md §10, expanded.
- Includes the build/test/clippy/fmt commands.
- Includes the regression-guard grep commands.

---

## 7. Per-Task Architect Review Checklist

For EACH task output before applying:

1. **Read the full output**. No skim review.
2. **Cross-reference call sites**: glm tends to miss callers when changing signatures.
   `grep` for every function name being renamed/retyped.
3. **Check `use` statements**: missing or extra imports cause compile noise.
4. **Run the regression-guard greps for the task's module** (BRIEF_V2 §9).
5. **Apply via Edit/Write** — never `git apply` on glm-produced diffs (mismatched hunks).
6. **Run `cargo build --release && cargo test`**. If a single test fails, do NOT
   bandage — diagnose root cause; the test or the implementation is wrong, decide which.
7. **Run `cargo clippy --all-targets -- -D warnings`**. Each warning is a real issue.
8. **Run `cargo fmt --check`**. Apply `cargo fmt` if needed.
9. **Inspect the diff against current `master`** — `git diff` for sanity.
10. **Defer commit until the round is complete**. Round-level atomic commits
    when possible; otherwise per-task commits.

---

## 8. Commit / Push Policy

- One commit per task ID (T1, T2, ...), unless an obvious atomic group emerges.
- Commit message format:
  ```
  <type>(<scope>): <subject>

  <body — what changed, why, references BRIEF_V2.md §X>

  Refs: BRIEF_V2.md §<n>, PLAN_V0.2.md T<id>
  Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
  ```
- `type`: `feat` for new module, `refactor` for rewrite, `test` for test-only, `docs` for docs.
- **Push only after a round's verification gate passes** AND user approves.
- v0.2.0 git tag: applied only after T9 + manual checklist sign-off.

---

## 9. Risks and Contingencies

| Risk | Mitigation |
|---|---|
| glm-5.1 diff mismatches | Always Write/Edit ourselves. Never git apply implementer output. |
| glm misses cross-cutting impact | §7 step 2 (grep call sites). Run build immediately after applying. |
| Parallel rounds produce conflicts in src/lib.rs | All three R1 tasks add to `lib.rs`. Architect serializes the lib.rs edits even though task content is parallel. |
| crossterm test fails without TTY in some envs | Use `is_terminal()` gate in lifecycle tests. |
| mock_ssh build complexity | If `[[bin]]` setup is painful, fallback to a checked-in shell script `tests/fixtures/mock_ssh.sh` (less portable; flag if used). |
| Signal handling test flakiness | If `test_round_trip_exit_signal` is flaky, mark `#[ignore]` and document in TESTING.md. |
| Behavioral surprise (e.g. parent killed by SIGINT) | Pause v0.2 work, escalate to user with concrete repro before patching. |

---

## 10. Definition of Done (v0.2.0)

All MUST be true to call v0.2.0 ready:

- [ ] T1–T9 complete and committed
- [ ] All v0.1.x tests (43) still pass
- [ ] New tests (≥ 25 estimated) pass
- [ ] `cargo clippy --all-targets -- -D warnings` 0 warnings
- [ ] `cargo fmt --check` clean
- [ ] All §9 regression-guard greps clean
- [ ] `docs/TESTING.md` manual checklist run by user on macOS Terminal + iTerm2
- [ ] No panic during 10 consecutive round-trips (manual)
- [ ] User explicit approval to tag v0.2.0
- [ ] `Cargo.toml` version bumped to `0.2.0`
- [ ] CHANGELOG entry (optional — discuss at T9)

---

## 11. Out-of-band issues we expect to surface (and where they go)

| If we discover... | We do... |
|---|---|
| ratatui API broke between versions | Architect investigates; user decides on upgrade vs pin |
| Signal handling needs ctrlc crate | Adds to Cargo.toml; pause to re-spec §5 Q3 |
| Mock ssh approach is fragile | Switch to shell script fallback; mark task R3-T7 mitigated |
| Selection-by-alias breaks user mental model | Pause, ask user; possible to revert to index-with-warn |
| Performance regression in render path | Out of scope — log as v0.3 issue |

---

## End of Plan.
