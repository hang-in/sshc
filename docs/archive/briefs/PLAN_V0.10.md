# sshc v0.10.0 — Execution Plan

> Companion to `BRIEF_V10.md`. Surface-polish + size second pass.
> Architect-direct execution; one commit per logical step.

## 1. Roles

| Role | Who | Responsibilities |
|---|---|---|
| Architect | current Claude session | Reads `BRIEF_V10.md` + this plan. Applies one round at a time, commits, re-runs the verification gate. |
| User | d9ng | Approves DoD + final push. Provides matrix verification on macOS at R7 (OSC 52 clipboard sanity + ProxyCommand WARN). |

## 2. Round breakdown

```
R0  Baseline (DONE this session).
    - v0.9.0 on master (commit dee6650), tag pushed, cargo-dist
      success, ARM64 artifact verified.
    - macOS arm64 release: 3,762,576 bytes (3.76 MB).
    - R-G1..R-G9 clean. fmt + clippy host + windows-cross clean.
    - cargo test --release: 206 lib + 3 integration green.

R1  G1 storage model: Option<String> → Vec<String> (BRIEF §3.1.1).
    - src/config/model.rs:
        Host::local_forward / remote_forward / dynamic_forward
        switch to `Vec<String>`. Empty Vec ↔ unset.
    - src/config/parser.rs:
        BlockState fields likewise. The `replace(value)` last-wins
        branch becomes `push(value)`. Drop the cascade-into-extra
        path that v0.9 G5 used as a workaround.
    - src/storage/serializer.rs:
        Iterate each Vec, emit one `LocalForward …` line per entry
        (or omit the section entirely when empty).
    - Fixture sweep: all 8 Host literal constructors take `Vec::new()`
      for the three new fields (was three `None`). One perl one-liner
      handles macOS / Linux / tests / examples in one pass.
    - test_round_trip_forwarding_through_parser in
      src/storage/serializer.rs gains a multi-entry case.
    - 1 commit (feat(config/model)).

R2  G1 ForwardingListModal (BRIEF §3.1.2-§3.1.4).
    - New file `src/ui/forms/forwarding_list.rs` with:
        pub struct ForwardingListModal {
            title: &'static str,
            entries: Vec<String>,
            selected: usize,
            editing: Option<String>,
        }
        impl FormState for ForwardingListModal { … }
    - Keymap:
        ↑/↓     selection
        Enter   start editing the selected row (or add a new row
                when selected == entries.len())
        d       delete the selected row (no confirm — Esc undoes
                editing-mode entry; for delete the only undo is to
                retype)
        Esc     if editing → cancel edit; else → submit list back
                to parent form
    - ui::modal::ModalKind gains a new variant or — simpler —
      we route via FormState polymorphism (already in place for
      HostForm + TagForm).
    - HostForm: when active_index lands on a forwarding field and
      the user presses Enter, instead of advancing field, open the
      list modal. The summary cell displays:
          0 entries     → []
          1 entry       → [<the entry>]
          n ≥ 2 entries → [<first> +n more]
    - FormPayload::Host: three forwarding fields go from String
      → Vec<String>. apply_form match arms + build_host signature
      thread Vec through.
    - Modal-from-form mechanics: the simplest path is `App::mode`
      ← Modal(ForwardingListModal). The parent HostForm sits in
      memory; when the list modal cancels/submits, we put HostForm
      back in App::mode with its `fields[i]` summary refreshed.
    - 1–2 commits (feat(ui/forms/forwarding_list), feat(app)).

R3  G4 doctor ProxyCommand (BRIEF §3.4).
    - src/doctor.rs:
        + fn check_proxy_commands() -> Option<Check>
        + helper iter_hosts_in_config_chain() — calls
          config::parser::parse_config on ~/.ssh/config (Include
          chain already followed by the parser).
        + helper extract_first_token(line: &str) — whitespace split
          + dequote on outermost `"…"` only.
        + helper find_on_path(token: &str) — Unix: walk PATH,
          require regular file + executable bit. Windows: PATH +
          PATHEXT.
    - Output:
        [WARN] proxy commands  '<token>' not on PATH (used by N host(s))
    - When clean: None — line omitted (matches CRLF check pattern).
    - Unit tests: 3 fixture cases (clean / missing / quoted token).
    - 1 commit (feat(doctor)).

R4  G5 `S` sort key (BRIEF §3.5).
    - src/app/mod.rs:
        + enum SortAxis { AliasAlpha, RecentDesc, ProbeStateOpenFirst }
        + App.sort_axis: SortAxis  (default AliasAlpha)
    - src/app/filter.rs (where apply_filter lives):
        At the end of apply_filter, before computing `selected`,
        sort `filtered` by (favorite_first, sort_axis_key).
        favorite_first preserves v0.6 R2's pin-to-top guarantee.
    - src/app/input.rs:
        KeyCode::Char('S') (Shift-modified — match on Char('S'),
        not Char('s') with KeyModifiers::SHIFT, because crossterm
        already gives us the cased char) cycles the axis and
        emits an Info status "sorted by alias / recent / open-first".
    - 3 unit tests on apply_filter sort behaviour.
    - 1 commit (feat(app)).

R5  G3 OSC 52 fallback (BRIEF §3.3).
    - Cargo.toml: base64 = "0.22"
    - New file `src/exec/clipboard.rs`:
        pub enum ClipboardBackend { System, Osc52 }
        pub fn copy_to_clipboard(text: &str)
            -> Result<ClipboardBackend, ClipboardError>
        Tries arboard first; on failure emits OSC 52 to stdout
        and returns Ok(Osc52).
    - src/app/mod.rs::copy_ssh_command_for_selected: route
      through clipboard::copy_to_clipboard, surface
      `copied (osc52)` suffix only when backend != System.
    - Unit test: encode "hello" → assert OSC 52 bytes start with
      `\x1b]52;c;` and end with `\x1b\\`.
    - SSHC_NO_OSC52 escape hatch (env var) — when set, skip the
      OSC 52 fallback. The original Error sticky path resumes.
    - 1 commit (feat(exec/clipboard)).

R6  G2 clipboard backend swap evaluation (BRIEF §3.2).
    - Record baseline: target/release/sshc bytes.
    - Try in order, each on its own branch (don't push):
        (a) `arboard = { default-features = false, features = [...] }`
            — turn off image decoders only.
        (b) `clipboard-anywhere` straight swap.
        (c) `copypasta` straight swap.
        (d) Hand-rolled platform-cfg (macOS objc2, Win32, Linux
            X11+Wayland).
    - Pick the smallest that:
        builds on macOS arm64, Linux x86_64, Windows MSVC x64
        (cargo-dist re-verifies at R7).
        keeps R-G6 (storage clean) and R-G8 (ui/forms clean).
        actually copies on a manual `c` test.
    - If no candidate clears -300 KB, keep current arboard and
      note attempted candidates in BRIEF §6 risks update.
    - 1 commit (deps + maybe new file if hand-rolled).

R7  Docs + release.
    - README + README.ko:
        - manage section: add `S` sort row.
        - form section: mention forwarding list modal (Enter to edit).
        - doctor section: mention ProxyCommand check.
        - clipboard section: mention OSC 52 fallback + tmux note.
    - CHANGELOG [0.10.0] one paragraph per G1..G5.
    - Cargo.toml: 0.9.0 → 0.10.0.
    - cargo install --locked --path . --force local refresh.
    - 1 commit (docs + chore).
    - tag v0.10.0 + push master + push v0.10.0.
    - Watch gh run list: confirm exactly ONE Release workflow run
      and that ARM64 + x64 Windows artifacts both land.
    - Manual smoke matrix (macOS):
        sshc -m → S sort cycle visible
                → c copy (system path)
                → ENV SSHC_FORCE_OSC52=1 sshc -m → c copy (osc52)
                → forwarding list modal: add 2 entries, save,
                  reopen, count check
        sshc --doctor → 7+ row output incl. ProxyCommand WARN if
                       a test host with bad proxy is wired in
        cargo install --git ... --tag v0.10.0 fresh install on
                       a Linux VM (or container) — make sure
                       chosen clipboard backend builds.
```

