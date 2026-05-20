# sshc v0.4.0 — Execution Plan

> Companion to `BRIEF_V4.md`. Drives the implementation: 7 tasks across
> 6 rounds (R0–R5). Architect (this Claude session) reviews every
> delegated task output. Implementer is `glm-5.1:cloud` via tunaLlama.

## 1. Roles

| Role | Who | Responsibilities |
|---|---|---|
| Architect | this Claude session | Owns BRIEF_V4.md. Reviews every delegated output (cross-cutting check, regression greps, build/test). Applies via Edit/Write. Decides commit grouping. |
| Implementer | glm-5.1:cloud via tunaLlama | Receives self-contained payloads with verbatim signatures. Returns full file contents separated by `===`. Does NOT touch git. |
| User | d9ng | Approves cross-task decisions, scope changes, push moments. |

**Hard rule** (carried from v0.3): implementer outputs are advisory.
Architect verifies cross-cutting impact before applying via Write/Edit.
No `git apply` of glm-produced diffs.

Split:
- **glm tasks**: greenfield modules with sharp contracts
  (`inline_app.rs`, `tui/inline_runtime.rs`, `tests/inline_*.rs`)
- **architect tasks**: modifications to existing files
  (`tui/lifecycle.rs`, `main.rs`, `app.rs` Enter/s/m rebind, `runtime.rs`
  re-organisation, docs)

## 2. Task inventory

From BRIEF_V4.md §7. Estimated LOC includes tests.

| ID | Title | Files | Depends | Owner | LOC est. | Risk |
|---|---|---|---|---|---|---|
| T0 | Inline viewport prototype (verification only) | `examples/inline_prototype.rs` | – | architect | 80 | medium |
| T1 | TerminalGuard ScreenMode + idempotent panic hook | `src/tui/lifecycle.rs` | T0 | architect | 120 | high |
| T2 | InlineApp + InlineAction + unit tests | `src/inline_app.rs`, `src/lib.rs` | T1 | glm | 280 | medium |
| T3 | Inline runtime (event loop + handle_connect_inline) | `src/tui/runtime.rs` (or new `src/tui/inline_runtime.rs`) | T1, T2 | glm | 100 | medium |
| T4 | App Enter/s/m rebind + read-only Enter→edit | `src/app.rs` | T1 | architect | 50 | low |
| T5 | main.rs CLI dispatch + run_inline / run_manage | `src/main.rs` | T2, T3, T4 | architect | 90 | medium |
| T6 | tests/inline_test.rs + manage rebind test updates | `tests/inline_test.rs`, `tests/*` | T5 | glm | 120 | low |
| T7 | docs: CHANGELOG v0.4, README v0.4, TESTING §8 + R-G9 | `CHANGELOG.md`, `README.md`, `docs/TESTING.md` | T6 | architect | 200 | low |

Total estimated new/changed LOC: ~1,040.

## 3. Parallel schedule

```
R0  T0                      architect (inline viewport prototype, scratch)
                            |
R1  T1                      architect (lifecycle: ScreenMode + panic hook)
                            |
R2  T2                      glm (InlineApp greenfield)
                            |
R3  T3   T4                 glm (T3 inline runtime) ∥ architect (T4 rebind)
                            |
R4  T5                      architect (main dispatch)
                            |
R5  T6                      glm (tests)
                            |
R6  T7                      architect (docs + version bump + release)
```

Per-round verification gate (mandatory before commit):
```bash
cargo build --release \
  && cargo test --release \
  && cargo clippy --all-targets -- -D warnings \
  && cargo fmt --check
```
Plus the BRIEF_V4 §9 regression-grep matrix (R-G1..R-G9).

## 4. Per-task delegation payload skeleton

```
TASK: <one-sentence outcome>

CONTEXT FILES YOU SHOULD READ FIRST (local paths):
- /Users/d9ng/privateProject/sshc/BRIEF_V4.md   (sections referenced below)
- <other local files relevant to this task>

CONTRACT (binding — copied verbatim from BRIEF_V4.md §<n>):
<paste the relevant block(s)>

INPUTS YOU MAY MODIFY:
- <file 1>

INPUTS YOU MUST NOT TOUCH:
- <files outside this task's scope>

VERBATIM DEPENDENCY SIGNATURES (already exist in repo):
<paste every type / function the task depends on, in code blocks>

DELIVERABLE:
- Section 1: full content of <new file 1>
- Section 2: full content of <new file 2>
Separate sections by a single line of `===`.
No prose, no markdown fences, no commentary outside code.

REGRESSION CHECKS (architect runs):
- <grep / clippy / test gate>
```

**Rule** (v0.2/v0.3 retrospective): always Write/Edit the deliverable
ourselves; never `git apply` glm-produced diffs.

## 5. Concrete payloads

### 5.1 R2-T2 — InlineApp greenfield

