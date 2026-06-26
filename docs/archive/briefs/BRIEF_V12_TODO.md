# v0.12 — TODO (next session)

> Pre-BRIEF scratch. Next session: read this file + `BRIEF_V11.md §9`
> (deferrals) + the v0.11.0 changelog entry, then draft `BRIEF_V12.md` /
> `PLAN_V0.12.md` properly.

## Current state (end of session 2026-06-26)

- `master` at `9153b47` (v0.11.0 release tag pushed, cargo-dist
  run 28210788860 completed success, 19 release artifacts).
- **Real artifact sizes vs v0.10.0** (the measured table, no
  predictions):

  | Target | v0.10.0 | v0.11.0 | Δ | % |
  |---|---:|---:|---:|---:|
  | macOS arm64 | 1,103 KB | 807 KB | -297 KB | -26.9% |
  | macOS x86_64 | 1,234 KB | 886 KB | -348 KB | -28.2% |
  | Windows ARM64 | 1,492 KB | 1,054 KB | -438 KB | -29.4% |
  | Windows x64 | 1,569 KB | 1,095 KB | -473 KB | -30.2% |
  | Linux aarch64 | 1,376 KB | 1,061 KB | -315 KB | -22.9% |
  | Linux x86_64 | 1,516 KB | 1,152 KB | -364 KB | -24.0% |

  Single Cargo.toml line change (`env_logger`
  `default-features = false`) — the user-facing surface didn't
  move and the recovery exceeded the entire +164 KB Linux x64
  bloat v0.10 had introduced.

- macOS arm64 host binary (release): 3,982,280 → 2,727,504 bytes
  (-1.25 MB on the host, smaller than v0.7.3 pre-ureq baseline).
- R-G1..R-G9 clean. 223 lib + 3 integration tests green on host.
- v0.11 post-R1 `cargo bloat` ranking:
    1. std 397 KiB
    2. sshc 355 KiB
    3. **toml_edit 165 KiB**
    4. **ureq 118 KiB**
    5. ratatui 72 KiB
    6. nucleo_matcher 69 KiB
    7. url 44 KiB / idna 37 KiB (ureq transitive)

## v0.12 candidates (rough priority — confirm with user before BRIEF)

| Priority | Item | Notes |
|---|---|---|
| 1 | **IdentityFile multi-value** | Same shape as v0.10 G1 forwarding work. `Host::identity_file: Option<PathBuf>` → `Vec<PathBuf>`. OpenSSH allows multiple `IdentityFile` per host and chooses by trying them in order. Reuses the `ForwardingListModal` infrastructure — either generalise it to `ListEditModal<T>` or fork it into a sibling. The host form's IdentityFile row already has a v0.7.1 `↑/↓` picker for the *single* file case; v0.12 lets the row hold a Vec and the same modal pattern manages multi. |
| 2 | **Forwarding list reorder via ↑/↓** | v0.10's `ForwardingListModal` supports add/edit/delete only. Power users with many forwarding entries want to reorder. Adds ~20 LOC plus 2 unit tests. Worth bundling with G1 since both touch the same modal surface. |
| 3 | **Sort axis state persistence** (v0.10 G5 carryover) | v0.10's session-only behaviour: every fresh `sshc` re-starts on `AliasAlpha`. Dogfooded by the user across the v0.11 session — decide whether to persist in `state.toml::memory` or keep session-only. If persist, add the field to the `MemorySection` schema (v0.6 R1 pattern) and a one-line migration. |
| 4 | **Further size recovery: toml_edit slim** | Post-v0.11 top consumer at 165 KiB. `state.toml` is read/written with `toml = "0.8"` which transitively pulls `toml_edit` for format-preserving edits. sshc serializes state.toml from scratch each time — format preservation isn't load-bearing. Candidates: `toml = "0.5"` (no toml_edit) or `basic-toml`. Measure before committing. Don't predict a number — v0.11 protocol. |
| 5 | **doctor: variable-substituted ProxyCommand sanity** | v0.10 G4 skips tokens containing `%` or `$`. For a more thorough check, doctor could *expand* a small whitelist of substitutions (`%h` → hostname, `%p` → port, `%r` → user) for each host and re-walk PATH. Skips the shell-variable case but catches the common ProxyCommand-with-%h pattern. |
| Deferred | Forwarding entries on the preview panel | The v0.6 preview shows tags + extra; add forwarding count + first entry. Small but cosmetic. |
| Deferred | OSC 52 + tmux note in doctor | If `SSHC_NO_OSC52` isn't set and `TMUX` env var is, doctor could remind the user about `set -g set-clipboard on`. |

## Anti-features carry-over (BRIEF_V5..V11 §9)

1. Self-built SSH client.
2. Secret / key management.
3. Team-shared catalogs.
4. Web UI / daemon / always-on process.
5. Complete OpenSSH `config(5)` parser.

Plus v0.9 / v0.10 / v0.11 reaffirmations:
- No SCP / file transfer.
- No `ssh-copy-id` integration.
- No parallel ssh / multi-host dispatch.
- No automatic update *download*.
- No identity enumeration on a discovered agent.
- No always-on update check.

These do not relax in v0.12.

## Session-start checklist for v0.12

1. `git pull --ff-only`.
2. Confirm `~/.cargo/bin/sshc --version` is 0.11.0.
3. Re-run R-G1..R-G9 matrix from `docs/TESTING.md §2`.
4. Read `BRIEF_V11.md` + this file + CHANGELOG `[0.11.0]` entry.
5. Decide between Priority 1+2 bundled (multi-IdentityFile +
   reorder) vs Priority 1 alone vs broader scope. The
   ForwardingListModal generalisation decision (extract vs
   sibling) is the main design question to settle upfront.
6. Draft `BRIEF_V12.md` covering the items the user picks.

## v0.11 retrospective notes (for the BRIEF_V12 §1 context paragraph)

- **Measured-not-predicted worked.** R1's commit message quoted
  `3,982,280 → 2,727,504 bytes`, full stop. cargo-dist's actual
  artifacts came back at -22% to -30% across every target — the
  R1 host-binary measurement turned out to be a faithful preview
  of the broader picture, but the commit didn't promise anything
  about the artifacts because we hadn't measured them yet.
  Contrast with v0.10 R6 ("Estimated -600 to -800 KB per
  artifact") which was wrong on the same day.
- **Single-commit cycle is a viable shape** when the goal is
  small and measurable. R1 was the entire cycle's substantive
  work; R0 (baseline) and R3 (release) were thin wrappers. R2
  (further size candidates) was correctly skipped because R1
  alone hit the target by 5x.
- **`cargo bloat` is now part of the toolkit.** v0.11 R0
  installed `cargo-bloat 0.12.1` once; future BRIEF size sections
  can quote the actual top-15 by .text. Worth adding to
  `docs/TESTING.md` as an optional pre-BRIEF step when sizing is
  in scope.
- **env_logger removal didn't break log call sites.** The `log`
  facade itself stayed; we just don't have a subscriber that
  filters by regex any more. If anyone ever complains, the
  conservative fallback (re-enable just `regex` feature) is
  documented in BRIEF_V11 §6.

## End of TODO.