Per-round verification gate (mandatory before commit):
```bash
cargo fmt --check \
  && cargo clippy --all-targets -- -D warnings \
  && cargo clippy --all-targets --target x86_64-pc-windows-msvc -- -D warnings \
  && cargo test --release
```
Plus R-G1..R-G9 grep matrix (`docs/TESTING.md §2`).

## 3. Step-by-step protocol carried over from v0.9

- Same form-widget hygiene as v0.9 R5: `host_form.rs` is now ~610
  LOC after G5. If R2's modal wiring drags it past 700, extract
  the section-header rendering into a sibling module.
- Network surface stays in `src/exec/*.rs` + `src/doctor.rs`. UI /
  form layers don't touch it. R5 (clipboard) lives in
  `src/exec/clipboard.rs` for the same reason.
- v0.9 R7 unblocked Windows MSVC cross-check from macOS. Keep it
  in the per-round gate — if R6 picks a clipboard backend that
  pulls ring back in, the gate will yell and we revert.

## 4. Architect-direct vs delegation

| Work | Owner | Notes |
|---|---|---|
| Storage model migration (R1) | architect | Trivial after v0.8 R0 / v0.9 R5 fixture sweep pattern — perl one-liner does most. |
| ForwardingListModal (R2) | architect | UX-critical and small (~150 LOC). |
| Clipboard backend evaluation (R6) | architect (could delegate measurement) | Pure mechanical: swap, build, measure. `tunaLlama:tuna-developer` candidate if v0.9 retros showed delegation value. |
| README/CHANGELOG diff (R7) | architect | Small, depends on final keybind names. |

