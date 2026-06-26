# v0.11 — TODO (next session)

> Pre-BRIEF scratch. Next session: read this file + `BRIEF_V10.md §9`
> (deferrals) + the v0.10.0 changelog entry, then draft `BRIEF_V11.md` /
> `PLAN_V0.11.md` properly.

## Current state (end of session 2026-06-26)

- `master` at `fb9afe5` (v0.10.0 release tag pushed, cargo-dist
  run 28209045223 completed success, 19 release artifacts including
  ARM64 + x64 Windows).
- **Real artifact sizes vs v0.9.0** (downloaded via `gh api`):

  | Target | v0.9.0 | v0.10.0 | Δ |
  |---|---|---|---|
  | macOS arm64 | 1,086 KB | 1,103 KB | +16 KB |
  | macOS x86_64 | 1,216 KB | 1,233 KB | +17 KB |
  | Windows ARM64 | 1,478 KB | 1,492 KB | +13 KB |
  | Windows x64 | 1,544 KB | 1,568 KB | +24 KB |
  | Linux aarch64 | 1,213 KB | 1,376 KB | **+163 KB** |
  | Linux x86_64 | 1,351 KB | 1,515 KB | **+164 KB** |

  v0.10 R6's commit message and CHANGELOG entry projected
  -600 to -800 KB on Linux/Windows from disabling
  `arboard`'s `image-data` feature. The cargo-dist runner came
  back with the opposite shape — every platform grew, Linux
  most of all. The R6 strip is still correct in principle but
  the *net* win didn't materialise because R3/R4/R5's new
  code outweighed it. The v0.10.0 GitHub Release page now
  carries an erratum noting this; CHANGELOG.md itself was left
  intact (commits remain part of master history; readers see
  the erratum on GitHub when they go to download).

- R-G1..R-G9 clean. 223 lib + 3 integration tests green on host.

## v0.11 candidates (rough priority — confirm with user before BRIEF)

| Priority | Item | Notes |
|---|---|---|
| 1 | **Actual size recovery, instrumented** | Take v0.10.0's 1.5 MB (Linux x64) baseline and find the real bloat. `cargo bloat --release --crates --target x86_64-unknown-linux-gnu` first, then per-symbol. Likely suspects after R7's gains were spent: `ureq` + native-tls deps, `nucleo` fuzzy matcher, `ratatui` table renderer. Target: get x64 Linux back under 1.3 MB. **Don't predict a number in the commit message this time — measure and report.** |
| 2 | **Sort axis state persistence** (v0.10 G5 carryover) | After dogfooding v0.10, decide whether session-only is the right shape or if `state.toml` should remember the last axis. If keep session-only, document the rationale; if persist, add to `state::memory`. |
| 3 | **IdentityFile multi-value** | Same shape as v0.10 G1 forwarding — `Option<PathBuf>` → `Vec<PathBuf>` with the same list-modal pattern. OpenSSH allows multiple IdentityFile per host. Reuses `ForwardingListModal` infrastructure if generalised, or builds a sibling modal otherwise. |
| 4 | **Reorder (↑/↓) inside ForwardingListModal** | v0.10 G1's modal supports add/edit/delete only; users with many forwarding entries would want to reorder. Trivial keystroke addition. |
| 5 | **doctor: check OSC 52 escape hatch sanity** | If the user has `SSHC_NO_OSC52=1` in their shell and is on Wayland w/o display, `c` will fail. doctor could mention this combination if it detects both. |
| Deferred | tag/host re-use stats | "These tags are unused" / "These hosts are stale (never connected)". Anti-feature 3-adjacent if it turns into shared state; OK if purely local. |
| Deferred | sshc → ssh agent forwarding hint | doctor could note when a host has `ForwardAgent yes` but the agent is unreachable. |

## Anti-features carry-over (BRIEF_V5..V10 §9)

1. Self-built SSH client.
2. Secret / key management.
3. Team-shared catalogs.
4. Web UI / daemon / always-on process.
5. Complete OpenSSH `config(5)` parser.

Plus v0.9 / v0.10 reaffirmations:
- No SCP / file transfer.
- No `ssh-copy-id` integration.
- No parallel ssh / multi-host dispatch.
- No automatic update *download* (doctor surfaces availability;
  user runs their own installer).
- No identity enumeration on a discovered agent.
- No always-on update check (only inside `--doctor`).

These do not relax in v0.11.

## Session-start checklist for v0.11

1. `git pull --ff-only` (cargo-dist may have pushed Homebrew tap
   updates between sessions).
2. Confirm `~/.cargo/bin/sshc --version` is 0.10.0.
3. Re-run R-G1..R-G9 matrix from `docs/TESTING.md §2`.
4. Read `BRIEF_V10.md` (especially §3.2 size-recovery rationale +
   the erratum on v0.10.0's GitHub Release page) + this file +
   CHANGELOG `[0.10.0]` entry.
5. Run `cargo bloat --release --crates` and `cargo bloat --release`
   on Linux x86_64 to get the actual top consumers before
   committing to a G1 approach.
6. Draft `BRIEF_V11.md` covering the items the user picks.

## v0.10 retrospective notes (for the BRIEF_V11 §1 context paragraph)

- **R6's commit message predicted a number it didn't have data
  for.** "Estimated -600 to -800 KB per artifact based on the
  transitive bytes that no longer link" turned out to be wrong on
  the same day. Rule for v0.11: **don't put predicted size deltas
  in commit messages — only put measured ones, even if it means a
  follow-up commit after the cargo-dist run lands**. R7's tag-time
  measurement was the moment to learn, not the moment to ship the
  prediction.
- **R2 (ForwardingListModal) wiring went smoother than v0.9 R5
  (form section + Vec wiring) thanks to keeping the child modal
  inside HostForm rather than promoting it to AppMode**. Worth
  preserving as a v0.11 pattern if IdentityFile multi-value lands
  (priority 3).
- **R7's tag time of <1 minute from "git push" to "cargo-dist
  starts" continues to be reliable.** Concurrency guard is doing
  its job; haven't seen a duplicate Release run since v0.7.2.

## End of TODO.