To be issued as a single `tuna_general_task` call after R1 (TerminalGuard
extension) is merged and pushed. Will paste verbatim:

- BRIEF_V4 §4 + §7.2
- Host signature
- nucleo::Matcher usage pattern from `src/app.rs::apply_filter`
- StatusMessage signature (so InlineApp can store reconnect-failure
  message even though inline mode doesn't render it the same way as
  manage)
- state::State signature for `new_with_state`

Tests required (in `src/inline_app.rs #[cfg(test)] mod tests`):
- `new` / `new_with_state` smoke
- Immediate-filter: typing 'a' appends to query and re-filters
- `Esc` clears query when non-empty; quits when empty
- `Ctrl+C` always quits
- `Enter` on non-empty filtered list emits `Connect(alias)`
- `Enter` on empty filtered list is no-op
- `r` with last_connected = Some(...) emits `Reconnect`
- `r` with last_connected = None emits no action (no status text in
  inline; logged or status-bar handling is the runtime's job)
- `j/k`, `↑/↓` navigation wraps
- Filter restricts then Backspace expands

### 5.2 R3-T3 — Inline runtime

Pasted verbatim:
- BRIEF_V4 §4.5 + §7.4
- InlineApp signature (just produced in R2)
- TerminalGuard::suspend signature (R1)
- ssh_run signature
- ratatui Viewport::Inline construction (architect supplies the working
  pattern verified in T0 prototype)

Deliverable: append two functions to `src/tui/runtime.rs`, OR new file
`src/tui/inline_runtime.rs`. Architect will decide layout at integration
time; payload accommodates either.

### 5.3 R5-T6 — Integration tests

Pasted verbatim:
- BRIEF_V4 §11 manual checklist (mechanical greps + lifecycle assertions)
- `crossterm::event::KeyEvent::new` pattern

Tests:
- `tests/inline_test.rs` (3 tests) — InlineApp round-trip integration:
  load fixture hosts → simulate keys → assert action sequence.
- Manage rebind delta to `src/app.rs` tests:
  - `test_app_enter_opens_form` (replaces v0.3 `test_app_enter_connect`)
  - `test_app_s_key_connects` (new)
  - `test_app_enter_on_external_host_opens_editor` (new)

## 6. Architect-direct task notes

### T0 — Inline viewport prototype
- File: `examples/inline_prototype.rs` (similar pattern to v0.3's
  `examples/render_preview.rs`).
- Purpose: verify ratatui 0.29 `Viewport::Inline(15)` behaviour and
  the suspend → ssh-fake → exit sequence in a real terminal.
- Validates: viewport renders below prompt, raw-mode toggle works,
  no terminal lockup on exit, panic restoration usable.
- May be deleted after R1 lands, OR kept in `examples/` as a smoke
  test. Architect decides based on its size.

### T1 — TerminalGuard ScreenMode
Sub-divide into surgical patches:
- T1a: introduce `ScreenMode` enum + `acquire(mode)` signature change.
  Update existing `TerminalGuard::acquire()` callers (main.rs) to pass
  `ScreenMode::Alternate` so v0.3 build stays green.
- T1b: implement Inline branch — `Terminal::with_options(.., Viewport::Inline(h))`,
  no AlternateScreen, raw mode on, cursor hidden.
- T1c: panic hook becomes idempotent. Use `OnceLock<ScreenMode>` (or
  similar static) so the hook knows which leave path to take.
- T1d: TerminalGuard::suspend handles both branches symmetrically; Drop
  does the right thing for the active mode.

Each sub-patch verifies build green before the next.

### T4 — App Enter/s/m rebind
- `handle_list_key` (not in filter_mode):
  - `KeyCode::Enter` → if `selected_host().source_file == sshc.conf`: emit
    nothing here, instead call `self.open_modify_form()`. Otherwise emit
    `AppAction::EditConfig` (architect: confirm whether emit-vs-call
    semantics; both are pre-existing patterns).
  - `KeyCode::Char('s')` → existing v0.3 `Enter` body (emit Connect).
  - `KeyCode::Char('m')` → remove. ('m' becomes unbound. `?` help text
    updates.)
- `?` help modal text updates: "Enter open / s ssh / a add / d del / t
  tags / e edit / r reconnect / q quit".
- Test updates:
  - `test_app_enter_connect` → `test_app_s_connects` (rename + key swap).
  - New `test_app_enter_opens_form` (assert mode becomes Modal(Form)).
  - New `test_app_enter_on_external_opens_editor` (assert pending_action
    == EditConfig).

### T5 — main.rs dispatch
Split into:
- `parse_mode()` — std::env::args walk.
- `terminal_height()` — crossterm::terminal::size + recover from error.
- `run_inline(h)` — state.toml load (defaults if absent), hosts parse,
  TerminalGuard::acquire(Inline(h)), event loop, suspend, ssh, exit.
- `run_manage()` — v0.3 flow refactored into a function returning
  `ExitCode`. AppAction loop body unchanged.

main.rs stays under R-G4 (≤80 non-comment lines). The two run_*
functions live in main.rs or a new `src/runtime/mod.rs` if they grow.

### T7 — Docs
- `CHANGELOG.md` v0.4.0 entry: inline mode, manage key rebind, default
  change, list shrinkage (no tag column in inline), backward-compat note.
- `README.md`: lead with `sshc` (inline). Manage mode promoted to its
  own subsection with `-m` flag. Keybindings table split into Inline /
  Manage.
- `docs/TESTING.md` §8 (new): v0.4 manual checklist (inline first-time,
  inline panic, inline ssh round-trip, manage Enter rebind, manage `s`,
  manage read-only Enter→editor, terminal too small → manage fallback).
- R-G9 added to §2.

## 7. Per-task architect review checklist

Before applying a glm-produced output:

1. Read the full output. No skim.
2. Grep call sites for any function being renamed or retyped.
3. Check `use` statements (missing imports, dead imports).
4. Verify FormState / Constraint / KeyEvent API matches the installed
   crate versions (ratatui 0.29, crossterm 0.28). Watch for deprecated
   methods like `highlight_style` → `row_highlight_style`.
5. Run §3 regression-grep matrix.
6. Apply via Write / Edit — never `git apply` glm diffs.
7. Run `cargo build --release && cargo test --release`. Failures:
   root-cause, not bandage.
8. Run `cargo clippy --all-targets -- -D warnings`.
9. Run `cargo fmt --check`. Apply `cargo fmt` if needed.
10. Inspect diff against master.
11. Defer commit until the round is complete.

## 8. Commit / push policy

- One commit per task ID. Logical group commits OK when the round is
  small (e.g. R3-T3 + R3-T4 combined if both are < 50 LOC each).
- Tag bump only at release boundary (`v0.4.0` at end of R6).
- Commit message format:
  ```
  <type>(<scope>): <subject>

  <body>

  Refs: BRIEF_V4.md §<n>, PLAN_V0.4.md T<id>
  Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
  ```
- Push only after a round's verification gate passes AND user approves.

## 9. Risks and contingencies

| Risk | Mitigation |
|---|---|
| ratatui Viewport::Inline + ssh suspend leaves residue in shell | T0 prototype validates the exact escape sequence used. If residue is bad, add `terminal.clear()` + explicit cursor MoveTo before suspend. |
| crossterm leave-alternate corrupts inline-only terminals | T1c panic hook gates LeaveAlternateScreen on `OnceLock<ScreenMode>`; only call if mode == Alternate. |
| Default-mode change confuses muscle memory (Enter = ssh → form) | Single-user tool, low blast radius. README explains. `?` help modal updated. |
| Inline mode height detection wrong on tmux/screen | `crossterm::terminal::size()` returns total size; tmux works. screen older versions: ¯\_(ツ)_/¯. Document. |
| Removing `m` breaks user's wrist memory | Acknowledged. `?` help modal lists current bindings. |
| ssh round-trip in inline mode + Ctrl-C confusion (Ctrl-C in TUI vs Ctrl-C in ssh) | Inline event loop must catch Ctrl-C only before ssh spawn. Once `guard.suspend()` is called, terminal is in cooked mode; Ctrl-C goes to ssh. |
| Tests for inline lifecycle hard to write headlessly | Cover InlineApp logic in unit tests; rely on T0 prototype + manual checklist for the lifecycle bits. |

## 10. Definition of Done (v0.4.0)

See BRIEF_V4 §11. PLAN-side checks:

- [ ] T0 prototype verified manually on macOS Terminal + iTerm2
- [ ] R1–R6 commits landed on master
- [ ] R-G1..R-G9 grep matrix clean
- [ ] cargo test --release 185+ passing
- [ ] cargo clippy --all-targets -- -D warnings 0
- [ ] cargo fmt --check clean
- [ ] Inline manual checklist signed off
- [ ] Manage rebind manual checklist signed off
- [ ] CHANGELOG, README, TESTING updated
- [ ] Cargo.toml 0.4.0
- [ ] tag v0.4.0 pushed
- [ ] GitHub release published

## 11. Out-of-band issues we expect to surface

| If we discover... | We do... |
|---|---|
| ratatui Viewport::Inline misbehaves on tmux | Document; recommend `-m` as workaround |
| Inline mode probe support requested mid-cycle | Defer to v0.5 explicitly (already in Out of Scope) |
| Performance issue with > 500 hosts in inline | Profile; consider lazy render. v0.5 work. |
| User wants Enter in manage to behave like v0.3 (ssh) | Single-user — revisit. If yes: revert T4, document. |

## End of Plan.
