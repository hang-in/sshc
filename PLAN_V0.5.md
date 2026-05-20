# sshc v0.5.0 — Execution Plan

> Companion to `BRIEF_V5.md`. Drives the refactor + one chosen
> feature. Single-author execution; commit-per-step pacing.

## 1. Roles

| Role | Who | Responsibilities |
|---|---|---|
| Architect | next Claude session | Reads BRIEF_V5.md + this plan. Applies one commit at a time. Runs the gate after every commit. |
| Reviewer (optional) | deepseek-v4-pro:cloud via ollama CLI | Sanity-check the post-refactor `src/app/` layout (`ollama run --hidethinking deepseek-v4-pro:cloud < prompt`). See `docs/REFACTOR_NOTES.md` §B for the recipe. |
| User | d9ng | Approves the v0.5 feature choice (§5) and the final push. |

The split work is too sensitive for a glm-style delegation — `app.rs`'s
visibility and accessor surface is the kind of cross-cutting change
the v0.3 retrospective explicitly flagged as architect-direct. Use
the reviewer for second-opinions, not for code generation.

## 2. Round breakdown

```
R0  Read BRIEF_V5 + REFACTOR_NOTES. Run R-G1..R-G9 baseline. Confirm app/mod.rs
    state matches v0.4.3 (filter.rs already extracted; sshc_conf_path cached).

R1  Move tests block to src/app/tests.rs.                              (architect, ~1 commit)

R2  Extract input.rs: handle_key + handle_list_key + handle_modal_key + 
    dispatch_modal_action + activate_selected.                          (architect, 1 commit if it lands clean, 2-3 if it doesn't)

R3  Extract forms.rs: open_*_form / open_help_modal + apply_form + 
    apply_add / apply_modify / apply_delete / apply_tags + 
    persist_sshc_conf + host_from_payload (free fn).                    (architect, 1-2 commits)

R4  Fold in the deepseek (5)+(6) fixes:
      - apply_form destructure → build_host helper → apply_add/modify
        accept already-built Host
      - normalized_tags(csv) helper                                     (architect, 1 commit)

R5  One feature from BRIEF §6. Architect implements. New unit/integration
    tests. README + CHANGELOG entry.                                    (architect, 1-2 commits)

R6  Cargo.toml 0.5.0 + CHANGELOG release entry. Tag + push.
    cargo-dist publishes automatically.                                 (architect, 1 commit + tag)
```

Per-round verification gate (mandatory before commit):
```bash
cargo build --release \
  && cargo test --release \
  && cargo clippy --all-targets -- -D warnings \
  && cargo fmt --check
```
Plus R-G1..R-G9 grep matrix (one-liner in `docs/TESTING.md` §2).

## 3. Step-by-step protocol for the split (R1–R3)

The v0.4.3 attempt failed by doing too much in one shot. v0.5
moves **one method at a time** with this protocol:

1. Identify a single target method (e.g. `handle_key`).
2. Open the destination file (`src/app/input.rs`). Add or extend an
   `impl super::App { ... }` block.
3. Copy the method body verbatim into the new file. Adjust visibility
   to `pub(super)` if it's called from siblings.
4. **Do not delete from mod.rs yet.**
5. `cargo build --release` — expect duplicate-definition errors.
   Confirm the build only complains about the duplicate, nothing else.
6. Now delete the body from `mod.rs`.
7. `cargo build --release && cargo test --release`. If green, commit
   the single-method move. If red, the failure is localized and easy
   to revert with `git checkout HEAD -- src/app/`.

Repeat per method. Free functions (`host_from_payload`) are simpler —
move the whole `fn ... { ... }` block, adjust the `use` imports at
the top of `forms.rs`.

**Hard rule from v0.4.3 retrospective**: never delete a range like
"lines 87 to 354" from mod.rs. Always delete the exact method body
that you just copied. `wc -l src/app/mod.rs` should decrease in
predictable chunks.

## 4. R1 — tests.rs (warm-up)

The tests block is self-contained — it pulls in `use super::*;` and
its only outside-the-block dependency is the `make_host` /
`make_host_with_tags` helpers, which sit inside the block already.

Steps:

1. Locate the `#[cfg(test)] mod tests { ... }` block in
   `src/app/mod.rs`.
2. Create `src/app/tests.rs` with the entire block's body (without
   the outer `#[cfg(test)] mod tests {` wrapper — just the inner
   `use super::*;` + functions + helpers).
3. Replace the block in `mod.rs` with `#[cfg(test)] mod tests;`.
4. Build + test. Commit.

This is the lowest-risk move and validates the muscle memory before
the structural splits.

## 5. R5 — feature choice

User picks ONE from BRIEF_V5 §6. Architect recommendation in priority
order:

1. **`sshc <alias>` direct-connect** (small, ships ssh-friendly shell
   workflow on top of existing parser + ssh_run).
2. **Host quick-clone (`c` key in manage mode)** (small, exercises
   the form pre-fill code path that v0.5's forms.rs split already
   touched).
3. **Favourites toggle** (medium, persists across sessions, gives
   real daily-use value, but takes a state.toml schema bump).

Whichever the user picks, BRIEF §6 already sketches it. No new BRIEF
document needed.

## 6. Commit message format

```
<type>(<scope>): <subject>

<body>

Refs: BRIEF_V5.md §<n>, PLAN_V0.5.md R<id>
Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
```

Scopes used in v0.4 cycle: `app`, `ui`, `tui`, `inline`, `main`,
`config`, `setup`, `storage`, `state`, `probe`, `exec`, `ci`,
`chore(release)`, `docs`, `test`.

## 7. Definition of Done

See BRIEF_V5.md §8. Mechanical part of the checklist also requires:

- [ ] R0–R6 commits landed on master
- [ ] Every commit individually builds + tests green (no "wip" /
      broken commits)
- [ ] No file in `src/app/` exceeds 280 non-comment lines
- [ ] deepseek-v4-pro sanity review of the post-split layout obtained
      and any high-priority comment addressed (optional but recommended;
      see `docs/REFACTOR_NOTES.md` §B for the prompt template)

## 8. Risks (carried from BRIEF §7)

| Risk | Mitigation |
|---|---|
| Accidental delete-too-much (v0.4.3 trap) | §3 protocol: copy → confirm-duplicate-error → delete-from-source. |
| Test helpers (`make_host`) referenced outside `tests` block | They aren't, per v0.4.3 review. Audit with `rg "make_host\(" src/` before R1; if any non-test caller appears, move to a `#[cfg(test)] mod test_helpers` in mod.rs instead of into tests.rs. |
| Feature creep into the split commits | Feature work (R5) lands AFTER R4 commits are pushed. Don't mix. |
| cargo-dist publish fails on `v0.5.0` tag push | HOMEBREW_TAP_TOKEN was set on 2026-05-20 and verified on v0.4.2 rerun. If it fails again: same manual `dist generate`-style recovery used for v0.4.1, documented in `docs/REFACTOR_NOTES.md`. |

## End of Plan.
