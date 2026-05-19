# sshs v0.3.0 — Execution Plan

> Companion to `BRIEF_V3.md`. Drives the implementation: 15 tasks across
> 9 rounds (R0–R8), with parallel-vs-serial scheduling, delegation
> payload templates, and the architect's review checklist.
>
> Architect (this Claude session) reviews every delegated task output.
> Implementer is `glm-5.1:cloud` via tunaLlama. Ollama Cloud supports up
> to 3 concurrent model sessions; parallel rounds use that headroom.

---

## 1. Roles

| Role | Who | Responsibilities |
|---|---|---|
| Architect | this Claude session | Owns BRIEF_V3.md. Reviews every delegated output (cross-cutting check, regression greps, build/test). Applies via Edit/Write. Decides commit grouping. |
| Implementer | glm-5.1:cloud via tunaLlama | Receives self-contained payloads. Returns code/diffs for new files OR small surgical patches. Does NOT touch git. |
| User | d9ng | Approves cross-task decisions, scope changes, push moments. |

**Hard rule** (v0.2 retrospective): implementer outputs are advisory.
Architect verifies cross-cutting impact before applying. No implementer
output is committed verbatim without review.

**Per BRIEF_V3 §11 note 10**, work is split:
- **glm tasks**: greenfield modules with sharp contracts
  (`storage/*`, `setup/*`, `probe/*`, `state/*`, `ui/modal.rs`,
  `ui/forms/*`, `config/tags.rs`)
- **architect tasks**: modifications to existing files
  (`app.rs`, `ui/list.rs`, `config/parser.rs`, `config/model.rs`,
  `tui/runtime.rs`)

---

## 2. Task Inventory

From BRIEF_V3.md §13. Re-stated here with explicit owner.

