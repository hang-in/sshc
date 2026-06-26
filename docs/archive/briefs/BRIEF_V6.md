# sshc v0.6.0 — Architect Brief

> Companion docs:
> - `PLAN_V0.6.md` — round breakdown + delegation payloads (to be drafted before R0)
> - `docs/REFACTOR_NOTES.md` — running log of ollama-cloud review pitfalls + recipes
> - `BRIEF_V5.md` — v0.5 brief + §9.2 anti-features (carry over verbatim)
> - `CHANGELOG.md` — shipped history (latest: v0.5.1)

## 1. Context

v0.5.0 (2026-05-20) split `src/app/mod.rs` into thematic sub-modules
(filter/input/forms/tests + mod) and added `sshc <alias>` direct-connect.
v0.5.1 (same day) added `sshc --doctor` as a read-only environment
report. The daily touch-point — type, Enter, ssh — is now the single
biggest leverage point for further work.

v0.6 deepens that touch-point with **pinning + recency**, adds a
**preview panel** to manage mode, and brings in **`ssh -G` validation**
as an explicit safety net for managed-config edits.

## 2. v0.6 Goal

Two threads in one minor release.

### Thread A — every-day picker (high leverage)

1. **Favorites / pin** — toggle a host as pinned with a key in manage
   mode. Pinned hosts float to the top of the picker (both inline and
   manage) regardless of fuzzy score. Stored separately from tags so
   the two systems don't collide.
2. **Recent-connection history** — `state.last_connected_alias`
   (single `String`) bumps to a bounded `Vec<RecentEntry>`. Picker
   uses recency as a secondary sort key after favorites, before fuzzy
   score.
3. **Inline-mode status line** — a one-line `Host / User / Port`
   summary below the selected row. The preview panel goes to manage
   mode only (inline viewport is too narrow for a side panel).
4. **Manage-mode preview panel** — right-side panel showing
   `HostName / User / Port / IdentityFile / Tags / Extra` for the
   currently selected host.

### Thread B — safe management (trust)

5. **`ssh -G <alias>` validation** — opt-in. A key in manage mode
   runs `ssh -G <selected_alias>` in a child process and shows the
   parsed config (verbatim or lightly grouped) in an Info modal.
   Cached per `(alias, sshc.conf mtime)` for the session.
   **Critically**: validation never blocks a save. A failed `ssh -G`
   is a `WARN`-level status message, not a `FAIL`.

## 3. State schema migration

Current `state.toml` shape (simplified):

```toml
[memory]
last_connected_alias = "prod-db"
```

v0.6 shape:

```toml
[memory]
# Most-recent-first. Bounded to RECENT_MAX (start with 20).
recent = [
  { alias = "prod-db", ts = "2026-05-20T14:31:02Z" },
  { alias = "nas",     ts = "2026-05-20T11:08:00Z" },
]
# Order-preserving. Used by the picker as the primary sort.
favorites = ["prod-db", "router"]
```

**Backwards-compat plan**: deserialise via a `LegacyMemory` shim
struct that accepts both schemas (`#[serde(default)]` on new fields,
optional `last_connected_alias: Option<String>`). If `recent` is
empty but the legacy field is present, `state::load` migrates:
`recent = [{alias: <that>, ts: state.toml file mtime}]`. Using the
file mtime — not `epoch 0` — keeps the migrated entry from sinking
to the bottom of recency sort the first time the user opens v0.6.
The old field is **read** for one release, never written; v0.7
removes the migration path.

**Migration test fixture**: ship a `tests/fixtures/state_v05.toml`
with the old schema. New test `state_migration_from_v05_loads`
asserts that loading it produces a single-entry `recent` and an
empty `favorites`, and that re-saving emits only the new schema.

Existing setup state (`include_check_done`,
`declined_include_injection`) untouched.

`InlineApp` and `App::last_connected` (current `Option<String>`)
become thin accessors that read `recent[0]`. `r` (reconnect) still
behaves the same from the user's perspective.

## 4. Module placement

