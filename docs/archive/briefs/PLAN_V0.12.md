# sshc v0.12.0 — Execution Plan

> Companion to `BRIEF_V12.md`. Three-goal UX cycle.

## 1. Roles

| Role | Who | Responsibilities |
|---|---|---|
| Architect | current Claude session | Reads `BRIEF_V12.md` + this plan. Applies one round at a time, commits, re-runs the verification gate. |
| User | d9ng | Approves DoD + final push. Reviews cargo-dist size deltas at R6. |

## 2. Round breakdown

```
R0  Baseline (DONE this session).
    - v0.11.0 on master, tag pushed, cargo-dist success.
    - macOS arm64 release: 2,726,848 bytes.
    - R-G1..R-G9 clean. 223 lib + 3 integration tests green.

R1  ListEditModal rename + ListKind extraction.
    - Rename file: src/ui/forms/forwarding_list.rs
      → src/ui/forms/list_edit.rs
    - Rename type: ForwardingListModal → ListEditModal
    - Introduce `ListKind` enum:
        pub enum ListKind {
            Forwarding(ForwardingKind),
            IdentityFile { candidates: Vec<PathBuf> },
        }
      (R1 only adds the variant for `Forwarding(_)`; R3 wires up
      `IdentityFile`.)
    - kind methods on ListKind delegate to existing forwarding
      validate/title for the Forwarding variant.
    - HostForm references update to use ListEditModal /
      ListKind::Forwarding(ForwardingKind::*).
    - No behaviour change. Every existing test passes verbatim.
    - 1 commit (refactor).

R2  G1 R2-A: Host::identity_file Vec migration.
    - Host::identity_file: Option<PathBuf> → Vec<PathBuf>.
    - BlockState::identity_file same shape.
    - parser: push per occurrence.
    - serializer: emit one line per entry.
    - Fixture sweep (8 hosts) via perl one-liner.
    - app/forms::open_modify_form / open_promote_form: pull
      first entry into String for the legacy form code (still
      single-path in R2; R3 promotes the row to a list).
    - build_host: legacy single String → wrap into single-entry
      Vec<PathBuf>. Empty path → Vec::new().
    - Round-trip test: serialize host with 3 IdentityFile lines,
      parse back, assert Vec preserved.
    - 1 commit (feat config/model).

R3  G1 R2-B: HostForm + ListEditModal IdentityFile wiring.
    - HostForm gains `identity_file_entries: Vec<String>` (paths
      stored as String for form symmetry with forwarding kinds).
    - fields[IDENTITY_INDEX] becomes summary cell (synced from
      identity_file_entries). The v0.7.1 ↑/↓ picker is removed
      from HostForm's top level.
    - Enter on IDENTITY_INDEX opens ListEditModal with
      ListKind::IdentityFile { candidates: self.identity_candidates }.
    - ListEditModal's edit mode re-implements the ↑/↓ candidate
      cycle behind ListKind::IdentityFile — only when editing
      that kind, ↑/↓ replace the buffer with the next/prev
      candidate. (Other kinds: ↑/↓ stays as it is — current
      behaviour ignores them in edit mode.)
    - ListKind::IdentityFile::validate accepts path-shape strings
      (the v0.7.0–v0.7.2 backslash rule moves here from
      HostForm::validate; cfg(windows) allows '\\').
    - build_host: parse each entry String → PathBuf (or skip
      empties).
    - Form modal integration test + ListEditModal IdentityFile
      candidate cycle test.
    - 1 commit (feat ui/forms).

R4  G2: ListEditModal reorder.
    - Add Shift+Up / Shift+Down handlers in browse mode.
    - + add row: Shift+arrows no-op.
    - Top entry Shift+Up no-op; bottom entry Shift+Down no-op.
    - Tests:
        * reorder_middle_entry_down_swaps_with_next
        * reorder_top_entry_up_is_noop
    - Hint line updates to include `Shift+↑/↓ reorder`.
    - 1 commit (feat ui/forms).

R5  G3: Sort axis state persistence.
    - src/state/schema.rs:
        + SortAxisPersisted enum (Alias / Recent / Reachability)
        + MemorySection::sort_axis: SortAxisPersisted with
          #[serde(default)]
    - SortAxis (in app::mod) gets `from_persisted` and
      `to_persisted` helpers — pure conversion, lives in app
      so state.toml stays R-G6 clean.
    - App::new reads state.memory.sort_axis.
    - App::cycle_sort_axis writes state.memory.sort_axis +
      best-effort state.save().
    - Tests:
        * test_sort_axis_loaded_from_state_on_new
        * test_cycle_sort_axis_persists_to_state_memory
    - 1 commit (feat app/state).

R6  Docs + release.
    - README + README.ko:
        * Manage table: S sort line gets "(remembered)" or similar.
        * Form section: IdentityFile row now opens a list modal
          like forwarding; v0.7.1 picker behaviour moved inside.
    - CHANGELOG [0.12.0] entry. Measured size deltas filled in
      AFTER the cargo-dist run lands.
    - Cargo.toml: 0.11.0 → 0.12.0.
    - cargo install --locked --path . --force local refresh.
    - 1 commit (chore release).
    - tag v0.12.0 + push.
    - Watch gh run list: confirm single Release workflow.
    - Compare 6 platform artifacts to v0.11.0 via gh api.
    - Report measured deltas — no predictions in commit messages.
```