| ID | Title | Files | Depends | Owner | LOC est. | Risk |
|---|---|---|---|---|---|---|
| T0 | v0.2 transitional cleanup | src/app.rs, src/tui/runtime.rs, tests | – | architect | 30 | low |
| T1 | error.rs additions | src/error.rs | T0 | glm | 100 | low |
| T2a | config/tags.rs (new file) | src/config/tags.rs | – | glm | 80 | low |
| T2b | Host.tags field + parser line recognition | src/config/model.rs, src/config/parser.rs | T2a | architect | 40 | low |
| T3 | state/* (load/save state.toml) | src/state/* | T1 | glm | 150 | medium |
| T4 | storage/* (flock, atomic write, include) | src/storage/* | T1 | glm | 250 | high |
| T5 | probe/* (pool, worker, state) | src/probe/* | T1 | glm | 200 | high |
| T6 | ui/modal.rs (generalized base) | src/ui/modal.rs | – | glm | 120 | medium |
| T7 | ui/forms/* (host_form, tag_form) | src/ui/forms/* | T6, T2a | glm | 250 | medium |
| T8 | ui/list.rs 5-col + tag prefix + source marker | src/ui/list.rs, src/ui/layout.rs | T2b | architect | 100 | medium |
| T9 | setup/* (first_run_flow) | src/setup/* | T3, T4 | glm | 180 | medium |
| T10 | app.rs extensions (mode, probe_states, actions) | src/app.rs | T1–T9 | architect | 250 | high |
| T11 | tui/runtime.rs probe wiring + modal dispatch | src/tui/runtime.rs | T5, T6, T10 | architect | 80 | medium |
| T12 | Integration tests (storage / setup / probe) | tests/*_test.rs | T4, T5, T9 | glm | 200 | medium |
| T13 | docs/TESTING update + CHANGELOG + README v0.3 | docs/, CHANGELOG.md, README.md | T11 | architect | 200 | low |

Total estimated new/changed LOC: ~2,030.

---

## 3. Parallel Schedule

```
R0  T0                                    architect (serial cleanup)
                                         |
R1  T1     T2a    T3                      glm × 3 (independent greenfield)
                                         |
R2  T2b   T9?(blocked by T3,T4)           architect (T2b parser/model patch)
                                         |
R3  T4     T5     T6                      glm × 3 (storage + probe + modal)
                                         |
R4  T7     T9                             glm × 2 (forms + setup)
                                         |
R5  T8                                    architect (5-col list)
                                         |
R6  T10                                   architect (app.rs core surgery)
                                         |
R7  T11                                   architect (runtime wiring)
                                         |
R8  T12                                   glm (integration tests)
                                         |
R9  T13                                   architect (docs / CHANGELOG / README)
```

Note: PLAN tasks numbered differently than BRIEF§13 numbering because
the architect split T2 into T2a (greenfield tags.rs, glm) and T2b
(existing-file patches to model.rs/parser.rs, architect).

Per-round verification gate:
```bash
cargo build --release \
  && cargo test \
  && cargo clippy --all-targets -- -D warnings \
  && cargo fmt --check
```
Plus the BRIEF_V3 §9 regression-grep matrix (R-G1..R-G8).

---

## 4. Per-Task Delegation Payload Skeleton

Identical to PLAN_V0.2.md §4:

```
TASK: <one-sentence outcome>

CONTEXT FILES YOU SHOULD READ FIRST (local paths):
- /Users/d9ng/privateProject/sshc/BRIEF_V3.md   (sections referenced below)
- <other local files relevant to this task>

CONTRACT (binding — copied verbatim from BRIEF_V3.md §<n>):
<paste the relevant block(s)>

INPUTS YOU MAY MODIFY:
- <file 1>
- <file 2>

INPUTS YOU MUST NOT TOUCH:
- <files outside this task's scope>

DELIVERABLES:
- Section 1: full content of <new file 1>
- (...)
Separate sections by a single line of `===`.
No prose, no markdown fences, no commentary.

REGRESSION CHECKS (architect runs):
- grep -E "<forbidden pattern>" <file>   should produce no matches
- <module boundary checks>
```

**Rule** (v0.2 retrospective): always Write/Edit the deliverable
ourselves; never `git apply` glm-produced diffs.

---

## 5. Concrete Delegation Payloads — R1 (parallel × 3)

To be issued as three concurrent `tuna_general_task` calls when R0
(architect cleanup) is committed and pushed.

### 5.1 R1-T1 payload (error.rs extensions)

```
TASK: Append three new error enums (StorageError, SetupError, ProbeError)
to /Users/d9ng/privateProject/sshc/src/error.rs, plus extend AppError
with From impls.

CONTRACT: <paste BRIEF_V3.md §6.2 verbatim>

INPUTS YOU MAY MODIFY:
- src/error.rs (append new enums + impls; do not remove or alter the
  existing v0.2 enums SshError / TerminalError / EditorError / AppError)

DELIVERABLE: full content of src/error.rs after your changes.

REGRESSION CHECKS:
- No `anyhow` usage in error.rs
- All four NEW error enums implement std::error::Error + Display
- All io::Error-wrapping variants implement source() -> Some(&inner)
- AppError grows From<StorageError>, From<SetupError>, From<ProbeError>
```

### 5.2 R1-T2a payload (config/tags.rs)

```
TASK: Create /Users/d9ng/privateProject/sshc/src/config/tags.rs with
three public functions per BRIEF_V3.md §6.4.

CONTRACT: <paste BRIEF_V3.md §6.4 verbatim>

INPUTS YOU MAY MODIFY:
- src/config/tags.rs (NEW)
- src/config/mod.rs (add `pub mod tags;`)

INPUTS YOU MUST NOT TOUCH: src/config/parser.rs, src/config/model.rs
(those are T2b — architect-direct).

UNIT TESTS REQUIRED:
- parse_tag_line: "# @tags: a, b" -> Some(vec!["a","b"]);
                  "# @tags:" -> Some(empty);
                  "# regular comment" -> None;
                  "  # @tags: A , b  , a " -> Some(vec!["a","b"]) (lowercase, dedup)
- render_tag_line: empty vec -> ""; ["x"] -> "# @tags: x"; ["a","b"] -> "# @tags: a, b"
- normalize_tag: "  Foo " -> Some("foo"); "" -> None; "   " -> None

DELIVERABLE — output two sections separated by `===`:
Section 1: full content of src/config/tags.rs
Section 2: unified diff for src/config/mod.rs (architect merges with parallel R1 lib.rs edits)
```

### 5.3 R1-T3 payload (state/* )

```
TASK: Create the state module per BRIEF_V3.md §6.11. Use the `toml`
and `serde` crates (add them to Cargo.toml as a unified diff in your
output).

CONTRACT: <paste BRIEF_V3.md §6.11 verbatim>

INPUTS YOU MAY MODIFY:
- src/state/mod.rs (NEW)
- src/state/schema.rs (NEW)
- Cargo.toml (add toml = "0.8" and serde = { version = "1", features = ["derive"] } — provide unified diff)
- src/lib.rs (add `pub mod state;` — diff)

UNIT TESTS REQUIRED:
- State::default() round-trip: serialize, deserialize, equal.
- load() with non-existent path returns default.
- save() then load() round-trip with a tempdir.
- version mismatch returns an error (e.g. write version=99, expect parse error).

DELIVERABLE — output 4 sections separated by `===`:
Section 1: full content of src/state/mod.rs
Section 2: full content of src/state/schema.rs
Section 3: unified diff for Cargo.toml
Section 4: unified diff for src/lib.rs
```

---

## 6. R3 outlines (post-R1 / R2 — concrete payloads written then)

### R3-T4 (storage/*)
Per BRIEF_V3.md §6.7 + §6.8. Files: `src/storage/{mod,path,writer,serializer,include_injector}.rs`. Cargo.toml: add `nix = { version = "0.29", features = ["fs"] }` for flock.

Required tests (in `tests/storage_test.rs`):
- atomic write round-trip on tempdir
- flock contention (spawn thread holding flock; assert second attempt returns LockHeldByOther)
- include_injector idempotency (call twice, second is no-op)

### R3-T5 (probe/*)
Per BRIEF_V3.md §6.9. Files: `src/probe/{mod,worker,state}.rs`.
Required tests (in `tests/probe_test.rs`):
- Probe a bound `TcpListener` on 127.0.0.1:0 → ProbeState::Open within 2s.
- Probe `192.0.2.1:22` (TEST-NET-1) → ProbeState::Failed within 2s.

### R3-T6 (ui/modal.rs)
Per BRIEF_V3.md §6.5. Single file. Architect's preference: enum-based modal kinds (Confirmation, Info, Form). FormState is a trait.

### R4-T7 (ui/forms/*)
Per BRIEF_V3.md §6.6. Files: `src/ui/forms/{mod,host_form,tag_form}.rs`. State-machine semantics enforced via FormOutcome enum.

### R4-T9 (setup/*)
Per BRIEF_V3.md §6.10. Files: `src/setup/{mod,detect,permissions}.rs`. Integration test against tempdir filesystem.

---

## 7. Architect-direct task notes

### T0 (cleanup of v0.2 transitional)
- Remove `App::should_quit`, `App::should_connect`, `App::should_edit` fields.
- Rewrite `run_event_loop` termination predicate to `app.pending_action.is_some()`.
- Update `handle_key` callers — no more should_* writes.
- Remove `App::refresh_hosts` alias (keep `replace_hosts` only); update any callers.
- Update tests that assert should_* to use take_action / pending_action.
- Single commit. Build / test green before R1.

### T2b (Host.tags + parser line recognition)
- Add `pub tags: Vec<String>` to Host with default empty.
- In parser, before each `Host` keyword: if previous non-blank line was `# @tags: ...`, attach tags to that block's BlockState.
- Update existing Host instantiations to include `tags: vec![]`.
- Update existing tests that build Host.

### T8 (ui/list.rs 5-col table + tag prefix + source marker)
- Replace current Line-based row with `ratatui::widgets::Row` + `Table`.
- 5 columns per BRIEF_V3 §5 Q6 constraints.
- Tag prefix renders in Alias cell when `!host.tags.is_empty()`.
- Source marker `· ` in Status column when `host.source_file != sshs.conf path`.
- `last_connected` still drives ★ via Status column.
- Status column now multi-purpose: ★ (last) / probe glyph / · (external) — combine; spec a single rendered char per row with priority `★ > probe > ·`. Actually the simplest: probe glyph as base; ★ overlays as separate column or replaces probe glyph when last == alias. Architect to decide at implementation time; prefer 2-char Status: `<probe><marker>` where marker is space or ★ or ·.

### T10 (app.rs extensions)
Sub-divide into surgical patches:
- T10a: add fields (mode, probe_states, state, probe_sender)
- T10b: AppMode enum + handle_key dispatch wrapper
- T10c: new AppAction variants
- T10d: filter syntax extension (@tag)
- T10e: form submit handlers (apply_form_add / apply_form_modify / apply_form_delete / apply_form_tags)
- T10f: probe update consumer (drain in event loop tick)

Each sub-patch verifies build green before the next.

### T11 (runtime.rs probe wiring)
- run_event_loop polls ProbePool::poll_updates each tick before draw.
- Apply updates to app.probe_states with generation check.
- Modal-mode short-circuit: when app.mode != AppMode::List, route key to modal handler instead of app.handle_key.

### T13 (docs + CHANGELOG + README)
- docs/TESTING.md: append v0.3 manual checklist + new grep gates R-G6/R-G7/R-G8.
- CHANGELOG.md: NEW. Initial entries: v0.2.0 (one-line summary) + v0.3.0 (feature list).
- README.md: update feature list, add screenshot of new layout, mention sshs.conf and Include injection, document `a/d/m/t/?` keys.

---

## 8. Per-Task Architect Review Checklist (v0.2-style)

Carried over from PLAN_V0.2.md §7. Reviewer must, before applying:

1. Read the full output. No skim.
2. Grep call sites for any function being renamed / retyped.
3. Check `use` statements (missing imports, dead imports).
4. Run §9 regression-grep matrix.
5. Apply via Edit / Write — never `git apply` glm diffs.
6. Run `cargo build --release && cargo test`. Failures: root-cause, not bandage.
7. Run `cargo clippy --all-targets -- -D warnings`.
8. Run `cargo fmt --check`. Apply `cargo fmt` if needed.
9. Inspect diff against master.
10. Defer commit until the round is complete.

---

## 9. Commit / Push Policy

- One commit per task ID unless an obvious atomic group emerges (e.g.
  R1 three tasks can land as one round-commit since lib.rs is shared).
- Tag bumps: only at release boundary (v0.3.0 at end of R9 + manual test).
- Commit message format:
  ```
  <type>(<scope>): <subject>

  <body>

  Refs: BRIEF_V3.md §<n>, PLAN_V0.3.md T<id>
  Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
  ```
- Push only after a round's verification gate passes AND user approves.

---

## 10. Risks and Contingencies

| Risk | Mitigation |
|---|---|
| flock semantics differ between macOS/Linux | Test on both before R8. `nix` crate normalizes. |
| `toml`/`serde` crate version conflicts | Pin to known-compatible majors (toml 0.8, serde 1). |
| probe thread leak on quit | Drop(ProbePool) joins with timeout, then detaches. Document. |
| terminal flicker on form open/close | Use ratatui's Layout::split to inset modal; full redraw on transition. |
| sshs.conf write fails mid-batch (form submit) | Status bar surfaces error. App in-memory state unchanged. User retries. |
| Include line conflicts with user's hand-edited config | Append after EOF respects existing structure. Backup .bak before write. User can revert. |
| Migration: v0.2 users see Include prompt unexpectedly | Modal copy is explicit: "First time using sshs v0.3?". User can decline → state.toml records. |
| Layout regression on small terminals | T8 column priority hides gracefully; manual test covers 80x24 and 60x24. |
| glm produces non-exhaustive matches for new enums | Architect checklist step 6 catches at build time. |

---

## 11. Definition of Done (v0.3.0)

All MUST be true to tag v0.3.0:

- [ ] R0–R9 commits landed on master
- [ ] T0..T13 complete
- [ ] All v0.2 tests pass + new ~30 tests (estimated)
- [ ] `cargo clippy --all-targets -- -D warnings` 0 warnings
- [ ] `cargo fmt --check` clean
- [ ] R-G1..R-G8 regression greps clean
- [ ] First-run flow tested manually on a clean account (or via `mv ~/.ssh ~/.ssh.bak`)
- [ ] Add / modify / delete via TUI verified
- [ ] Probe glyph cycle observed on real and unreachable hosts
- [ ] README.md updated with v0.3 features + screenshot
- [ ] CHANGELOG.md updated
- [ ] Cargo.toml `version = "0.3.0"`
- [ ] `git tag v0.3.0` + push
- [ ] GitHub release notes published

---

## 12. Out-of-band issues we expect to surface

| If we discover... | We do... |
|---|---|
| `Match` directive shows up in user configs and parser tags leak | Patch tag parsing to also flush on Match (was already a v0.1 fix) |
| Probe glyph rendering differs on Linux ttys | Document; add fallback ASCII set behind feature flag |
| sshs.conf needs schema versioning later | Add `# sshs-schema: 1` banner now (out of scope for v0.3) |
| Performance issue with > 200 hosts | Profile in v0.4; reduce probe frequency |
| User wants persistent connection counts | v0.4 feature, do not bolt on |

---

## End of Plan.
