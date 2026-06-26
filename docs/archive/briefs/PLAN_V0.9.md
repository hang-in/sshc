# sshc v0.9.0 — Execution Plan

> Companion to `BRIEF_V9.md`. Drives the operational-hardening +
> selective-UX-borrowing work. Architect-direct execution; one
> commit per logical step.

## 1. Roles

| Role | Who | Responsibilities |
|---|---|---|
| Architect | next Claude session (or current) | Reads `BRIEF_V9.md` + this plan. Applies one round per session block, commits, re-runs the verification gate. |
| User | d9ng | Approves DoD + final push. Provides Windows host for ARM64 verification (R8) and CRLF-config / nested-Include manual-matrix runs. |
| Optional reviewer | ollama via `tunaLlama:tuna-developer` | Candidate for R5 (Forwarding form layout) and R7 (size-recovery candidate comparison) where the spec is tight. |

## 2. Round breakdown

```
R0  Baseline.
    - git pull --ff-only (cargo-dist Homebrew tap may have advanced).
    - Confirm v0.8.4 state: cargo test green, clippy clean (host),
      R-G1..R-G9 clean, main.rs <= 10 LOC.
    - cargo build --release — record macOS arm64 binary size
      ('baseline_size_v084.txt' in scratchpad for R7 comparison).
    - No code changes. No commit unless re-running surfaces drift.

R1  G1: doctor CRLF check (BRIEF §3.1).
    - src/doctor.rs:
        + fn check_main_config_line_endings() -> Option<Check>
          (returns None when clean → check is omitted from output;
           returns Some(Warn) when CRLF is present in first 100 lines).
    - Insert into run()'s check list before the update check.
    - Unit tests:
        clean LF config → None
        CRLF config → Some(Warn) with the expected detail string
        empty config → None
    - README + README.ko: doctor table mentions the new line.
    - 1 commit (feat(doctor)).

R2  G2: doctor nested-Include check (BRIEF §3.2).
    - src/doctor.rs:
        + fn check_include_scope() -> Option<Check>
        + helper find_sshc_include_line(content, sshc_path) -> Option<usize>
        + helper preceding_host_or_match(content, lineno) ->
            Option<(usize, String)>
    - Logic:
        - If no sshc Include line: None.
        - If preceding stanza is `Match` or `Host *`: None
          (unconditional — sshc.conf works for every alias).
        - If preceding stanza is `Host <pattern>` other than `*`:
          Some(Warn) with the alias and line number in the detail.
    - Unit tests:
        config ending in `Host foo` + bare Include → WARN
        config with `Match all` before Include → None
        config with `Host *` before Include → None
        config with no Include → None
    - 1 commit (feat(doctor)).

R3  G3: StatusKind enum + sticky-on-error (BRIEF §3.3).
    - src/ui/status_bar.rs:
        + pub enum StatusKind { Info, Error }
        + struct StatusMessage gains `kind: StatusKind` field.
        + impl StatusMessage:
              pub fn new(text) → kind = Info (back-compat)
              pub fn error(text) → kind = Error
        + fn is_visible(&self, now: Instant) ->
              if kind == Error: true
              else: now - created_at < 3s
    - Call-site sweep (`app/forms.rs`, `app/input.rs`, runtime):
        - apply_form's Err branch  → StatusMessage::error
        - persist_sshc_conf errors → StatusMessage::error
        - permissions / setup errors → StatusMessage::error
        - everything else (pin / promote / 'i' added) → ::new (Info,
          transient) — unchanged.
    - On next keystroke (input.rs handle_key entry), clear any
      Error sticky message before routing the keystroke.
    - Unit tests:
        Info expires after 3s
        Error stays visible past 10s
        Error cleared by simulated keystroke
    - 1 commit (refactor(ui/status_bar), feat(app)).

R4  G4: arboard + `c` copy ssh command (BRIEF §3.4).
    - Cargo.toml: arboard = "3"
    - src/exec/ssh_config.rs (already owns `ssh -G` resolve):
        + pub fn ssh_command_for_alias(alias: &str) ->
              Result<String, ValidationError>
          → invokes ssh -G, extracts user/hostname/port/identityfile,
            returns "ssh USER@HOST -p PORT -i KEY".
    - src/app/input.rs: KeyCode::Char('c') in manage handler
      (block in read-only? no — copy is harmless without writes).
    - src/app/mod.rs::copy_ssh_command_for_selected():
        + call ssh_command_for_alias
        + arboard::Clipboard::new()?.set_text(cmd.clone())
        + status_message = Info "copied: <cmd shortened>"
        + fallback on clipboard failure: print to stdout? no — TUI
          context; just Error status "clipboard unavailable".
    - Unit tests:
        ssh_command_for_alias on fixture with all fields → correct
          string.
        defaults handled (no User → omit @, no Port → omit -p).
    - 1 commit (feat(exec/ssh_config), feat(app)).

R5  G5: Forwarding form section (BRIEF §3.5).
    - src/config/model.rs: Host struct already has `extra: Vec<String>`
      for free-form options. Decide:
        Option A — three new typed fields: local_forward,
          remote_forward, dynamic_forward (Vec<String>, since
          multiple are legal).
        Option B — keep parsing extra; recognise these three
          directives by prefix at form-render time.
      Pick A for cleanliness; serializer keeps extra for everything
      else.
    - src/ui/forms/host_form.rs:
        FIELD_COUNT grows from 7 → 10.
        labels become:
            "Alias", "HostName", "User", "Port", "IdentityFile",
            "Tags", "LocalForward", "RemoteForward",
            "DynamicForward", "Options"
        Section header rendered as dim non-focusable row at indices
          (5, 6) for "─── Forwarding ───" and at (8, 9) for "───
          Advanced ───". Tab skips them.
        Modal min height grows; if narrower than 18 rows, status
          line + filter line + section headers compete — implement
          scroll inside the form (ratatui ScrollbarState).
    - Validation regex per field (loose, matches OpenSSH).
    - sshc.conf serializer: emit per-field on save.
    - parser: also recognize per-field on read so round-trip works.
    - Integration test (tests/round_trip_test.rs):
        host with LocalForward "8080 localhost:80" + RemoteForward
          → write → read → field equality.
    - 1-2 commits (feat(config/model), feat(ui/forms), test).

R6  G6: `g` TCP reachability check (BRIEF §3.6).
    - src/exec/tcp_reach.rs (new file):
        + pub fn check_tcp_reach(host: &str, port: u16) ->
              ReachResult { Reachable { ms: u128 }, Unreachable {
              error: String } }
        + std::net::TcpStream::connect_timeout with 2-second budget.
        + Time the connect via Instant::now() bookend.
    - src/app/input.rs: KeyCode::Char('g') → invokes
      reach_check_for_selected on App.
    - src/app/mod.rs::reach_check_for_selected():
        + ssh -G resolve (cache hit if available, no re-spawn)
        + extract hostname + port
        + call check_tcp_reach
        + status: Info on success, Error sticky on failure.
    - Unit tests:
        connect_timeout to 127.0.0.1:1 (almost always closed) →
          Unreachable
        connect_timeout to listening localhost socket → Reachable
          with ms < 2000.
    - 1 commit (feat(exec/tcp_reach), feat(app)).

R7  G7: HTTP / TLS size recovery (BRIEF §3.7).
    - Measure baseline: target/release/sshc bytes vs R0 record.
    - Try the candidates in order, one branch each:
        (a) ureq + rustls-tls-native-roots (cargo bloat measured)
        (b) ureq + native-tls — using AgentBuilder to wire TlsConnector
            explicitly (the missing piece from v0.8 R6's "no TLS backend
            configured" stall)
        (c) minreq with https-rustls
        (d) attohttpc with tls-native
    - Pick the smallest that:
        - builds on macOS arm64, Linux x86_64, Windows MSVC x64
          (cargo-dist replays this for us at R9).
        - actually completes a GitHub /releases/latest call.
        - cargo bloat shows <= 4.8 MB target binary (-400 KB vs v0.8.4).
    - If no candidate clears the bar, keep ureq+rustls and note
      attempted candidates in BRIEF §6 risks update.
    - 1 commit (deps + doctor).

R8  G8: Windows ARM64 target (BRIEF §3.8).
    - dist-workspace.toml: add aarch64-pc-windows-msvc to targets.
    - dist generate to refresh .github/workflows/release.yml (then
      re-add the MANUAL concurrency block — confirmed v0.8 R0 +
      cargo-dist's allow-dirty = ['ci'] keeps it).
    - No code changes expected — Rust + windows-sys already support
      ARM64. If R7 picked native-tls and SChannel works only on x64
      target, fall back to per-target dependency split.
    - 1 commit (chore(ci)).

R9  Docs + release.
    - README + README.ko:
        - manage section: add `c copy ssh command`, `g TCP reach`.
        - form section: mention LocalForward/RemoteForward/
          DynamicForward fields.
        - doctor section: mention CRLF + Include scope checks.
    - CHANGELOG [0.9.0] with one paragraph per G1..G8.
    - Cargo.toml: 0.8.4 → 0.9.0.
    - cargo install --locked --path . --force local refresh.
    - 1 commit (docs + chore).
    - tag v0.9.0 + push master + push v0.9.0.
    - Watch gh run list: confirm exactly ONE Release workflow run.
    - cargo-dist artifacts up (>=17 files; ARM64 Windows adds 2
      more if G8 lands). Homebrew tap committed.
    - Manual smoke matrix (macOS):
        sshc -m → c (clipboard) → g (TCP) → form with LocalForward
        sshc --doctor → 9-row output incl. update + line-endings
          (if applicable).
    - User runs Windows + Linux verification before closing.
```

