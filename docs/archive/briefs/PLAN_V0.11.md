# sshc v0.11.0 — Execution Plan

> Companion to `BRIEF_V11.md`. Single-goal size recovery cycle.
> Architect-direct execution.

## 1. Roles

| Role | Who | Responsibilities |
|---|---|---|
| Architect | current Claude session | Reads `BRIEF_V11.md` + this plan. Applies one round at a time, commits, re-runs the verification gate. Records measured numbers, never predicted ones. |
| User | d9ng | Approves DoD + final push. Reviews actual cargo-dist size deltas at R3. |

## 2. Round breakdown

```
R0  Baseline (DONE this session).
    - v0.10.0 on master (commit fb9afe5), tag pushed, cargo-dist
      success.
    - macOS arm64 release: 3,982,280 bytes (3.80 MB).
    - `cargo bloat --release --crates -n 15` shows
        regex_automata 264.1 KiB
        regex_syntax   169.2 KiB
        aho_corasick   114.0 KiB
      total 547 KiB via env_logger → env_filter → regex.
    - R-G1..R-G9 clean. 223 lib + 3 integration tests green.

R1  G1: env_logger default-features off.
    - Cargo.toml:
        env_logger = "0.11"
      becomes
        env_logger = { version = "0.11", default-features = false }
    - cargo build --release; record new size.
    - cargo bloat --release --crates -n 15; assert
      regex_automata / regex_syntax / aho_corasick are GONE from
      the top 10.
    - cargo test --release; verify nothing broke. (log macros
      still compile because the `log` crate itself is fine; the
      absence of a regex filter doesn't change call sites.)
    - Manual smoke: `RUST_LOG=sshc=debug ~/.cargo/bin/sshc --doctor`
      shows logs (prefix match still works); a wildcard pattern
      like `RUST_LOG=ssh*=debug` no longer parses (acceptable —
      we never documented it).
    - 1 commit (deps).
    - **Do NOT put a predicted KB number in the commit message.**
      Report the measured delta only.

R2  (Conditional) further size candidates.
    Only enter R2 if R1 yields LESS than -200 KB on macOS arm64
    release. If R1 hits the target, skip to R3.

    Candidates, in priority order based on R0 cargo-bloat:
      (a) toml_edit (157.8 KiB) — see if a smaller toml parser
          (`basic-toml`, `toml = "0.5"`) round-trips state.toml.
      (b) nucleo_matcher (69.2 KiB) — replace with sublime_fuzzy
          or a hand-rolled fuzzy for sshc's small host lists.
      (c) url / idna (43.8 + 37.3 KiB) — only ureq pulls them in;
          see if a `ureq` feature flag drops them.

    Each commit must:
    - Try one candidate.
    - cargo bloat to verify the change in the dep graph.
    - Record measured size delta in the commit message.
    - If the candidate fails the build matrix or doesn't deliver,
      revert and move on.

R3  Docs + release.
    - CHANGELOG [0.11.0] entry. Two facts only:
        1. What changed (env_logger default off; any R2 candidates
           that landed).
        2. Measured size deltas — every cargo-dist artifact
           target. Pull these from cargo-dist's actual artifact
           list AFTER the tag-push run completes (insert as an
           erratum-style follow-up commit if needed, mirroring
           v0.10's lesson).
    - Cargo.toml: 0.10.0 → 0.11.0.
    - cargo install --locked --path . --force local refresh.
    - 1 commit (docs + chore).
    - tag v0.11.0 + push master + push v0.11.0.
    - Watch gh run list: confirm exactly ONE Release workflow run.
    - Compare every artifact size to v0.10.0 via `gh api`.
    - If the actual sizes don't match what the CHANGELOG suggests,
      ship an erratum or amend before announcing.
```

Per-round verification gate (mandatory before commit):
```bash
cargo fmt --check \
  && cargo clippy --all-targets -- -D warnings \
  && cargo clippy --all-targets --target x86_64-pc-windows-msvc -- -D warnings \
  && cargo test --release
```
Plus R-G1..R-G9 grep matrix (`docs/TESTING.md §2`).

## 3. Step-by-step protocol carried over from v0.10

- **R6's broken predict-and-ship pattern is what v0.11 exists to
  correct.** Commit messages this cycle quote measurements taken
  with `cargo bloat` and `cargo build --release; stat -f %z`, not
  educated guesses.
- Network surface stays in `src/exec/*.rs` + `src/doctor.rs`.
- v0.10 R7 stayed on cargo-dist's standard pipeline; no workflow
  edits planned for v0.11.

## 4. Architect-direct vs delegation

| Work | Owner | Notes |
|---|---|---|
| Cargo.toml edits (R1, R2) | architect | Trivial — one line each. |
| cargo bloat interpretation (R1, R2) | architect | Reading dep graph + picking the next swap; not delegatable. |
| README/CHANGELOG diff (R3) | architect | Small. |

## 5. Definition of Done

See `BRIEF_V11.md §7`. Mechanical part requires additionally:

- [ ] R1 commit + (maybe) R2 commits + R3 commit landed on master.
- [ ] Each commit message contains measured size delta, not
      predicted.
- [ ] `release.yml` produces exactly one workflow run for `v0.11.0`.
- [ ] At least one platform's artifact shrinks vs v0.10.0 — not
      strictly required (the artifact contains lots of moving
      parts) but if all six grew, R1 has a bug.

## 6. Risks (carried from BRIEF §6 + plan-specific)

| Risk | Mitigation |
|---|---|
| R1 turns out that `env_logger = { default-features = false }` won't compile in the dep graph (some feature transitively required) | Try the more conservative path: keep `auto-color` + `humantime`, only drop `regex`. Measure again. |
| Size delta on macOS arm64 looks small but Linux artifact (where v0.10 grew most) doesn't shrink either | R3 will say so. Don't massage the CHANGELOG; ship the numbers. v0.12 can take another swing. |
| User feedback after v0.10 introduces a new high-priority bug while we're in v0.11 | v0.11.1 hotfix pattern — same as v0.7.2 / v0.8.x. R1 → revert and patch path stays clean. |

## 7. Commit message format

```
<type>(<scope>): <subject>

<body — includes the measured size delta>

Refs: BRIEF_V11.md §<n>, PLAN_V0.11.md R<id>
Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
```

## End of Plan.