## 5. Definition of Done

See `BRIEF_V10.md §7`. Mechanical part requires additionally:

- [ ] R1–R7 commits landed on master.
- [ ] Each commit independently builds + tests green (no "wip"
      commits).
- [ ] `release.yml` produces exactly one workflow run for `v0.10.0`.
- [ ] cargo-dist artifact count stays at 19 (or grows — never
      shrinks, modulo the chosen clipboard backend dropping a
      transitive binary like LICENSE files).
- [ ] Manual matrix run by user before R7 tag push.

## 6. Risks (carried from BRIEF §6 + plan-specific)

| Risk | Mitigation |
|---|---|
| R2 modal-in-modal mechanics confuse the v0.6-era ModalKind enum | If the existing FormState polymorphism doesn't accommodate a list-of-strings modal cleanly, add a sibling `ListModal` variant to ModalKind instead of forcing it through Form. |
| R5 OSC 52 collides with crossterm's raw-mode output expectations | Emit the escape sequence on a single `print!` + flush, between event polls. The terminal absorbs it; the next ratatui draw repaints anyway. |
| R6 hand-rolled clipboard touches platform headers we don't have | Time-box hand-rolled to *one* commit attempt; on first cross-build failure, fall back to a wrapper crate. |
| G5 sort surfaces ProbeState ordering that confuses users (Open vs InFlight) | The axis name in the status message resolves it: "sorted by reachability" — users see the label, not the internal enum. |
| Multi-step v0.9 → v0.10 forwarding read by an older sshc loses data | CHANGELOG migration note: "users with multi-line LocalForward saved by v0.10 should not roll back to v0.9; v0.9 would silently keep only the last entry". |

## 7. Commit message format

```
<type>(<scope>): <subject>

<body>

Refs: BRIEF_V10.md §<n>, PLAN_V0.10.md R<id>
Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
```

Scopes from v0.9 cycle (carried): `app`, `ui`, `tui`, `inline`,
`main`, `cli`, `config`, `setup`, `storage`, `state`, `probe`,
`exec`, `doctor`, `chore(release)`, `docs`, `test`, `chore(ci)`,
`deps`. v0.10 adds `ui/forms/forwarding_list` (R2) and reuses
`exec/clipboard` (R5).

## End of Plan.
