# sshc v0.6.0 — Execution Plan

> Companion to `BRIEF_V6.md`. Drives the favorites/recent/preview/
> validation work. Architect-direct execution; commit-per-step.

## 1. Roles

| Role | Who | Responsibilities |
|---|---|---|
| Architect | next Claude session (or current) | Reads BRIEF_V6.md + this plan. Applies one round per session block, commits, re-runs the verification gate. |
| Reviewer (optional) | deepseek-v4-pro:cloud via `ollama run --hidethinking` | Sanity-check the post-R6 module layout. **Note**: `TERM=dumb` does NOT suppress ollama's wrap codes. Strip ANSI after capture with `perl -pe 's/\e\[[0-9;]*[a-zA-Z]//g'` until the recipe in `docs/REFACTOR_NOTES.md §B` is replaced with an HTTP-API direct call. |
| User | d9ng | Approves DoD + final push. |

## 2. Round breakdown

```
R0  Baseline. Confirm v0.5.1 state: 147 unit + integration tests green,
    R-G1..R-G9 clean, fmt + clippy clean, --doctor still works.

R1  State schema bump (BRIEF §3).
    - Add favorites: Vec<String>, recent: Vec<RecentEntry> to State.memory.
    - serde(default) + LegacyMemory shim that accepts last_connected_alias.
    - tests/fixtures/state_v05.toml + state_migration_from_v05_loads test.
    - migrate-on-load: legacy alias → recent[0] with ts = state.toml mtime.
    - 1 commit.

R2  Favorites toggle + first-tier sort (BRIEF §2.A.1).
    - App::toggle_favorite(alias) + App::is_favorite(alias).
    - filter.rs sort: favorites float to top (BEFORE fuzzy score).
    - 'f' key in input.rs (manage-mode only); inline emits status message.
    - persist on toggle via existing storage::with_locked_write path.
    - new tests: toggle round-trip, sort with mixed pinned/unpinned.
    - 1 commit.

R3  Recent secondary sort (BRIEF §2.A.2).
    - record_recent(alias) called from connect path (inline + manage).
    - bounded RECENT_MAX = 20, oldest dropped.
    - filter.rs sort: favorites > recent.ts desc > fuzzy.
    - new tests: record_recent insertion/dedupe, comparator with three signals.
    - 1 commit.

R4  Inline mode status line (BRIEF §2.A.3 + §5 inline).
    - inline_app render: one-line "HostName / User / Port" below selection,
      gated on viewport >= 5 rows.
    - ★ glyph in front of pinned hosts.
    - 1 commit (small).

R5  Manage-mode preview panel (BRIEF §2.A.4 + §5 manage).
    - src/ui/preview.rs new widget (HostName/User/Port/IdentityFile/Tags/Extra).
    - manage layout: split widget. Below 100 cols, hide preview, full-width list.
    - tests: render_preview snapshot for {minimal host, host-with-extra, missing-fields}.
    - 1-2 commits.

R6  ssh -G validation (BRIEF §2.B + §6).
    - src/exec/ssh_config.rs: validate_alias(alias) -> Result<String, ValidationError>.
    - Command::new("ssh").arg("-G").arg(alias) with 5s wall-clock guard.
    - App.validation_cache: HashMap<String, String>.
    - App::invalidate_validation_cache() in apply_add/modify/delete/tags.
    - 'v' key in input.rs (manage only) → status "Validating <alias>…" →
      modal with raw stdout.
    - 1-2 commits.

R7  Release.
    - help text: 'f' and 'v' in manage section.
    - README + README.ko: two new rows in keybindings, mention validation/favorites.
    - CHANGELOG [0.6.0] entry.
    - Cargo.toml 0.6.0.
    - tag v0.6.0 + push.
    - 1 commit + tag.
```

Per-round verification gate (mandatory before commit):
```bash
cargo build --release \
  && cargo test --release \
  && cargo clippy --all-targets -- -D warnings \
  && cargo fmt --check
```
Plus R-G1..R-G9 grep matrix (`docs/TESTING.md §2`).

## 3. Step-by-step protocol carried over from v0.5

Same v0.4.3 retrospective rules apply (one method at a time when moving
across modules; copy → confirm dup error → delete → build+test → commit):

- Never `git rm` a range of lines from `src/app/mod.rs` blindly.
- When promoting a private method to `pub(super)` for a sibling
  module call, do that promotion in its own commit.
- `wc -l src/app/*.rs` should not balloon — preview/favorites split
  goes to `src/ui/preview.rs` and (if it grows >50 LOC) `src/app/favorites.rs`.

## 4. Architect-direct vs delegation

| Work | Owner | Notes |
|---|---|---|
| All `src/app/` and `src/exec/` edits | architect (you) | Visibility surgery, state machine wiring. Too sensitive for glm-style delegation. |
| Widget render code (`src/ui/preview.rs`) | could delegate | Pure render fn taking `&Host` + `Rect`. Possible candidate for `tunaLlama:tuna-developer` if spec is tight enough. **Decision**: architect-direct for v0.6 to avoid spec-drift. |
| README/CHANGELOG diff | architect | Small, depends on final keybind names. |
| BRIEF/PLAN review pass | deepseek-v4-pro:cloud | Run once after R6 land, before R7 release. Strip ANSI. |

## 5. Definition of Done

See BRIEF_V6 §9. Mechanical part requires additionally:

- [ ] R0–R7 commits landed on master.
- [ ] Each commit independently builds + tests green (no "wip" commits).
- [ ] No `src/app/*.rs` exceeds ~290 non-comment lines after R5.
- [ ] `src/exec/ssh_config.rs` is the only place a `Command::new("ssh").arg("-G")` appears.
- [ ] deepseek-v4-pro sanity review obtained after R6 (optional but recommended).

## 6. Risks (carried from BRIEF §8 + plan-specific)

| Risk | Mitigation |
|---|---|
| Recent-history sort changes inline-mode picker order unexpectedly mid-session | R3 lands the comparator behind `record_recent` so the order is only updated after a successful connect. No mid-session re-sort. |
| `★` glyph width breaks fixed-column layout | Inline uses ratatui-cell-width; `★` is single-cell BMP. Verify with `unicode-width` on viewport calc. Audit before R4. |
| `ssh -G` blocks UI on a slow `~/.ssh/config` Include chain | 5-second wall-clock guard kills the child. UI shows "validation timed out" modal. |
| Preview panel width math breaks below 100 cols | Hard fallback at runtime — recompute every redraw, never cache. |

## 7. Commit message format

```
<type>(<scope>): <subject>

<body>

Refs: BRIEF_V6.md §<n>, PLAN_V0.6.md R<id>
Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
```

Scopes from v0.5 cycle: `app`, `ui`, `tui`, `inline`, `main`, `cli`,
`config`, `setup`, `storage`, `state`, `probe`, `exec`, `doctor`,
`chore(release)`, `docs`, `test`. v0.6 adds `app/favorites` and
`exec/ssh_config` as sub-scopes if clarity helps.

## End of Plan.
