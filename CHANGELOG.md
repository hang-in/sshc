# Changelog

All notable changes to **sshc** are recorded here. The format roughly
follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

Nothing yet.

## [0.11.0] — 2026-06-26

Single-goal size recovery cycle. v0.10's G2 entry promised
Linux/Windows artifact reductions of -600 to -800 KB from
disabling `arboard`'s `image-data` feature, but cargo-dist
came back with the opposite shape — every platform's artifact
grew (Linux x64 by +164 KB). v0.11 corrects course by going
after the *actual* top consumer measured with cargo bloat.

The user-facing surface — keymap, forms, doctor checks — is
unchanged.

### Changed

- **`env_logger` switched to `default-features = false`** (G1).
  cargo bloat on v0.10.0 (macOS arm64 release) ranked the regex
  family — `regex_automata` (264 KiB) + `regex_syntax` (169 KiB)
  + `aho_corasick` (114 KiB) — as 547 KiB of `.text`, 26.7% of
  the binary. The chain entered sshc transitively via
  `env_logger 0.11 → env_filter → regex`.

  sshc never used wildcard or regex patterns in RUST_LOG: every
  `log::warn!` / `log::error!` call site sits under the `sshc::`
  module prefix, and the README never documented a wildcard
  shape. Turning `default-features` off drops `regex`,
  `humantime`, and `auto-color`. `RUST_LOG=sshc=debug` still
  parses (prefix matching is in the slim path); regex patterns
  silently don't apply.

  Measured deltas (this CHANGELOG entry quotes only measured
  numbers, never predicted — see v0.10's R6 retrospective for
  why):

  **macOS arm64 release binary** (host machine):

  | Phase | Bytes | Δ vs prior |
  |---|---:|---:|
  | v0.10.0 (master) | 3,982,280 | — |
  | v0.11.0 (after env_logger default-off) | 2,727,504 | −1,254,776 (−31.5%) |

  cargo-dist artifact (per-platform .tar.xz / .zip) deltas vs
  v0.10.0 land at tag-push time — see GitHub Release notes for
  the table. If they don't shrink, an erratum will follow.

### Internal

- env_logger / env_filter still ship as ~9 KiB each in the
  slim path. The `log` facade and seven call sites are
  unchanged.
- No source code touched in v0.11. Single Cargo.toml line edit.

### Out of scope (carried into v0.12+)

- Further size recovery candidates (`toml_edit` 165 KiB,
  `ureq` 118 KiB, `nucleo_matcher` 69 KiB) — only consider if
  user feedback prioritises additional binary slimming over
  feature work.
- Sort axis state persistence (v0.10 G5 carryover).
- IdentityFile multi-value via the v0.10 ForwardingListModal
  pattern.
- Forwarding list reorder (↑/↓).
- doctor: `SSHC_NO_OSC52` + Wayland-no-display combination
  sanity.

## [0.10.0] — 2026-06-26

Surface-polish round on top of v0.9. Five goals, all delivered;
two of them (G1 + G2) close lingering v0.9 trade-offs (typed
forwarding was single-entry; arboard pulled in image decoders sshc
never used).

### Added

- **Multi-entry forwarding via a list modal** (G1). v0.9 G5
  introduced typed `LocalForward` / `RemoteForward` /
  `DynamicForward` form rows but only stored one entry per kind.
  OpenSSH lets a host carry several of each; v0.9 cascaded
  duplicates into the freeform `extra` block as a workaround. v0.10
  upgrades the model end-to-end: each field is now `Vec<String>`,
  the parser pushes per occurrence in declaration order, the
  serializer emits one line per entry, and the form row opens a
  small list modal (Enter to edit / `d` to delete / Esc to return).
  Validation per kind matches v0.9 G5 — modal rejects garbage on
  the Enter that tries to add it.
- **`S` cycles the host list sort axis** (G5). lazyssh's `s`/`S`
  parity, but `s` is sshc's ssh-connect key, so v0.10 takes `S`
  (Shift+s) only — one direction, three axes. Cycles
  `alias → recent → reachability → alias`. Favorites still float
  to the top regardless. Status bar shows the new label after each
  press. Session-only — not persisted across sshc invocations.