Per-round verification gate (mandatory before commit):
```bash
cargo fmt --check \
  && cargo clippy --all-targets -- -D warnings \
  && cargo test --release
```
Plus R-G1..R-G9 grep matrix (`docs/TESTING.md §2`).

Cross-platform note carried over from v0.8 PLAN §2: local
`cargo check --target x86_64-pc-windows-msvc` is broken from macOS
host since ring's build.rs requires MSVC headers. cargo-dist's
Windows runner verifies the cross-build at R9 tag push. If R7
swaps to native-tls or another non-ring crypto backend, re-test
the local cross-compile — it may come back into reach.

## 3. Step-by-step protocol carried over from v0.8

- Never `git rm` a range of lines from `src/app/mod.rs` blindly. R5
  grows `src/ui/forms/host_form.rs` significantly; if it crosses
  ~600 LOC, split section rendering into a sibling module
  (`src/ui/forms/host_form_render.rs`).
- v0.7-era pattern still applies: when a change touches anything
  platform-specific (process spawn, file paths, env vars, named
  pipes), add the relevant `#[cfg]` in the same commit. R6 TCP
  connect is cross-platform via `std::net`, no cfg needed.
- Status of the network surface: R4 (arboard), R6 (TcpStream), R7
  (HTTP client) all touch network/IO boundaries. Confine to
  `src/exec/*.rs` and `src/doctor.rs`. UI / form layers stay clean.