| New artifact | Location | Visibility |
|---|---|---|
| `RecentEntry` struct + `RECENT_MAX` const | `src/state/mod.rs` | `pub` |
| `favorites: Vec<String>` field in `State::memory` | `src/state/mod.rs` | `pub` |
| `App::is_favorite(alias) / toggle_favorite(alias)` | `src/app/mod.rs` or new `src/app/favorites.rs` (file split if >50 LOC) | `pub` |
| Sort comparator (favorite > recency > fuzzy) | `src/app/filter.rs` (`apply_filter` rework) | `pub(super)` |
| Preview panel widget | new `src/ui/preview.rs` | `pub(crate)` |
| Inline status line summary | extend `src/inline_app.rs` (or `src/ui/inline_status.rs`) | local |
| `ssh -G` runner | new `src/exec/ssh_config.rs` (NOT in `src/app/*` — R-G1) | `pub` |
| Manage-mode key handlers for `f`, `v` | `src/app/input.rs` | private |

## 5. UI surface changes

### Manage-mode keybindings (delta)

| Key | Action |
|---|---|
| `f` | Toggle favorite on selected host. Status line flash: `★ pinned` / `pin removed`. |
| `v` | Run `ssh -G <selected_alias>` and open an Info modal with the result. Esc / Enter to dismiss. |
| `?` (existing help modal) | Updated text mentions `f` and `v`. |

### Inline mode

- Selection highlight unchanged.
- One status line below the highlighted row showing the host
  summary. Only shown when the host list is ≥ 3 rows tall — on
  smaller viewports the existing 2-row status block keeps full
  width.
- Pinned hosts get a `★` glyph in the first column (next to the
  existing `▸` filter marker).
- `f` in inline mode is **not** a toggle. Inline stays a read-only
  picker. The original draft had `f` emit a status hint
  ("favorites are managed via 'sshc -m'"), but inline's
  `KeyCode::Char(c)` catch-all routes every printable char into the
  fzf filter — intercepting `f` would break searches for hosts like
  `fileserver`. Final policy: `f` is just a filter char in inline.
  User learns favorites exist via the `★` glyph (R4) and the
  README/help text. Same zero-mutation policy as add/edit/delete.

### `--help` text

- Add `f` and `v` to the manage-mode key list.
- No new CLI flag (validation is in-TUI only for v0.6).

## 6. `ssh -G` validation strategy

```rust
// src/exec/ssh_config.rs
pub fn validate_alias(alias: &str) -> Result<String, ValidationError> {
    let out = Command::new("ssh")
        .arg("-G")
        .arg(alias)
        .stdin(Stdio::null())
        .output()?;            // never blocks indefinitely — ssh -G returns immediately
    if !out.status.success() {
        return Err(ValidationError::SshExitNonZero {
            code: out.status.code(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        });
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}
```

- **No timeout knob** — `ssh -G` parses local config only, no
  network. Empirically <50 ms on a 200-host config.
- **No parsing** in v0.6. Display raw stdout. Future v0.7 could
  group by directive, but v0.6 keeps the surface small.
- **Loading state**: while the child runs, show
  `Validating <alias>…` in the status line. Run synchronously
  (`ssh -G` returns in <50 ms on local config), no background
  thread.
- **Caching — alias-keyed**: `HashMap<String, String>` keyed by
  alias, invalidated by a central `App::invalidate_validation_cache()`
  called from `apply_add` / `apply_modify` / `apply_delete` /
  `apply_tags`. Rejected the hash-keyed alternative
  (`HashMap<(alias, sha256(host_block)), String>`) because it adds
  a sha2 dep and a per-keystroke hash cost just to remove one
  invalidation call. Revisit if a non-form mutation path appears.
- **Error handling**: any `Err` is rendered as a `WARN`-status modal,
  never blocks the user. Stderr from ssh is included verbatim so
  config syntax errors become visible.

## 7. R-G gates

No new gates. Confirm existing R-G hold:

- **R-G1** (no `Command` in `src/app/*`): `validate_alias` lives in
  `src/exec/ssh_config.rs`. `App` calls into `crate::exec::ssh_config`
  via the dispatch path, same as `ssh_run`.
- **R-G4** (`main.rs ≤ 80`): unchanged — no main.rs edits planned.
- **R-G6** (storage/setup/probe/state no TUI deps): `RecentEntry` /
  `favorites` live in `src/state/`; no crossterm/ratatui imports.

After each round, re-run the matrix from `docs/TESTING.md §2`.

## 8. Risks