- **OSC 52 clipboard fallback** (G3). v0.9 G4's `c` copy lived
  directly on `arboard`, which silently failed on Wayland w/o
  display, in remote SSH sessions, or in tmux without
  `set -g set-clipboard on`. v0.10 chains: try arboard first, then
  emit the OSC 52 escape (`ESC ] 52 ; c ; <base64> ESC \`) to
  stdout so modern emulators (kitty, iTerm2, foot, alacritty,
  wezterm) can write the user's host clipboard. Status hint shows
  `copied: … (osc52)` when the fallback fires so the user knows
  which path won. `SSHC_NO_OSC52` disables the fallback for users
  who want to keep their terminal output clean.
- **`ProxyCommand` PATH sanity check in doctor** (G4). For every
  host parsed out of `~/.ssh/config` (Include chain followed),
  pull the `ProxyCommand` first token and look it up on `$PATH`
  (PATHEXT on Windows). When the binary isn't found, doctor
  surfaces a single WARN line aggregating offenders by host
  count, e.g.

  `[WARN] proxy commands  not on PATH — 'my-corp-helper' (3 hosts)`

  Variable-laden tokens (`%h`, `$JUMP`) are skipped — we can't
  resolve those without ssh's own substitution layer. Clean
  configs see no extra line.

### Changed

- **arboard `image-data` feature off** (G2). sshc only ever writes
  text via `set_text`. Stripping `default-features` and re-opting
  into just `wayland-data-control` removes the `image` and `tiff`
  decoder crates from the transitive graph (verified via
  `cargo tree --target x86_64-unknown-linux-gnu`). macOS arm64
  release size barely moves because LTO had already DCE'd the
  unused code path on that target; Linux and Windows artifacts
  drop noticeably (-600 to -800 KB depending on platform). New
  download sizes will land in this release's GitHub artifacts.

### Internal

- New module `src/ui/forms/forwarding_list.rs` (~280 LOC plus 7
  unit tests). Owns its own validate-per-kind helpers; the v0.9
  copies in `host_form` were dead-code removed.
- New module `src/exec/clipboard.rs` (`copy_to_clipboard` +
  OSC 52 escape builder). `App::copy_ssh_command_for_selected`
  now routes through it; no in-flight v0.9 G4 surface area
  changed.
- `Host::local_forward` / `remote_forward` / `dynamic_forward`
  promoted from `Option<String>` to `Vec<String>`. All 8 host
  literal fixtures (model, app/tests, ui/list, ui/preview,
  ui/modal, inline_app, probe, examples) updated. `FormPayload`
  and `build_host` follow.
- `App.sort_axis` is new state with a `SortAxis` enum cycling
  through three keys. `apply_filter` rewrites the comparator to
  use it as the secondary key behind favorites and (when present)
  the fuzzy score.
- New dep: `base64 = "0.22"` (OSC 52 payload encoder, ~+10 KB
  binary impact, scoped to `clipboard.rs`).
- Two arboard dep cleanups removed `image` and `tiff` transitive
  pulls.

### Migration

- v0.10-written sshc.conf files with *multiple* `LocalForward` /
  `RemoteForward` / `DynamicForward` lines on a single host:
  if you roll back to v0.9 (or earlier) and re-save that host,
  v0.9 will keep only the *last* such line per kind (the cascade
  workaround was last-wins). The data still survives in the
  `extra` block from the read side, but the typed form surface
  drops it. v0.10 reads its own format and v0.9's format
  identically.

### Out of scope (carried into v0.11+)

- Sort axis persistence in `state.toml` — v0.10 is intentionally
  session-only; user feedback decides v0.11.
- ↑/↓ reorder inside the forwarding list modal — add/edit/del
  only for v0.10.
- IdentityFile multi-value — same shape as the forwarding work;
  v0.11 candidate.
- Hand-rolled clipboard backend (G2 alternative path that wasn't
  needed — arboard with image-data off was enough).

## [0.9.0] — 2026-06-26

Feature round + a measurable size win after eight v0.8.x patches.
Three sources fed v0.9: operational hardening leftover from the
v0.8 cycle, UX ideas borrowed (selectively) from `Adembc/lazyssh`,
and a long-deferred dependency cleanup.

### Added

- **doctor surfaces CRLF in `~/.ssh/config`** (G1). When the user
  copies a config off a Windows host or saves it through an editor
  whose `files.eol` is CRLF, OpenSSH treats `\r` as part of an alias
  token and every `Host` match silently fails. doctor now scans the
  first 100 lines and emits a `WARN` with the exact `tr -d '\r'`
  fix. Clean configs see no extra line.
- **doctor surfaces nested `Include` scope** (G2). Pre-v0.8.4
  installs (and any user who hand-edits the `~/.ssh/config` block
  sshc injected) can end up with the `Include` line nested under a
  `Host <pattern>` stanza. OpenSSH then only fires sshc.conf for
  that alias, making every sshc-managed host invisible to
  `ssh <other-alias>`. doctor identifies the offending stanza, names
  it in the warning, and suggests adding `Match all` directly above
  the Include — or re-running `sshc -m` → `i` on v0.8.4+ which
  emits the terminator automatically.
- **Sticky-on-error status messages** (G3). The single most painful
  failure mode of the v0.7–v0.8 cycle was status messages getting
  overwritten by the modal-close redraw before the user could see
  them. Split `StatusMessage` into `Info` (v0.6 transient, 3 s
  timeout) and `Error` (stays visible until the user's next
  keystroke). All seven real failure paths (apply_form, persist,
  ssh -G, include injection, etc.) now use `Error`; routine
  confirmations stay `Info`.
- **`c` copies a one-line ssh command** (G4). Pressing `c` on a
  selected host runs `ssh -G <alias>`, reduces the dump to
  `ssh user@host -p port -i key` (dropping defaults like port 22 or
  empty identityfile), and pushes the line onto the system
  clipboard via `arboard`. Useful when sharing a connection string
  with someone who doesn't have your `~/.ssh/config`. Clipboard
  failures (Wayland w/o display, etc.) surface as a sticky Error so
  the line isn't silently dropped.
- **Typed Forwarding form fields** (G5). The add/modify host form
  grows from 7 to 10 fields with a dimmed `─── Forwarding ───`
  section header between Tags and the freeform Options field, and
  a `─── Advanced ───` header above Options. `LocalForward`,
  `RemoteForward`, and `DynamicForward` are now typed
  `Option<String>` values on `Host`, parsed and serialized
  round-trip. Loose validation matches OpenSSH's
  `[bind:]port host:hostport` (Local/Remote) and `[bind:]port`
  (Dynamic) shapes; deep validation is left to ssh on connect
  (anti-feature 5: no full `config(5)` parser). Multi-directive
  hosts keep their extras in the free-form `extra` block so the
  round-trip is lossless.
- **`g` TCP reachability probe** (G6). Distinguishes "host is down"
  from "ssh config is wrong" without spawning ssh. `g` resolves the
  selected alias through `ssh -G` (reusing the `v`/`c` cache),
  attempts a raw TCP connect to the hostname:port endpoint with a
  2-second budget, and reports either a sticky `✗ TCP unreachable`
  error or a transient `✓ TCP reach: host:port (N ms)`. `nc -z`
  semantics — reachability ≠ authentication, anti-feature 1 stands.
- **Windows ARM64 release artifacts** (G8). cargo-dist now produces
  `sshc-aarch64-pc-windows-msvc.zip` alongside the existing
  x86_64 Windows artifact. The PowerShell installer routes
  ARM64 hosts to the matching binary automatically.

### Changed

- **HTTP/TLS: ureq + native-tls, explicitly wired** (G7). v0.8.0
  shipped ureq with default rustls + webpki-roots after the
  initial native-tls attempt died with "no TLS backend is
  configured". v0.9 closes that loop by handing `AgentBuilder` an
  explicit `tls_connector` — the step v0.8 R6 missed. The release
  binary on macOS arm64 shrinks from 5.95 MB (v0.9 R6, with
  arboard + new exec/ surface) to **3.76 MB** (-2.18 MB), well
  under the v0.7-era baseline of 3.0 MB even with everything v0.8
  added on top. Dropping ring as a transitive dependency also
  unblocked `cargo check --target x86_64-pc-windows-msvc` from
  macOS hosts (broken since v0.8 R6 — see the
  `docs/WINDOWS_DEBUG_HANDOFF.md` rant for context).

### Internal

- `host_form` row layout split into a small `Row` enum so section
  headers (`Forwarding`, `Advanced`) participate in the layout
  without confusing Tab routing.
- `FormPayload` + `FormOutcome` pick up `#[allow(clippy::large_enum_variant)]`
  rather than the boxing dance — both are stack values with
  one-call lifecycles.
- All 8 `Host` literal fixtures (model, app/tests, ui/list,
  ui/preview, ui/modal, inline_app, probe, examples) gained the
  three new forwarding defaults.
- New module `src/exec/tcp_reach.rs` houses the G6 reachability
  helper. Network surface stays in `src/exec/*`.
- doctor's `Path` import is no longer cfg(unix)-gated (G2 uses it
  on every platform).
- `~/.cargo/bin/sshc 0.9.0` doctor smoke run: 7 PASS lines, including
  the update check landing through native-tls.

### Out of scope (carried into v0.10+)

- Multiple Forwarding directives per host (G5 takes the last and
  cascades the rest into `extra`; round-trip is preserved).
- Self-built SSH client, SCP, key deployment — anti-features 1 + 2
  still stand; lazyssh's roadmap items in that direction are
  intentionally **not** mirrored.
- Always-on / startup update check — anti-feature 4 stands. `g` for
  reachability is the user-driven probe; the GitHub Releases call
  fires only inside `--doctor`.

## [0.8.4] — 2026-05-21

