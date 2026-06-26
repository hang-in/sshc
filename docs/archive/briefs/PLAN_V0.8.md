# sshc v0.8.0 — Execution Plan

> Companion to `BRIEF_V8.md`. Drives the Windows-agent-detection +
> external-host promote + doctor-update-check work. Architect-direct
> execution; one commit per step.

## 1. Roles

| Role | Who | Responsibilities |
|---|---|---|
| Architect | next Claude session (or current) | Reads `BRIEF_V8.md` + this plan. Applies one round per session block, commits, re-runs the verification gate. |
| Reviewer (optional) | ollama-hosted reviewer (deepseek/glm) via `tunaLlama:delegate-to-ollama` | Optional sanity-check after R5 (G2 surface settles) and after R6 (G3 dep + http surface). Strip ANSI after capture. |
| User | d9ng | Approves DoD + final push. Provides Windows host for manual matrix in R2/R6. |

## 2. Round breakdown

```
R0  Baseline + CI guard.
    - git pull --ff-only (cargo-dist Homebrew tap may have advanced).
    - Confirm v0.7.3 state: cargo test green host + windows-cross,
      clippy clean both targets, fmt clean, R-G1..R-G9 clean,
      main.rs <= 10 LOC.
    - .github/workflows/release.yml:
        on: { push: { tags: ... }, pull_request: {} }
        + concurrency:
            group: release-${{ github.ref }}
            cancel-in-progress: false
      Goal: same SHA never triggers two parallel Release runs again
      (v0.7.2 lost one of two runs to "release already exists").
    - cargo build --release sanity baseline for size delta in R6.
    - 1 commit (chore(ci)) + 0 functional changes.

R1  G1 — Windows agent named pipe detection (BRIEF §3.1).
    - New helper in src/doctor.rs:
        #[cfg(windows)]
        fn windows_agent_pipe_present(path: &str) -> bool {
            // CreateFileW(path, GENERIC_READ, 0, NULL, OPEN_EXISTING,
            //             FILE_ATTRIBUTE_NORMAL, NULL); close on success.
        }
    - check_ssh_auth_sock() Windows arm rewritten:
        OpenSSH pipe present  -> PASS
        Pageant pipe present  -> PASS
        both                  -> PASS (merged detail)
        neither               -> WARN "no agent pipe found (start
                                 Windows OpenSSH agent service or Pageant)"
    - Unix arm untouched.
    - Unit test for pipe-existence helper on Windows (cargo test
      --target x86_64-pc-windows-msvc compiles).
    - 1 commit (feat(doctor)).

R2  G1 — message polish + docs.
    - Verify detail strings show literal pipe path so users can grep
      them when reporting.
    - README + README.ko: §"doctor 출력" small update — 6 checks
      become 7 (with G3) but for now mention "Windows agent pipe".
    - Manual matrix on a real Windows host: stop ssh-agent service,
      run sshc --doctor → WARN. Start it → PASS.
    - 1 commit (docs).

R3  G2 — manage-mode `M` key routing (BRIEF §3.2.1–3.2.2).
    - src/app/input.rs:
        + fn promote_selected(&mut self): no-op unless source is
          external; emits "'<alias>' already managed by sshc.conf"
          when called on a managed host (no panic, no state change).
    - bind 'M' (Shift+m) in manage-mode key handler. Inline mode:
      ignore — promote is a manage-only concept.
    - src/ui/list.rs or footer hint: when external host is selected,
      hint becomes
        Enter open in $EDITOR • M promote to sshc.conf (original kept) • …
    - Unit tests:
        promote_selected on managed host → status_message set, no
          state change, no pending_action.
        promote_selected on external host → emits OpenPromoteForm
          (new variant; details land in R4).
    - 1 commit (feat(app)).

R4  G2 — form prefill + sshc.conf write path (BRIEF §3.2.3).
    - src/app/forms.rs:
        + fn prepare_promote(&self, host: &Host) -> Result<FormState,
                                                          PromoteError>
            errors: AliasCollision, WildcardAlias
        + open_modify_form gains a "from external" entry point that
          fills every field config::model exposes for that host.
    - On submit: payload goes through the existing add path
      (storage::with_locked_write → sshc.conf). NO new writer
      entry point — R-G2 stands.
    - status bar on success:
        "'<alias>' promoted to sshc.conf — original ~/.ssh/config
         entry left intact, delete it manually if duplicate ssh -G
         output bothers you"
    - status bar on collision:
        "'<alias>' already exists in sshc.conf — promote aborted"
    - status bar on wildcard:
        "wildcard alias '<alias>' cannot be promoted — sshc only
         manages explicit aliases"
    - Integration test (tests/round_trip_test.rs pattern):
        external host in ~/.ssh/config + empty sshc.conf
        → promote
        → sshc.conf contains the alias, ~/.ssh/config byte-equal.
    - 1–2 commits (feat(app/forms), test(round_trip)).

R5  G2 — UI footer + status messages settle.
    - Confirm footer hint in inline + manage views render correctly
      below 100 cols (preview panel still hides; new hint must not
      push status_bar off-screen).
    - Unit tests for the 3 status-message variants (success /
      collision / wildcard) via StatusMessage matcher.
    - Optional review delegation (deepseek-v4-pro:cloud) for the
      G2 surface diff. Strip ANSI on capture.
    - 1 commit (ui).

R6  G3 — doctor update check (BRIEF §3.3).
    - Cargo.toml: ureq = { version = "2.10", default-features = false,
                          features = ["native-tls"] }
    - src/doctor.rs:
        + fn check_latest_version() -> Check
            - if env::var("SSHC_NO_UPDATE_CHECK").is_ok() → PASS skip
            - GET https://api.github.com/repos/hang-in/sshc/releases/latest
              with User-Agent "sshc/<version>", connect_timeout 2s,
              read_timeout 3s.
            - parse tag_name via simple substring; strip "v" prefix.
            - compare against env!("CARGO_PKG_VERSION") with a
              local fn compare_versions(a, b) -> Ordering.
            - map (Equal | Greater) → PASS, Less → WARN with URL,
              network/parse failure → WARN "could not reach github
              (offline?)".
    - run() check list grows from 6 to 7; the new "update" line
      appears last.
    - Unit tests (5 branches): equal, current ahead, current behind,
      malformed tag_name, env var skip. Use a fake responder
      bound on 127.0.0.1 (mockito if available, else hand-rolled
      TcpListener fixture).
    - DoD: cargo build --release size delta measured. v0.8 R6 actual:
      +2.1 MB vs R0 baseline (3.0 MB → 5.2 MB on macOS arm64). The
      "+500 KB" target assumed `native-tls`, which ureq 2.12 doesn't
      pick up at runtime (no TLS backend configured); default
      `rustls + webpki-roots` wins on portability but costs ~+2 MB.
      Accepted for v0.8; v0.9 risk item to evaluate `attohttpc` /
      `minreq` for size recovery.
    - 1–2 commits (feat(doctor), test(doctor)).

R7  Docs + version bump.
    - README + README.ko:
        - manage section: add `M` row.
        - doctor section: mention update line + SSHC_NO_UPDATE_CHECK.
        - Windows agent line in doctor table.
    - CHANGELOG [0.8.0] entry with three Added subsections:
        Windows agent pipe detection.
        manage-mode M (external → sshc.conf promote).
        doctor update check (+ SSHC_NO_UPDATE_CHECK escape hatch).
    - Cargo.toml: 0.7.3 → 0.8.0.
    - cargo install --locked --path . --force local refresh.
    - 1 commit (docs + chore).

R8  Release.
    - tag v0.8.0 annotated + push master + push v0.8.0.
    - Watch gh run list: confirm exactly ONE Release workflow run
      (concurrency guard from R0 doing its job).
    - cargo-dist artifacts up (9 files incl. sshc.rb), Homebrew tap
      committed.
    - Manual smoke on Windows: doctor 7-row output incl. update,
      manage-mode M promote, save flows.
    - 1 commit (chore(release)) + tag.
```