Per-round verification gate (mandatory before commit):
```bash
cargo fmt --check \
  && cargo clippy --all-targets -- -D warnings \
  && cargo clippy --all-targets --target x86_64-pc-windows-msvc -- -D warnings \
  && cargo test --release
```
Plus R-G1..R-G9 grep matrix (`docs/TESTING.md §2`).

## 3. Step-by-step protocol

- Same measure-don't-predict rule as v0.11: commit messages
  quote actual numbers (test count, byte size) — never project.
- After R6 tag-push, if any platform GROWS vs v0.11.0, that's
  fine to ship (UX work has weight); just note the actual delta.

## 4. Architect-direct vs delegation

| Work | Owner | Notes |
|---|---|---|
| All rounds | architect | No subagent delegation — UX work + small refactors are tight cycles. |

## 5. Definition of Done

See `BRIEF_V12.md §7`. Plan-specific:

- [ ] R1 commit (refactor only — all v0.10 forwarding tests pass
      unchanged).
- [ ] R2 commit (storage + fixture sweep + round-trip test).
- [ ] R3 commit (form integration + ListEditModal IdentityFile
      kind + candidate cycle in edit mode).
- [ ] R4 commit (reorder + 2 tests).
- [ ] R5 commit (persistence + 2 tests).
- [ ] R6 commit (docs + version bump).
- [ ] `release.yml` produces exactly one workflow run for v0.12.0.

## 6. Risks (carried from BRIEF §6 + plan-specific)

| Risk | Mitigation |
|---|---|
| R1 rename surfaces a stale `forwarding_list` import somewhere | Compile error catches all. Trivial. |
| R2 v0.7.1 picker removal in R3 surprises existing users mid-form | Modal hint shows `↑/↓ pick from N candidates` in edit mode when ListKind::IdentityFile and candidates non-empty. README documents the change. |
| R3 backslash validation regression on Windows IdentityFile paths | The v0.7.2 cfg-split allowing `\` moves intact into ListKind::IdentityFile::validate. Windows-cross clippy + the existing v0.7.2 path-shape test catches it. |
| R5 state.toml migration: existing users have no sort_axis key | `#[serde(default)]` handles it. Manual test: copy a v0.11 state.toml and launch v0.12 — no error, defaults to Alias. |

## 7. Commit message format

```
<type>(<scope>): <subject>

<body — quoting measured numbers when relevant>

Refs: BRIEF_V12.md §<n>, PLAN_V0.12.md R<id>
Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
```

## End of Plan.