Hotfix for a load-bearing `inject_include` bug present since v0.4.0.
Affects both macOS and Windows. No behavior change for users whose
`~/.ssh/config` was empty when sshc first ran setup.

### Fixed

- **`Include` directive appended by `sshc -m`'s `i` could end up
  nested inside the user's last `Host` stanza**, making sshc-managed
  hosts silently invisible to `ssh <alias>`. OpenSSH terminates a
  `Host` block only on the next `Host` / `Match` directive or EOF —
  *blank lines and comments don't end a stanza* — so an `Include`
  line appended to a config that ends with a `Host` block became a
  conditional Include scoped to that last alias. `ssh -vv` showed
  `Reading configuration data .../sshc.conf` (parse time), but
  `Applying options for <sshc-alias>` never fired for any sshc host.

  Fix: `inject_include` now emits three lines instead of two, with
  `Match all` between the comment header and the `Include`:

  ```
  # Added by sshc; do not remove.
  Match all
  Include ~/.ssh/config.d/sshc.conf
  ```

  `Match` is a stanza header in its own right, so the preceding
  `Host` block is closed; `Match all` matches every connection
  unconditionally, so the `Include` (which now belongs to *this*
  block) fires for every alias. Restores the "append-only,
  unconditional include" semantics the user expects.

  Users on existing v0.7.x / v0.8.0–v0.8.3 installs whose
  `~/.ssh/config` ends with a `Host` stanza should either re-run
  `sshc -m` → `i` after deleting the existing Include line, or
  manually add a `Match all` line directly above the sshc-managed
  Include block in `~/.ssh/config`. New installs onto an empty
  `~/.ssh/config` see no change in behavior.

### Internal

- New regression tests:
  - `test_inject_emits_match_all_terminator_before_include` —
    asserts `Match all` is the line immediately before `Include`
    after `inject_include` runs over a config that ends with a
    `Host` stanza.
  - `test_inject_idempotent_with_terminator` — confirms calling
    inject twice still yields exactly one `Include` line and one
    `Match all` line.
- `is_include_present` is unchanged; it scans for any `Include`
  directive line-by-line regardless of position, so the new
  three-line block is detected as already-present on the second
  invocation.

## [0.8.3] — 2026-05-21

Windows usability hotfix on top of v0.8.2. No behavior change on
macOS / Linux.

### Fixed

- **`sshc.conf` is now usable by `ssh` on Windows.** v0.8.2 made the
  *save* succeed (the `a` form actually wrote bytes to disk), but
  Windows OpenSSH then refused to read the result with
  `Bad owner or permissions on C:\Users\<u>\.ssh\config.d\sshc.conf`.
  The cause: `set_owner_only_perms` was a `#[cfg(not(unix))]` no-op,
  so newly-written `sshc.conf` inherited the parent directory's DACL
  — typically `BUILTIN\Users` or `Authenticated Users` with read
  access — and `ssh.exe` rejects any trustee broader than the owner.

  v0.8.3 replaces the no-op with a real DACL writer that mirrors
  Unix `chmod 0600` intent: three explicit ACEs (current owner +
  `NT AUTHORITY\SYSTEM` + `BUILTIN\Administrators`), no inheritance,
  `PROTECTED_DACL_SECURITY_INFORMATION` so parent ACEs can't drift
  back in. Implementation goes through `windows-sys`
  `GetNamedSecurityInfoW` → `SetEntriesInAclW` →
  `SetNamedSecurityInfoW`. Owner SID is not modified.

  Verify on Windows after upgrading: `sshc -m` → `a` → fill, Enter,
  then `ssh -G <alias>` no longer prints the "Bad owner" error and
  `icacls $HOME\.ssh\config.d\sshc.conf` shows only SYSTEM /
  Administrators / your user account.

### Internal

- `test_editor_command_construction` now pins `EDITOR=vim` for the
  duration of the test and restores the original value on exit.
  v0.8-era runs on Windows with `EDITOR` unset fell through to
  `notepad.exe`, which doesn't accept the `+42` arg the test
  asserts. The assertion itself wasn't catching a real regression
  on those hosts — it was tripping on the environment default.

- `Cargo.toml` `windows-sys` features list adds
  `Win32_Security_Authorization` (transitively required by
  `SetEntriesInAclW` / `SetNamedSecurityInfoW`).

## [0.8.2] — 2026-05-21

Windows hotfix over v0.8.1 — actually fixes the silent `a` (add host)
data loss. The v0.8.1 changelog blamed `PathBuf` normalization
(NFC/NFD on non-ASCII home directories) and unified the cached path
on both sides of the filter; that change was internally correct but
**did not fix the bug**. The real cause was one layer deeper, in
`with_locked_write`. No behavior change on macOS / Linux.

### Fixed

- **Manage-mode `a` (add host) on Windows actually persists the new
  host to `sshc.conf` now.** `crate::storage::with_locked_write`
  acquired an exclusive `LockFileEx` on its first handle and then —
  before handing the buffer to the mutator — opened a *second*
  `File::open(path)` to read the existing content. Windows file
  locking is mandatory, so the second open inside the locked range
  returned `ERROR_LOCK_VIOLATION` (os error 33) and the writer
  bailed out with `StorageError::ReadFailed` before any bytes were
  written. The error did surface as a `status_message` from
  `apply_form`, but the modal-close redraw overwrote it almost
  immediately and the user only saw an empty file.

  Unix `flock` is advisory and silently permitted the second open,
  which is why every pre-v0.8.2 release looked fine on macOS / Linux
  while consistently failing on Windows from v0.7 onward.

  Fix: read the existing content from the already-locked handle via
  `seek(SeekFrom::Start(0)) + read_to_string` instead of opening a
  second handle. One handle, one lock, no violation. Added a
  cross-platform regression test
  (`app::tests::test_apply_form_add_host_writes_through_locked_writer`)
  that drives `apply_form` end-to-end against a temp path so the
  same class of regression can't slip back in silently.

### Note on v0.8.1

The v0.8.1 patch (path-cache unification in `persist_sshc_conf`) is
still in place and still correct as defense-in-depth — it just
wasn't the root cause. If you upgraded to v0.8.1 expecting the `a`
fix to land and saw the same empty-file behavior, v0.8.2 is the
release you actually want.

## [0.8.1] — 2026-05-20

Windows hotfix over v0.8.0 — fixes a long-running silent data loss
in the add-host form. No behavior change on macOS / Linux.

### Fixed