Per-round verification gate (mandatory before commit):
```bash
# R0–R5: full matrix.
cargo fmt --check \
  && cargo clippy --all-targets -- -D warnings \
  && cargo clippy --all-targets --target x86_64-pc-windows-msvc -- -D warnings \
  && cargo test --release \
  && cargo check --target x86_64-pc-windows-msvc

# R6+ (after ureq + rustls): Windows cross-compile from macOS pulls in
# ring's build.rs which needs MSVC headers we don't have locally.
# Drop the cross-check from the local gate and lean on cargo-dist's
# real Windows runner (verified at R8 tag push). Local matrix shrinks
# to:
cargo fmt --check \
  && cargo clippy --all-targets -- -D warnings \
  && cargo test --release
```
Plus R-G1..R-G9 grep matrix (`docs/TESTING.md §2`).

## 3. Step-by-step protocol carried over from v0.6 / v0.7

- Never `git rm` a range of lines from `src/app/mod.rs` blindly. If
  R3/R4 grow `app/forms.rs` beyond ~300 LOC, extract `promote_*`
  helpers into a new sibling module rather than letting the file
  balloon.
- When promoting a private method to `pub(super)` for the new
  promote path, do that promotion in its own commit (carries over
  from v0.4.3 retrospective).
- `wc -l src/app/*.rs` should stay flat or shrink — promote logic
  is small enough to fit in `forms.rs`.