- v0.8 R0's R-G8 regression(`std::fs` in ui/forms) is a cautionary
  tale: if R5 needs filesystem access for forwarding-port checks,
  do the IO in `app/forms.rs` and pass results into the widget.

## 4. Architect-direct vs delegation

| Work | Owner | Notes |
|---|---|---|
| `src/doctor.rs` edits (R1, R2) | architect | Tight `check_*` pattern, easy. |
| `src/ui/status_bar.rs` enum + sticky logic (R3) | architect | Cross-cuts; do it directly. |
| Clipboard wiring (R4) | architect | Small, dep + 50 LOC. |
| Forwarding form layout (R5) | maybe delegate | Spec is tight: three fields + section headers + Tab skip. Candidate for `tunaLlama:tuna-developer` if spec is written verbatim. **Decision**: try delegation here; this is the largest deterministic surface and the right place to A/B the tunaLlama experiment we punted in v0.8. |
| TCP reach impl (R6) | architect | Tiny. |
| Size-recovery candidate comparison (R7) | maybe delegate | 4 candidates to swap in/out + measure. Mechanical and spec-driven. Candidate for delegation; the "pick the smallest that meets DoD" decision stays with the architect. |
| README/CHANGELOG diff (R9) | architect | Small, depends on final keybind names + doctor wording. |

## 5. Definition of Done

See `BRIEF_V9.md §7`. Mechanical part requires additionally:

- [ ] R0–R9 commits landed on master.
- [ ] Each commit independently builds + tests green (no "wip"
      commits).
- [ ] No `src/app/*.rs` exceeds ~350 non-comment lines after R4–R6.
- [ ] No `src/ui/forms/*.rs` exceeds ~600 LOC after R5 (extract
      sibling module if pressed).
- [ ] `src/doctor.rs` is the only place ureq / arboard `_other`
      crates are imported (network/clipboard surface stays
      contained).
- [ ] `release.yml` produces exactly one workflow run for `v0.9.0`.
- [ ] Manual cross-platform matrix run by user before R9 tag push
      (macOS architect-side; Linux + Windows user-side).

## 6. Risks (carried from BRIEF §6 + plan-specific)

| Risk | Mitigation |
|---|---|
| R5 grows form widget past usable height in 24-row terminals | Implement scroll mid-round if measurement shows the form needs >18 visible rows; alternative is to fold Advanced section behind a hotkey ('Tab Tab' to advanced page). |
| R7 size recovery requires re-deriving cargo bloat profile from scratch | Use cargo-bloat 0.12+ pinned at top of round. Record measurements in scratchpad, not committed code. |
| R8 cargo-dist generate overwrites our concurrency block | `allow-dirty = ['ci']` was added v0.8 R8 — verify still in dist-workspace.toml at R0 baseline. |
| `tunaLlama:tuna-developer` delegation in R5/R7 produces drift that requires re-do | Time-box delegation to one attempt per round; on drift, architect-direct takes over and the delegation note goes into commit body for retrospective. |
| Clipboard fail on Wayland silently confuses user | R4 surfaces "clipboard unavailable" as `Error` sticky (G3 helps here) — user sees the failure mode. |
| Sticky Error message gets in the way of subsequent legitimate Info messages | R3 clears Error on next keystroke regardless of what that keystroke does. Subsequent commands' Info messages render normally. |

## 7. Commit message format

```
<type>(<scope>): <subject>

<body>

Refs: BRIEF_V9.md §<n>, PLAN_V0.9.md R<id>
Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
```

Scopes from v0.8 cycle: `app`, `ui`, `tui`, `inline`, `main`, `cli`,
`config`, `setup`, `storage`, `state`, `probe`, `exec`, `doctor`,
`chore(release)`, `docs`, `test`, `chore(ci)`. v0.9 reuses
`doctor` (R1, R2), `ui/status_bar` (R3), `exec/ssh_config` (R4),
`ui/forms` + `config/model` (R5), `exec/tcp_reach` (R6 — new file),
`deps` (R7), `chore(ci)` (R8).

## End of Plan.