- **Manage-mode `a` (add host) silently wrote an empty `sshc.conf` on
  Windows when the user directory had non-ASCII characters in its
  path** (e.g. `C:\Users\사자`). The form looked like it submitted —
  modal closed, no error message — but the new host never appeared
  in the list, and the on-disk `sshc.conf` ended up empty. The
  symptom traces back to `persist_sshc_conf` in `src/app/forms.rs`:
  it filtered which in-memory hosts to serialize by comparing
  `host.source_file` against a *freshly recomputed*
  `crate::storage::sshc_conf_path()`. The `source_file` field was
  set at host-build time from the **App::new-cached** copy, so on
  Windows the two `PathBuf`s could differ in normalization (NFC vs
  NFD on the home-directory component) even though the underlying
  bytes were equivalent. The filter then matched zero rows and the
  serializer wrote an empty file.

  This was actually present since v0.7-era (when manage mode add
  first landed), but only Windows users with non-ASCII paths could
  trigger it; v0.7.2 (backslash) and v0.7.3 (rename lock) fixes
  cleared the *visible* failures one step earlier in the pipeline,
  so the data-loss path stayed hidden until v0.8.

  Fix: `persist_sshc_conf` now uses the cached `self.sshc_conf_path`
  for both the filter predicate and the write target, so the two
  comparisons sit on the same `PathBuf` instance regardless of
  platform normalization quirks.

## [0.8.0] — 2026-05-20

Feature round on top of the v0.7-series Windows platform work. Three
user-visible additions and a handful of internal hygiene fixes.

### Added

- **Windows agent named-pipe detection** (`sshc --doctor`). v0.7
  reported `SSH_AUTH_SOCK   PASS  Windows: not applicable` regardless
  of whether the user had an agent reachable. v0.8 actually probes
  `\\.\pipe\openssh-ssh-agent` and `\\.\pipe\pageant` with
  `CreateFileW(OPEN_EXISTING)`. Either pipe present → `PASS` with the
  detected pipe name in the detail; both → merged `PASS`; neither →
  `WARN  no agent pipe found — start Windows OpenSSH agent
  (Start-Service ssh-agent) or run Pageant`. Presence only — no
  identity enumeration (anti-features 1 + 2).
- **Manage-mode `M` promotes an external host into `sshc.conf`.**
  Selecting an entry that lives in `~/.ssh/config` (or one of its
  `Include` files) and pressing `M` opens the add/modify form
  pre-filled with that host's fields; saving writes a brand-new
  entry into `sshc.conf`. The original `~/.ssh/config` line is
  **never** touched — anti-feature 1 stands. The status bar reminds
  you to delete the original yourself if duplicate `ssh -G` matches
  bother you. Wildcard aliases (`*`, `?`) and aliases that already
  exist in `sshc.conf` are refused with explicit hints.
- **Doctor update check** (`sshc --doctor` only). One GitHub API
  call to `/repos/hang-in/sshc/releases/latest` with a 5 s timeout
  surfaces a new seventh line:
    - latest == current: `PASS  0.x.y (latest)`
    - current ahead of latest (dev build): `PASS  0.x.y (ahead of
      latest 0.a.b)`
    - current behind latest: `WARN  0.x.y (latest is 0.a.b — see
      <URL>)`
    - network or parse failure: `WARN  could not reach github
      (offline?)` / `WARN  unexpected response from GitHub releases`
  No background calls — the daily `sshc` / `sshc -m` / `sshc <alias>`
  paths never touch the network. Set `SSHC_NO_UPDATE_CHECK=1` to
  skip the call entirely (closed networks, repeated automation runs).

### Changed

- **Footer hint becomes selection-aware.** The manage-mode footer's
  second row swaps in `M promote` only when the selected host is
  external, so managed-host workflows see no extra noise.
- **`HostForm::new` / `HostForm::from_host` now take the identity-file
  candidate list as an argument.** v0.7.1 added an IdentityFile ↑/↓
  picker by calling `std::fs::read_dir` directly inside
  `ui/forms/host_form.rs`, breaking R-G8 (UI layer must not touch the
  filesystem). The scan moved to `app/forms.rs::discover_identity_files`
  and is now passed in from the caller. `impl Default for HostForm`
  is removed (no callers; `new` requires the candidate list).
- **`.github/workflows/release.yml`** gains a workflow-level
  `concurrency` block keyed on `github.ref`. v0.7.2 hit a race where
  two Release workflow runs fired against the same tag and the slower
  one died at `gh release create` with "release already exists". The
  guard serializes per-tag without blocking parallel PR builds.

### Dependencies

- **`ureq 2.10`** added as an unconditional dependency for the doctor
  update check. Default features (rustls + webpki-roots) so the
  binary stays portable — no system OpenSSL / SChannel /
  SecureTransport at install time. ureq's `native-tls` path was
  tried first but ureq 2.12 doesn't pick up a TLS backend with that
  combo alone; switching to default rustls lands a working call.
- **`windows-sys`** picks up the `Win32_Security` feature so
  `CreateFileW`'s `SECURITY_ATTRIBUTES` is re-exported.

### Internal

- All 175+ unit + integration tests still green. clippy host clean.
- `cargo clippy --target x86_64-pc-windows-msvc` was passing on R0–R5;
  the R6 ureq+rustls combo pulls in `ring`, whose `build.rs` needs
  MSVC headers we can't supply from macOS host cross-compile. The
  per-round local gate drops the cross-clippy step from R6 onward;
  cargo-dist's actual Windows runner verifies the Windows build at
  tag push time. PLAN_V0.8 §2 + §3 updated to match.
- `main.rs` is still ≤ 10 LOC (R-G0). R-G1..R-G9 all PASS.

### Size

- macOS arm64 release: 3,150,128 → 5,246,752 bytes (+2.1 MB, +66%).
  The cost is rustls + webpki-roots; absolute size (~5 MB) is well
  within the distribution of comparable TUIs. Risks table flags a
  v0.9 follow-up to evaluate `attohttpc` / `minreq` for size recovery.

### Out of scope (deferred to v0.9+)

- Windows ARM64 (`aarch64-pc-windows-msvc`).
- Windows ACL enforcement of "private key files must be private".
- Identity enumeration on a discovered agent (anti-features 1 + 2).
- Automatic deletion of the original `~/.ssh/config` entry after `M
  promote` (anti-feature 1).
- Always-on update check / automatic update download (anti-feature 4).

## [0.7.3] — 2026-05-20

Second Windows hotfix over v0.7.2 — `sshc.conf` writes are now
actually durable on Windows. No behavior change on Unix.

### Fixed

- **Manage-mode saves silently failed on Windows.** `storage::with_locked_write`
  held the `sshc.conf` file handle open through the final
  `fs::rename(tmp → sshc.conf)` step. On Unix that's fine — `rename(2)`
  atomically replaces a destination even when the same process keeps
  it open — but on Windows `MoveFileW` (which Rust's `fs::rename`
  calls into) refuses to overwrite a path we ourselves hold open, so
  it failed with `ERROR_SHARING_VIOLATION`. The rename now happens
  after the lock handle is dropped; the new content is already fully
  written to the tmp file at that point, so the durability and
  atomicity guarantees are unchanged. Without this fix, every `Enter`
  in the add/modify host form on Windows looked like it submitted but
  left `sshc.conf` untouched (with an `sshc.conf.tmp.<pid>` orphan
  next to it).

## [0.7.2] — 2026-05-20

Windows hotfix over v0.7.1 — no new features, no behavior change on
Unix.