- Windows is still a *first-class* build target. R0–R5 commits must
  pass `cargo check --target x86_64-pc-windows-msvc` locally; R6+
  commits get the same guarantee from cargo-dist's actual Windows
  runner instead (ring's build.rs blocks cross-compile from macOS
  without an MSVC toolchain). If a change touches anything
  platform-specific (process spawn, file paths, permissions, env
  vars, named pipes), still add the relevant `#[cfg]` *in the same
  commit*, never as a follow-up — the cargo-dist run will catch any
  Windows compile break before R8 tag push.
- R6 introduces the first runtime network dependency in sshc.
  Treat the call site like any external IO: short timeouts, no
  unwrap, no `?` past the boundary — failures map to a `WARN`
  status line, never to a doctor abort.

## 4. Architect-direct vs delegation

| Work | Owner | Notes |
|---|---|---|
| `src/app/` + `src/doctor.rs` edits | architect (you) | State machine wiring + new network surface. Too sensitive for delegation. |
| Windows named-pipe helper (R1) | architect | Tight `windows-sys` FFI. Cheap to write directly; specifying it for delegation is the same effort. |
| Version-compare fn + tag_name extraction (R6) | could delegate | Pure logic, 5 unit-test branches. Candidate for `tunaLlama:delegate-to-ollama` *if* the spec includes all 5 branches verbatim. **Decision**: architect-direct unless time-boxed; spec drift risk is non-trivial because GitHub API formats vary. |
| README/CHANGELOG diff | architect | Small, depends on final keybind names + doctor wording. |
| Post-R5 / post-R6 review pass | optional ollama reviewer | One sanity pass after the G2 surface settles, another after ureq lands. Strip ANSI. |

## 5. Definition of Done

See `BRIEF_V8.md §7`. Mechanical part requires additionally:

- [ ] R0–R8 commits landed on master.
- [ ] Each commit independently builds + tests green (no "wip" commits).
- [ ] No `src/app/*.rs` exceeds ~300 non-comment lines after R4.
- [ ] `src/doctor.rs` is the only place ureq is imported (network
      surface stays contained).
- [ ] `release.yml` produces exactly one workflow run for `v0.8.0`.
- [ ] Manual Windows matrix run by user before R8 tag push.

## 6. Risks (carried from BRIEF §6 + plan-specific)

| Risk | Mitigation |
|---|---|
| `M` collides with future single-letter command we haven't named yet | Reserve capital-letter space *now* as "irreversible-ish actions" (Promote, future Migrate, etc.) and document the convention in `README` §keybindings. |
| Windows named-pipe detection PASS-then-stale: ssh-agent pipe still listed during a service stop transition | Doctor is a point-in-time check by design. Detail string mentions "as of doctor run" only if user complaints surface — otherwise leave it as the same convention as the other checks. |
| `ureq` + `native-tls` pulls OpenSSL on some Linux distros and breaks `cargo install --locked` | R0 baseline build on macOS + a Linux VM (or docker `rust:1`) before R6 is merged. If native-tls fails, switch to `rustls-tls-native-roots` feature instead — keeps system trust store, drops OpenSSL. |
| Update check `WARN` becomes user noise once sshc stabilizes (rare new releases) | `SSHC_NO_UPDATE_CHECK=1` is documented prominently. If complaints surface, consider flipping default to opt-in via a `sshc config` flag in v0.9. |
| R0 concurrency guard accidentally serializes legitimate parallel runs (e.g. PR build + tag push) | `cancel-in-progress: false` means new runs queue, they don't cancel. Group keyed on `github.ref` so PR builds and tag pushes have different groups. |
| Form prefill in R4 misses a field that v0.7 added (IdentityFile picker target list, tags, extra options) | `prepare_promote` is tested with a fixture host that exercises every `config::model::Host` field; add a property-style "round-trip a fully-populated host" test. |

## 7. Commit message format

```
<type>(<scope>): <subject>

<body>

Refs: BRIEF_V8.md §<n>, PLAN_V0.8.md R<id>
Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
```

Scopes from v0.7 cycle: `app`, `ui`, `tui`, `inline`, `main`, `cli`,
`config`, `setup`, `storage`, `state`, `probe`, `exec`, `doctor`,
`chore(release)`, `docs`, `test`, `chore(ci)`. v0.8 adds `app/forms`
sub-scope for the promote work and reuses `doctor` for both G1 and
G3.

## End of Plan.