| Risk | Mitigation |
|---|---|
| State schema migration corrupts existing `state.toml` | `serde(default)`; backup the file on first load when the migration path triggers (`state.toml.bak.v0.5`); never write the old field. |
| `ssh -G` slow / hangs on misconfigured agent | Doesn't read agent / doesn't open sockets. Read-only on `~/.ssh/config*`. If a path-resolution recursion shows up, set an external `Duration::from_secs(5)` wall-clock guard (kill child). |
| Favorites + recency sort interactions surprise the user | Documented order: favorites > recency > fuzzy. Show pinned glyph in the picker so users see *why* a host floats. Add unit tests for the sort comparator with all three signals present. |
| Preview panel layout on narrow terminals | If terminal width < 100 cols, hide the panel and fall back to the existing single-column layout. Width check on every redraw, not just startup. |
| `f` collides with planned filter shortcut | Currently filter is `/`; `f` is free. Audit `rg "KeyCode::Char\('f'\)"` before R2. |
| Cache invalidation forgotten on edit | One central `App::invalidate_validation_cache()` called from `apply_add` / `apply_modify` / `apply_delete` / `apply_tags`. Unit-tested. |
| Frequent `state.toml` writes from toggling favorites | Persist on toggle (atomic write — `with_locked_write` already handles this) and accept the cost. The file is <5 KB; users don't pin 100 times a minute. If profiling later shows churn, batch with a dirty flag flushed on `App::Drop` or `Quit` action. |

## 9. Definition of Done

Tag v0.6.0 only when all of these are true:

- [ ] Favorites: `f` key in manage mode adds/removes pin. Persists.
- [ ] Recent history: 20-entry bounded `Vec<RecentEntry>` in state.
- [ ] Backwards-compat load of pre-v0.6 `state.toml` works (test:
      hand-craft an old file, load, save, verify migration).
- [ ] Inline mode shows host summary line for selected row.
- [ ] Manage-mode preview panel renders for terminals ≥100 cols wide.
- [ ] `v` key runs `ssh -G <alias>` and shows result in Info modal.
      Cache cleared on form submits.
- [ ] `?` help modal text updated with `f` and `v`.
- [ ] README + README.ko updated. Help text in `cli.rs` updated.
- [ ] CHANGELOG `[0.6.0]` entry written.
- [ ] All existing unit + integration tests pass.
- [ ] New unit tests: sort comparator, state migration, favorites
      toggle round-trip, validation cache invalidation.
- [ ] R-G1..R-G9 clean.
- [ ] `clippy --all-targets -- -D warnings` clean.
- [ ] `fmt --check` clean.
- [ ] Manual smoke on macOS: open inline picker, pin a host, exit,
      reopen, verify pin survives. Press `v` on a known-good and
      known-malformed alias.
- [ ] cargo-dist publish-homebrew-formula succeeds on tag push.

## 10. Out of scope

### 10.1 Carry-over from BRIEF_V5 §9.2 (project-wide, permanent NO)

1. Self-built SSH client.
2. Secret / key management (1Password / agent / Keychain territory).
3. Team-shared catalogs, permissions, audit logs.
4. Web UI / daemon.
5. Complete OpenSSH `config(5)` parser.

These do not relax for v0.6 or any future version.

### 10.2 Deferred to v0.7+

- **Probe column / latency / last-seen in inline picker**. Useful
  for homelab cockpit feel; gated behind a probe pool TTL design
  that doesn't block the connect path. See Codex roadmap discussion
  2026-05-20.
- **Mosh / `tailscale ssh` / per-host command override**.
- **Wake-on-LAN action** on a managed host.
- **shell completion** (zsh/bash `sshc <Tab>`).
- **Windows support**.

## 11. Versioning convention (project-wide)

| Version | Reserved for |
|---|---|
| `vX.0.0` (major) | Breaking API / on-disk format changes. No plan in 2026. |
| `vX.Y.0` (minor) | New features + small refactors bundled. |
| `vX.Y.Z` (patch) | Bug fixes, doc-only, small report-only tools (e.g. v0.5.1 `--doctor`). **No new user-facing features in patches.** |

Cement this so v0.6.1 (if/when it happens) is a fix release, not a
"oh let me sneak feature X in".

## End of Brief.