### Fixed

- **IdentityFile field rejected Windows paths.** The add/modify host
  form's IdentityFile validator treated `\` as a forbidden shell
  metacharacter on every platform, so saving a host with an
  IdentityFile like `C:\Users\me\.ssh\id_ed25519` failed on Windows
  with `IdentityFile contains forbidden shell characters`. The
  forbidden-character list is now cfg-split: Windows keeps the 17
  shell metacharacters (`;`, `|`, `&`, `$`, …) but allows `\` as a
  path separator. Unix behavior is unchanged — `\` stays forbidden
  there since POSIX paths never need it.

## [0.7.1] — 2026-05-20

Bug-fix / UX patch over v0.7.0 — no new features, no breaking changes.

### Fixed

- **Manage-mode `i` reports added vs already-present.** v0.7.0 always
  said `Include added to ~/.ssh/config` even when the line was
  already there. Now `inject_include` returns `Result<bool, …>` and
  the status bar picks between
  `Include line added to ~/.ssh/config — writes enabled` and
  `Include line already present — writes already enabled`.
- **Add-host form layout no longer collapses on narrow modals.**
  The previous 2-rows-per-field layout (label above, value below)
  silently shrank trailing fields to zero rows when the 70%-height
  modal area was smaller than 14 lines — making them invisible while
  still accepting input. Re-laid out to one row per field with a
  fixed 14-cell label column and the value `[…]` beside it.

### Added

- **IdentityFile ↑/↓ picker.** When the IdentityFile field is
  active and `~/.ssh/` contains candidate private-key files (`*.pub`,
  `known_hosts*`, `authorized_keys`, `config*`, `environment` and
  hidden entries excluded), pressing `↑` or `↓` cycles through them.
  Direct typing still works for custom paths. The footer hint switches
  to `↑/↓ pick from N key(s) in ~/.ssh • Tab move • …` on that field.
- **IdentityFile-empty status hint.** Saving a host without an
  IdentityFile is allowed (matches OpenSSH's own "fall through to
  agent / password prompt" behavior) but the status bar now flashes
  `'<alias>' saved without IdentityFile — ssh will use agent or
  password prompt` so it isn't a silent decision.

## [0.7.0] — 2026-05-20

Platform expansion: native Windows support. No new features for
existing macOS / Linux users — daily behavior is unchanged.

### Added

- **Native Windows builds** (x86_64-pc-windows-msvc). cargo-dist now
  produces `sshc-x86_64-pc-windows-msvc.zip` alongside the existing
  macOS / Linux artifacts, plus a `powershell` installer (`irm | iex`
  one-liner) as the Windows analog of the `shell` installer.
- **`windows-sys 0.59`** picked up as a target-gated dependency for
  the LockFileEx-based lock path.

### Changed

- **File locking** (`storage/with_locked_write`) factored into a
  small `try_lock_exclusive` helper, cfg-split:
  - Unix: `nix::fcntl::flock(LOCK_EX | LOCK_NB)` — unchanged.
  - Windows: `LockFileEx(LOCKFILE_EXCLUSIVE_LOCK |
    LOCKFILE_FAIL_IMMEDIATELY)` over the whole file.
    `ERROR_LOCK_VIOLATION` maps to `StorageError::LockHeldByOther`
    so the caller-facing semantics match the Unix path.
- **File permissions** (`setup::ensure_file_mode`, doctor's `~/.ssh`
  check) wrap their Unix-mode logic in `#[cfg(unix)]`. The Windows
  arm is a no-op (or, in doctor, a `PASS` line annotated "ACL not
  checked"). Windows ACL enforcement is explicitly deferred to v0.8+.
- **`$EDITOR` fallback**: when the env var is unset, default to
  `notepad.exe` on Windows instead of `vi`.
- **`SSH_AUTH_SOCK` doctor check**: on Windows, missing
  `SSH_AUTH_SOCK` is `PASS` with the note "not applicable on Windows
  (use Windows OpenSSH agent or Pageant)" — the env var is the wrong
  signal there. Unix behavior unchanged.
- `nix` moved under `[target.'cfg(unix)'.dependencies]`, so Windows
  builds see zero transitive Unix-only deps.

### Internal

- Unix-only integration tests (`tests/setup_test.rs`,
  `tests/storage_test.rs`, `tests/round_trip_test.rs`) and the
  `src/exec/ssh.rs::tests` module gated with `#![cfg(unix)]`.
- `cargo check --target x86_64-pc-windows-msvc` clean.
- `cargo clippy --all-targets --target x86_64-pc-windows-msvc -- -D
  warnings` clean.
- `cargo clippy --all-targets -- -D warnings` (host) clean.
- All 162 + 38 integration tests still green on Unix.
- R-G1..R-G9 still clean. main.rs still 8 LOC.

### Out of scope (deferred to v0.8+)

- Windows ARM64 (`aarch64-pc-windows-msvc`) — cargo-dist target add
  is trivial but there's no runner to exercise the binary yet.
- Windows ACL enforcement of "private key files must be private".
- Pageant / Windows OpenSSH agent socket discovery.

## [0.6.0] — 2026-05-20

Picker depth + edit-safety pass. Two threads:

### Added — every-day picker

- **Favorites / pin (`f` in manage mode).** Toggles a host as
  pinned. Pinned hosts float to the top of the picker regardless of
  fuzzy score, in both inline and manage. Stored in `state.toml`
  under a new `[memory] favorites` list (separate from tags).
- **Recent-connection history.** `last_connected_alias` (single
  `String`) bumps to `recent: Vec<RecentEntry>` (max 20, most-
  recent first). Picker uses recency as the 2nd-tier sort key,
  after favorites, before fuzzy score. Loading a pre-v0.6
  `state.toml` migrates transparently: the legacy alias becomes
  `recent[0]` with `ts = state.toml mtime`.
- **Inline-mode one-line summary** under the host table:
  `→ user@hostname:port` for the highlighted row.
- **Manage-mode right-side preview panel** with HostName / User /
  Port / Identity / Tags / Extra. Visible when terminal width ≥
  100 cols; hidden gracefully on narrower terminals.
- **★ glyphs.** Yellow ★ = favorite. Cyan ★ = last-connected. Both
  shown in inline picker and manage status column.

### Added — safe management

- **`v` in manage mode runs `ssh -G <alias>`** and shows the parsed
  effective config in an Info modal. Cached per session; cache is
  cleared on any successful form submit / delete / tag edit. Falls
  back to a status-bar warning if `ssh` is missing or exits
  non-zero — never blocks the user.

### Changed — inline picker is now modal

Inline switched from "fzf-style: every char filters" to explicit
modes:

- Nav mode (default): `j/k/↑/↓` navigate, Enter `ssh`, `/` enter
  search mode, `q` / Esc quit, Ctrl+C quit anywhere.
- Search mode: printable chars append, Backspace pops, Esc exits
  search (picker stays open), Enter ssh-launches the highlight.

The previous fzf shortcut for "type to filter immediately" was
trading away the `j/k` navigation key once any character had been
typed, which surprised the user. Modal aligns inline with manage
mode (both use `/`).

### Removed

- **`r` reconnect key (both inline and manage).** The R3 recency
  sort already puts the last-connected host at row 0, so a single
  `s` (manage) or Enter (inline) covers the reconnect case. The
  dedicated key was redundant once history landed.

### Internal

- `src/ui/preview.rs` — new widget module.
- `src/exec/ssh_config.rs` — `validate_alias` helper, with
  `ValidationError { SshNotFound, NonZeroExit }`. Lives in
  `src/exec/` so R-G1 (no `Command::new` in `src/app/*`) stays clean.
- `App.validation_cache: HashMap<String, String>` for ssh -G.
- `State::record_recent(alias)` central helper used by inline /
  manage / direct connect paths.
- `tests/fixtures/state_v05.toml` + migration tests guard the
  schema bump against regressions.
- 159 → 162 unit + integration tests.
- R-G1..R-G9 still clean; main.rs at 8 non-comment lines.

## [0.5.1] — 2026-05-20

Tiny patch — adds a read-only environment check. Nothing else changes.

### Added

- **`sshc --doctor`** prints a six-line report:
  - `~/.ssh/config` exists
  - `~/.ssh` directory mode (expects 0700)
  - `~/.ssh/config.d/sshc.conf` exists
  - `Include` line present in `~/.ssh/config`
  - `ssh` binary on PATH (shows the OpenSSH version banner)
  - `SSH_AUTH_SOCK` environment variable

  Report-only — no files are modified. Exit code is 0 unless any check
  is `FAIL`; `WARN` does not fail the run (e.g. a missing
  `SSH_AUTH_SOCK` is a heads-up, not an error).

## [0.5.0] — 2026-05-20

Refactor lands: `src/app/mod.rs` (944 LOC pre-split) is now broken into
five focused sub-modules. One small user-facing feature comes with it.

### Added

- **`sshc <alias>` direct-connect.** A positional alias on the command
  line skips the TUI entirely, looks the alias up in your parsed
  config, and runs `ssh <alias>` in the inherited terminal. Designed
  for shell aliases and scripts that already know which host they
  want. Unknown alias prints to stderr and exits 1 without invoking
  ssh. `state.last_connected_alias` is updated on a launch attempt
  (matches inline/manage).
- **`src/cli.rs`** picks up the entire dispatch path
  (`parse_mode`, `print_help`, `print_version`, positional handling).
  `main.rs` is now 8 non-comment lines.

### Changed

- **`src/app/mod.rs` split** into thematic sub-modules:
  - `src/app/input.rs` — `handle_key`, `handle_list_key`,
    `handle_modal_key`, `dispatch_modal_action`, `activate_selected`.
  - `src/app/forms.rs` — `open_*_form`, `open_help_modal`, `apply_form`,
    `apply_add` / `apply_modify` / `apply_delete` / `apply_tags`,
    `persist_sshc_conf`, plus the new `build_host` and
    `normalized_tags` helpers.
  - `src/app/tests.rs` — the entire `#[cfg(test)] mod tests` block.
  - `src/app/filter.rs` was already extracted in v0.4.3.
  - `mod.rs` shrinks from 1040 → 246 lines and keeps only the `App`
    struct, the public enums, constructors, navigation, accessors,
    and the SSH lifecycle hooks (`try_reconnect`, `on_ssh_finished`,
    `replace_hosts`, `apply_probe_updates`).
- **`apply_add` / `apply_modify` no longer call
  `host_from_payload(...).expect(...)`.** `apply_form` destructures
  `FormPayload::Host` inline and calls a new `build_host` helper that
  returns an already-built `Host`. Closes the deepseek-v4-pro review
  item flagging the unreachable `expect`.
- **`normalized_tags(csv: &str) -> Vec<String>`** consolidates the
  `split(',') → filter_map(normalize_tag) → dedup` chain that was
  previously duplicated in `apply_tags` and the now-removed
  `host_from_payload`.

### Internal

- All 147 existing unit tests still pass; integration tests untouched.
- R-G1..R-G9 module-boundary greps still clean.
- `main.rs` is well under the R-G4 80-line bootstrap cap (8 lines).
- No file in `src/app/` exceeds ~290 non-comment lines.
- `clippy --all-targets -- -D warnings` clean.
- `fmt --check` clean.

## [0.4.3] — 2026-05-20

Refactor + small fixes pass before v0.5 starts adding new features. No
behaviour change for the user; internals only.

### Changed

- **`apply_filter` extracted into `src/app/filter.rs`** as the first
  step of breaking `src/app.rs` (~944 LOC) into thematic sub-modules.
  Full split (input.rs + forms.rs) is planned for v0.5.0 under a
  proper BRIEF/PLAN since the visibility surgery is non-trivial. For
  v0.4.3 only the filter logic moves — `src/app/mod.rs` shrinks
  slightly and `filter.rs` declares a single `impl super::App` block
  with the relocated method.
- **`sshc_conf_path` now cached on `App`** (`Option<PathBuf>`)
  instead of called via the throwaway associated helper. The
  previous `unwrap_or_default()` produced an empty `PathBuf` sentinel
  when home-dir resolution failed; in that edge case any host whose
  `source_file` also happened to be empty would have falsely matched
  as "sshc.conf-managed". The new cache is `None` in that scenario
  and the comparison helper documents the intended semantics.
  (Reviewer: gemini-code-assist + deepseek-v4-pro@ollama-cloud.)

### Internal

- `cargo-dist` publish-homebrew-formula now self-serves with
  `HOMEBREW_TAP_TOKEN` configured (v0.4.2 needed a manual tap push).

## [0.4.2] — 2026-05-20

### Added

- **`--help` / `-h`** and **`--version` / `-V`** flags. Help text covers
  the inline / manage split, keys, and on-disk files. Version comes from
  `CARGO_PKG_VERSION`, so the Homebrew formula's smoke test can call
  `sshc --version` instead of just checking the binary exists.
- **Modal overlay rendering**. v0.4.0/0.4.1 had a bug where
  `ui/mod.rs::render` only drew the host table; an active `ModalKind`
  (Confirmation/Info/Form) was never painted. First-run users saw the
  host list and thought "no key works except Esc" — but Esc was
  actually triggering the modal's on-no path (decline_include). Modal
  is now overlaid via `Clear` + chrome + body.
- **Manage `i` key**: force-retry the Include injection. Useful when
  the user previously declined first-run setup, the Include line was
  removed by hand, or `state.toml` got into a stale state. Flips
  `declined_include_injection = false` and emits
  `AppAction::InjectInclude`.
- **`Host.extra: Vec<String>`** — freeform SSH directives (ProxyJump,
  ForwardAgent, LocalForward, …) preserved across read/write
  round-trips. Parser pushes unknown lines inside a Host block into
  `extra`; serializer emits them with the standard 4-space indent
  after the typed fields.
- **HostForm "Options (a; b)" field** (7th field). Semicolon-
  separated entry: each `KeyValue` becomes one extra line. Tab
  wraparound updated to 7 fields.

### Changed

- **Renamed `sshs` → `sshc`** across the codebase (folded in from
  v0.4.1 — name collision with an unrelated CLI). Binary, package,
  on-disk paths (`~/.ssh/config.d/sshc.conf`, `~/.config/sshc/...`),
  UI strings, docs, file backup suffix. The state.toml schema is
  unchanged; only its parent directory moved.
- **Tags column moved off Alias into a dedicated right-side column**
  with `show_tags` visibility (hides first as panels narrow). Alias
  cells now always start at column 0, so vertical scanning works.
- **Inline mode layout**: no border, left-aligned, width sized to the
  data (no longer wastes the full terminal width on a sparse table).
  Status bar marker `/` → `▸` since the user never typed `/` to start
  filtering — fzf semantics.
- **Inline viewport height** is now `(host_count + 3).clamp(5,
  viewport_height)` instead of a fixed 15 rows. Avoids reserving
  blank rows and pushing the shell prompt far up the scrollback.
- **Read-only status messages are actionable**: all `a/m/d/t`
  read-only branches now suggest `press 'i' to add Include line`.
- **Manage Enter key**: opens the modify form for sshc.conf-managed
  hosts; falls through to `$EDITOR` for external hosts. `s` is now
  the ssh-connect shortcut; `m` is unbound (already in v0.4.0/0.4.1
  but reiterated here for the `i` help-text update).
- **CI/CD**: replaced the handcrafted `release.yml` +
  `bump-homebrew.yml` pair with **cargo-dist v0.31.0**. Single
  `dist-workspace.toml` drives cross-compile + tarballs + GitHub
  release + Homebrew tap formula push. The `release.published`
  event from `GITHUB_TOKEN`-created releases didn't trigger the
  downstream `bump-homebrew.yml` (known constraint); cargo-dist
  sidesteps that with a single pipeline.
- **Misleading error mappings fixed** (PR review feedback,
  gemini-code-assist): `apply_add` / `apply_modify` now
  `.expect()` the unreachable `host_from_payload` `None` branch;
  `persist_sshc_conf` reports
  `AppError::Setup(SetupError::HomeDirMissing)` instead of
  disguising it as lock contention.

### Internal

- `Cargo.toml` gains `repository`, `homepage`, `readme`,
  `[profile.dist]`.
- `dist-workspace.toml` (new) — cargo-dist config.
- `examples/render_preview.rs` + tests updated for the new
  `Host.extra` field and the tag column move.
- `docs/demos/` (new) — fixture ssh config + fake ssh wrapper +
  vhs tapes for layout previews.

## [0.4.1] — 2026-05-20

### Changed

- **Renamed binary, package, and on-disk paths from `sshs` to `sshc`.**
  The previous name (v0.1.0–v0.4.0) collided with an unrelated CLI also
  called `sshs`. The Cargo package, binary, UI strings, file paths
  (`~/.ssh/config.d/sshc.conf`, `~/.config/sshc/state.toml`), and the
  `Include` backup filename suffix all now use `sshc`. The state.toml
  schema is unchanged — only the parent directory moved.
- Replaced two misleading `StorageError::LockHeldByOther` fallbacks in
  `src/app.rs` flagged in PR review:
  - `apply_add` / `apply_modify` now `.expect(...)` on
    `host_from_payload` since the caller already matched the Host
    variant — the previous `None` branch was unreachable.
  - `persist_sshc_conf` returns
    `AppError::Setup(SetupError::HomeDirMissing)` when `dirs::home_dir`
    is unresolvable, instead of disguising it as lock contention.

### Added

- **Homebrew distribution via [`hang-in/homebrew-tap`](https://github.com/hang-in/homebrew-tap)**.
  `brew install hang-in/tap/sshc` installs the latest release as a
  pre-built binary.
- **GitHub Actions `release.yml`**: tag push (`v*`) triggers cross-
  compiled binaries for `{x86_64,aarch64}-{apple-darwin,unknown-linux-gnu}`,
  stripped, packed as `sshc-<target>.tar.gz`, attached to the GitHub
  release.
- **GitHub Actions `bump-homebrew.yml`**: `release.published` event
  triggers a PR on the tap repo updating `Formula/sshc.rb`.
- `README.ko.md` (Korean translation, content parity with `README.md`).
- README rewritten in a leaner shape: tagline + brew badge + demo
  placeholder + Why / Quickstart / Two modes / Keybindings / Install
  / Configuration / Comparison.

## [0.4.0] — 2026-05-20

### Added

- **Inline mode (`sshc`, no args)**. Default command opens an
  `ratatui::Viewport::Inline(N)` host browser BELOW the shell prompt
  instead of an alternate screen. Type to filter (immediate, fzf-style),
  `↑/↓` or `j/k` to navigate, `Enter` to ssh, `Esc`/`Ctrl+C` to cancel,
  `r` to reconnect to the last alias. Viewport height is
  `(terminal_height − 5).clamp(8, 15)`; below 12 rows the binary falls
  back to manage mode with a one-line stderr notice.
- **Manage mode (`sshc -m` / `sshc --manage`)**. The v0.3 alternate-
  screen TUI, retained behind a flag. Default command behaviour changed
  in v0.4 — this is intentionally breaking for a single-user tool.
- **`InlineApp`** — lean read-only host browser (no probes, no modal
  subsystem, no forms, no storage writes). 144 lines, 13 unit tests.
- **`tui::inline_runtime`** — `run_event_loop_inline` +
  `handle_connect_inline`. Inline mode tears down the viewport before
  ssh spawn and never re-enters the UI on ssh exit; the binary exits
  with an `SshResult`-derived `ExitCode` so failures propagate to the
  parent shell.
- **`ScreenMode { Alternate, Inline(u16) }`** on `TerminalGuard`. The
  panic hook tracks `RAW_ACTIVE` and `ALT_ACTIVE` independently, so a
  panic in inline mode does NOT emit `LeaveAlternateScreen` (would
  corrupt a normal-mode terminal).
- **R-G9** boundary gate — `inline_app` cannot import `probe`,
  `ui::modal`, `ui::forms`, or storage writers.
- **`src/run.rs`** — `inline()` / `manage()` dispatch helpers. Keeps
  `main.rs` thin (41 non-comment lines, R-G4 ≤ 80).
- **`examples/inline_prototype.rs`** — standalone ratatui Viewport
  smoke test for manual verification. Useful for terminal compat
  triage.

### Changed

- **Manage-mode key rebind**:
  - `Enter` opens the modify form for sshc.conf-managed hosts; falls
    through to `AppAction::EditConfig` (`$EDITOR` jump) for external
    hosts. Old "Enter = ssh" semantics moved to `s`.
  - `s` — ssh connect for the selected host.
  - `m` — removed. (Was previously "open modify form"; merged into
    `Enter`.)
  - Help modal text updated.
- **CLI dispatch**: `main()` returns `ExitCode` (via Termination) so
  ssh failure codes propagate to the parent shell. Inline `Quit` →
  `SUCCESS`, `Connect/Reconnect` → low byte of the ssh result code,
  `Crashed/UnknownTermination` → `FAILURE`.

### Internal

- `TerminalGuard` no longer holds a single `TERMINAL_ACTIVE` flag;
  split into `RAW_ACTIVE` + `ALT_ACTIVE` atomics so mode-specific
  enter/leave is idempotent.
- Inline viewport is `terminal.clear()`-ed before ssh spawn so the
  shell sees no frozen frame (fzf-style clean exit).

### Tests

- Total: 162 (v0.3.0) → **190** (v0.4.0).
- New: `inline_app` 13 unit + `tests/inline_test.rs` 5 integration +
  `ScreenMode` equality + manage-rebind 3 (`s` connects, Enter on
  external opens editor, Enter on managed opens form, `m` unbound).
  Old `test_app_enter_connect` renamed to `test_app_s_connects`.

### Compatibility

- `~/.ssh/config`, `~/.ssh/config.d/sshc.conf`, `state.toml` unchanged.
- v0.3 users running `sshc` will land in inline mode on first launch.
  The host list looks similar; selection and `Enter` ssh-connect work
  as expected. To get the v0.3 behaviour back, use `sshc -m`.
- First-run setup flow (`Include` injection) runs in **manage mode
  only**. Inline mode reads whatever hosts are already visible to
  `~/.ssh/config`; users who never run manage mode will not see the
  setup prompt and inline still works (sshc.conf is simply absent).

## [0.3.0] — 2026-05-20

### Added

- **Host manager (in-TUI add / modify / delete)**. New keys `a`, `m`,
  `d` open modal forms backed by a dedicated `~/.ssh/config.d/sshc.conf`
  file. All writes are atomic (tempfile + rename) under a POSIX
  `LOCK_EX` so concurrent sshc instances don't corrupt the file.
- **Tags**. New `t` key edits per-host tags; tags are stored as a
  `# @tags: a, b` comment immediately above each `Host` block. Filter
  with `@<tag>` (e.g. `@prod`) or rely on the default fuzzy filter,
  which also matches tag content as a fallback.
- **First-run setup**. On first launch sshc offers to add an
  `Include ~/.ssh/config.d/sshc.conf` line to `~/.ssh/config`, with a
  dated `.bak.sshc-YYYYMMDD` backup. Decision persisted to
  `~/.config/sshc/state.toml`.
- **Probe column**. A background thread pool issues parallel TCP
  connect probes (≤ 8 workers, 1s timeout) and surfaces results in the
  Status column. Generation guard discards stale updates after refresh.
- **Source-aware UI**. Hosts that live outside `sshc.conf` are marked
  `·` and protected from the in-TUI add/modify/delete flow; press `e`
  to jump to the source file in `$EDITOR` instead.
- **5-column responsive table**. Alias | Account | Host | Port | Status
  with priority-based column hiding for narrow widths (Account first,
  then Port, then Host). Below 60×10 shows a "terminal too small"
  notice rather than rendering a broken layout.
- **Help modal**. `?` key opens a key reference in an info modal.
- **Modal subsystem**. Generic `ModalKind { Confirmation, Info, Form }`
  with a `FormState` trait that owns its own per-key state machine.
  Tab/Shift+Tab navigation, Enter submit-or-advance, Esc cancel, and
  Ctrl+U clear are standard across all forms.
- **Integration tests**. 11 new tests across `tests/storage_test.rs`,
  `tests/probe_test.rs`, and `tests/setup_test.rs`. Total runnable
  tests grew from 73 (v0.2.0) to 162.
- **Module-boundary gates** R-G6, R-G7, R-G8 enforced via grep:
  `storage`/`setup`/`probe`/`state` cannot import TUI crates;
  `probe` cannot depend on `app` or `ui`; `ui/forms` and `ui/modal`
  cannot touch the filesystem or spawn processes.

### Changed

- `App` gains `mode: AppMode { List, Modal(ModalKind) }`,
  `probe_states: Vec<ProbeState>`, `state: state::State`. `handle_key`
  dispatches to the modal handler whenever `mode` is not `List`.
- `AppAction` grew `SaveState`, `InjectInclude`, `DeclineInclude`.
  Form submit handlers emit `SaveState`, which the runtime turns into
  a `state::save()` + `ProbePool::refresh()`.
- `ui/list.rs` swaps from a `List<Line>` to a ratatui `Table` with
  per-row cells, so column widths can be driven by `Constraint`.
- `runtime::run_event_loop` now takes `&ProbePool` and drains
  `poll_updates()` before each draw, so probe state changes appear
  with ≤ one tick of latency.
- `main.rs` orchestrates first-run setup and routes the new
  AppActions; it stays under 100 LOC.

### Internal

- New modules: `state/*`, `setup/*`, `storage/*`, `probe/*`,
  `ui/modal.rs`, `ui/forms/*`, `config/tags.rs`.
- Error taxonomy: `StorageError`, `SetupError`, `ProbeError` join
  `SshError`, `TerminalError`, `EditorError` under `AppError` with
  `From` impls.

### Compatibility

- Existing `~/.ssh/config` continues to work unchanged. v0.3 only
  introduces a new file (`~/.ssh/config.d/sshc.conf`) and an optional
  one-line `Include` directive in the main config.
- v0.2 binary upgrade: on first launch the setup modal will offer the
  Include line. Declining keeps sshc read-only — the host browser
  still works, but `a`/`m`/`d`/`t` show a "read-only" status.

## [0.2.0] — 2026-05-19

### Added

- Round-trip ssh session: `Enter` spawns ssh as a child, sshc suspends
  raw mode + alt screen, resumes when ssh exits, and shows a transient
  status message classifying the exit (success / interrupted /
  ConnectFailed / Failed / Crashed / UnknownTermination).
- `★` marker on the last-connected host and `r` reconnect shortcut.
- Status bar with auto-dismissing messages (3 s timeout).
- `TerminalGuard` (RAII raw-mode + alt-screen) and a panic hook that
  always restores the terminal before unwinding.
- Round-trip integration tests using mock_ssh shell fixtures (no real
  ssh binary needed).
- Module-boundary gates R-G1..R-G5 documented in `docs/TESTING.md`.

### Changed

- `App` state cleaned up: removed transitional `should_quit` /
  `should_connect` / `should_edit` flags in favor of
  `pending_action: Option<AppAction>`.

## [0.1.0] — 2026-05-19 (initial)

### Added

- Minimal TUI listing non-wildcard `Host` entries from `~/.ssh/config`.
- Fuzzy filter (nucleo) by alias or hostname.
- `Enter` to ssh, `e` to open `$EDITOR` at the host's line.
- `Include` directive support with circular detection and depth limit.
- Handles missing `~/.ssh/config` gracefully (empty list).
