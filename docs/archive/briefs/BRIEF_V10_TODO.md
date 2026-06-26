# v0.10 — TODO (next session)

> Pre-BRIEF scratch. Next session: read this file + `BRIEF_V9.md §9`
> (deferrals) + the v0.9.0 changelog entry, then draft `BRIEF_V10.md` /
> `PLAN_V0.10.md` properly.

## Current state (end of session 2026-06-26)

- `master` at `dee6650` (v0.9.0 release tag pushed, cargo-dist run
  28206512333 completed success).
- macOS arm64 release size: **3.76 MB** (v0.7.3 baseline was
  ~3.0 MB → +760 KB, mostly arboard with its image/tiff transitive
  dependencies). ureq+native-tls landed cleanly; ring is gone.
- R-G1..R-G9 clean. 206 lib + 3 integration tests green on host.
- cargo-dist Windows ARM64 verified at v0.9.0 tag push — 19 release
  artifacts including `sshc-aarch64-pc-windows-msvc.zip`.
- v0.8 Windows debug handoff (`docs/WINDOWS_DEBUG_HANDOFF.md`) closed
  at §12; kept in-tree for reference.

## v0.10 candidates (rough priority — confirm with user before BRIEF)

| Priority | Item | Notes |
|---|---|---|
| 1 | **Multi-forwarding** | v0.9 G5 stored `local_forward / remote_forward / dynamic_forward` as `Option<String>` (last-wins). OpenSSH lets a host carry several `LocalForward` lines; sshc round-trips extras through `extra` but the form can't surface them. v0.10 either lifts the three fields to `Vec<String>` with a sub-form (Tab cycles between entries) or — simpler — adds a second "edit forwarding list" modal opened with a hotkey on the field. **Decision needed**: UX shape. |
| 2 | **Second-pass size recovery** | arboard pulls in `image` + `tiff` transitively (~+600 KB of decoders we never use). Candidates: `clipboard-anywhere` (smaller surface), `copypasta`, or make `arboard` an opt-out feature for users who don't need `c`. Measure before committing. Target: -300 KB. |
| 3 | **Wayland clipboard fallback** | When `arboard` fails on Wayland w/o `WAYLAND_DISPLAY` or in remote `tmux` sessions, fall back to OSC 52 escape sequence (which most modern terminals — kitty, iTerm2, foot, alacritty — honor). Detection by trying `arboard` first, falling back to OSC 52 on `ClipboardError::ContentNotAvailable` / unavailable backend. Pure stdio path stays anti-feature 4 friendly. |
| 4 | **doctor: ProxyCommand sanity** | Hosts that use `ProxyCommand` silently fail when the proxy binary isn't on PATH. doctor could iterate known proxies from sshc.conf, do a `which`-equivalent for each, and `WARN` on misses. Read-only, anti-feature 1 unaffected. |
| 5 | **`S` sort key (lazyssh parity)** | lazyssh has `s`/`S` for sort/reverse. We use `s` for ssh — so `S` (Shift+s) is free for sort. Sort axes: alias, hostname, recent-use, ProbeState (Open first). Could be useful for users with 100+ hosts. |
| Deferred | Windows agent forwarding via Pageant pipe re-broker | A user request after v0.8 G1 lands; out of scope here. |
| Deferred | Tags-second-class CLI flag (`sshc --tag prod`) | Possible v0.11 — needs CLI surface decision first. |

## Anti-features carry-over (BRIEF_V5/V6/V7/V8/V9 §9)

1. Self-built SSH client.
2. Secret / key management.
3. Team-shared catalogs.
4. Web UI / daemon / always-on process.
5. Complete OpenSSH `config(5)` parser.

Plus the v0.9-specific reaffirmations:
- No SCP / file transfer.
- No `ssh-copy-id` integration.
- No parallel ssh / multi-host dispatch.
- No automatic update *download* (G3 doctor surfaces availability;
  user runs their own installer).
- No identity enumeration on a discovered agent (anti-features 1+2).

These do not relax in v0.10.

## Session-start checklist for v0.10

1. `git pull --ff-only` (cargo-dist Homebrew tap may have advanced
   between sessions).
2. Confirm `~/.cargo/bin/sshc --version` is 0.9.0.
3. Re-run R-G1..R-G9 matrix from `docs/TESTING.md §2`.
4. Read `BRIEF_V9.md` (especially §3.7 size-recovery rationale and
   §3.5 forwarding shape) + this file + CHANGELOG `[0.9.0]` entry.
5. Settle the multi-forwarding UX (G1 above) with the user before
   touching code — the form ergonomics decide whether the storage
   model changes to `Vec`.
6. Draft `BRIEF_V10.md` covering the items the user picks.

## v0.9 retrospective notes (for the BRIEF_V10 §1 context paragraph)

- v0.9 R7 (ureq → native-tls explicit wire) was the single highest
  ROI patch in the project: PLAN target was -400 KB, actual was
  -2.18 MB *and* unblocked Windows MSVC cross-compile from macOS.
  Worth examining if any other "tried-it-once and gave up"
  decisions have the same shape.
- The R0 baseline gate caught no drift this cycle (vs v0.8 R0 which
  found the R-G8 regression from v0.7.1). Encouraging — the v0.8
  TESTING discipline is sticking.
- The Status sticky-vs-info split (G3) was the right call in
  retrospect: every v0.7-v0.8 cycle debug session had a moment
  where a failure message vanished before the user could screenshot
  it. The cost was ~30 LOC and seven targeted call-site edits.

## End of TODO.
