# v0.8 — TODO (next session)

> Pre-BRIEF scratch. Next session: read this file + `BRIEF_V7.md §9.1`
> (deferrals) + the v0.7.1 changelog entry, then draft `BRIEF_V8.md` /
> `PLAN_V0.8.md` properly.

## Current state (end of session 2026-05-20)

- `master` at `ac3a9c0` (v0.7.1 release tag pushed).
- All R-G gates clean, 162 unit tests + integration tests green on
  both Unix host build and `x86_64-pc-windows-msvc` cross-check.
- cargo-dist Windows artifact + powershell installer in place.

## v0.8 priorities (user decision, 2026-05-20)

| Priority | Item | Notes |
|---|---|---|
| 1 | **Pageant / Windows OpenSSH agent socket discovery** | doctor's SSH_AUTH_SOCK arm currently reports "Windows: not applicable". v0.8 detects the actual Windows agent state — Pageant named pipe `\\.\pipe\pageant` or the Microsoft OpenSSH `\\.\pipe\openssh-ssh-agent` — and reports PASS when present, WARN otherwise. No identity enumeration, just presence. |
| 2 | **Promote feature: external host → sshc.conf** | User suggested 2026-05-20: in manage mode, selecting an external host (currently `$EDITOR`-only) gains a key (`M` migrate?) that pre-fills the add form with its fields and writes a new entry to `sshc.conf`. User has to manually delete the original `~/.ssh/config` entry (sshc still must not touch user-authored config — anti-feature 1 stands). |
| Deferred | Windows ARM64 (`aarch64-pc-windows-msvc`) | cargo-dist target add is one line, but there's no runner to exercise the binary. Skip until that changes. |
| Deferred | ACL enforcement for "private key files must be private" on Windows | Still in BRIEF_V7 §9.1 deferral list. |

## Anti-features carry-over (BRIEF_V5/V6/V7 §9)

1. Self-built SSH client.
2. Secret / key management (passwords, keyfile content).
3. Team-shared catalogs, permissions, audit logs.
4. Web UI / daemon / always-on process.
5. Complete OpenSSH `config(5)` parser.

These do not relax in v0.8.

## Session-start checklist for v0.8

1. `git pull --ff-only` (cargo-dist may have pushed Homebrew tap updates).
2. Confirm `sshc --version` is 0.7.1 (or whichever release is latest).
3. Re-run R-G1..R-G9 matrix from `docs/TESTING.md §2`.
4. Read `BRIEF_V7.md` (especially §4.4 SSH_AUTH_SOCK rationale) +
   this file + CHANGELOG `[0.7.1]` entry.
5. Draft `BRIEF_V8.md` covering items 1+2 above.

## End of TODO.
