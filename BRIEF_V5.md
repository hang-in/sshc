# sshc v0.5.0 — Architect Brief

> Companion docs:
> - `PLAN_V0.5.md` — round breakdown + delegation payloads
> - `docs/REFACTOR_NOTES.md` — v0.4.3 split attempts + lessons learned + deepseek-v4-pro:cloud review notes
> - `CHANGELOG.md` — shipped history

## 1. Context

sshc is a Rust TUI for managing SSH hosts. Shipped through v0.4.3
(2026-05-20):

- **v0.1.0..v0.4.0**: feature buildup. Inline + manage modes, fzf-style
  picker, modal forms, tags, probe column, first-run setup, cargo-dist
  publication to `hang-in/homebrew-tap`.
- **v0.4.1**: rename `sshs` → `sshc` (binary, package, on-disk paths).
- **v0.4.2**: `--help`/`--version` CLI, `Host.extra` (freeform SSH
  directives via 7th form field), modal-overlay render bug fix.
- **v0.4.3**: filter.rs extracted from app.rs; App caches
  `sshc_conf_path: Option<PathBuf>` instead of re-resolving every
  comparison.

Stack: Rust 1.85+, ratatui 0.29, crossterm 0.28, nucleo 0.5, nix flock,
serde/toml. 190 automated tests. cargo-dist v0.31.0 ships releases to
Homebrew tap on every `v*` tag push.

## 2. v0.5.0 Goal

Two threads:

1. **Refactor**: finish breaking `src/app/mod.rs` (~890 LOC after v0.4.3)
   into thematic sub-modules. v0.4.3 split filter.rs cleanly; input.rs
   and forms.rs were attempted and reverted because of visibility +
   accessor surface issues. v0.5 does this **carefully, step-by-step,
   one method at a time**, with cargo build green after every move.
2. **Pick one user-facing feature**: candidates listed in §6.
   Decide at BRIEF acceptance time; do not do all of them.

The refactor goes first because it lowers the cost of every subsequent
feature; the feature comes second because v0.5 should ship something
visible (minor version convention).

## 3. Refactor — target layout

```
src/app/
├── mod.rs       App struct + AppAction/AppMode/FormContext +
│                new + new_with_state + try_reconnect +
│                on_ssh_finished + replace_hosts +
│                apply_probe_updates + accessors
│                (host_count / total_host_count / selected_host /
│                 exit_modal / take_action / has_pending_action) +
│                is_read_only + sshc_conf_path_or_blank
├── input.rs     handle_key + handle_list_key + handle_modal_key +
│                dispatch_modal_action + activate_selected
├── forms.rs     open_add_form / open_modify_form / open_delete_confirm /
│                open_tag_form / open_help_modal +
│                apply_form + apply_add / apply_modify / apply_delete /
│                apply_tags + persist_sshc_conf +
│                host_from_payload (free fn)
├── filter.rs    apply_filter   (DONE in v0.4.3)
└── tests.rs     all #[cfg(test)] mod tests content
```

Approximate LOC after split: each file 150–280. mod.rs ≤ 250.

### 3.1 Visibility contract

Methods that the input dispatcher (`input.rs`) must call from
`forms.rs` need to be **at least `pub(super)`**. Specifically:

- `open_add_form`, `open_modify_form`, `open_delete_confirm`,
  `open_tag_form`, `open_help_modal` — called from `handle_list_key`.
- `apply_form` — called from `handle_modal_key` (Form submit).
- `apply_delete` — called from `dispatch_modal_action` (delete_selected).

Methods that `forms.rs` must call from `mod.rs` accessors:

- `selected_host` (already `pub`)
- `host_count`, `total_host_count` (already `pub`)
- `is_read_only` (already `pub`)
- `sshc_conf_path_or_blank` (currently `pub(super)` — keep it).

The `host_from_payload` free function moves to `forms.rs` with no
visibility change (it's a free `fn`, private to the file).

`apply_filter` is already `pub(super)` per v0.4.3 — callers in
`mod.rs` (replace_hosts) and `input.rs` (filter mode keystrokes) reach
it through the inherent visibility of `impl super::App`.

### 3.2 Test relocation

The existing `#[cfg(test)] mod tests` block at the bottom of
`mod.rs` (~250 LOC, ~14 tests) moves to `src/app/tests.rs`. mod.rs
declares it via `#[cfg(test)] mod tests;`. All tests stay; no
test logic changes. The `make_host` / `make_host_with_tags`
helpers move with them.

### 3.3 Acceptance criteria for the refactor

- `cargo test --release` — all 190 tests still pass.
- `cargo clippy --all-targets -- -D warnings` clean.
- `cargo fmt --check` clean.
- R-G1..R-G9 grep matrix still clean.
- `wc -l src/app/*.rs` shows no file > 300 non-comment lines.
- Every commit in the refactor sequence builds (i.e. step-by-step
  moves, not a single mega-commit).
- Public surface unchanged: `use sshc::app::{App, AppAction, AppMode}`
  and `app.handle_key(...)` / `app.take_action()` / etc. compile from
  outside without import path changes.

## 4. Refactor — incidental fixes folded in

Two improvements from the deepseek-v4-pro:cloud review of v0.4.2's
app.rs (`docs/REFACTOR_NOTES.md` has the full transcript):

### 4.1 `host_from_payload` `expect()` redundancy

`apply_add` and `apply_modify` both call
`host_from_payload(payload, &self.sshc_conf_path_or_blank())
.expect("apply_form routes Host payloads to apply_*")`. The `None`
arm is unreachable by construction — `apply_form` already matched
`FormPayload::Host`.

**Fix**: in `forms.rs`, change `apply_form` to destructure the
payload's fields and pass them as typed arguments to a new
`build_host(self, alias, hostname, user, port, identity_file,
tags_csv, extra) -> Host` helper. `apply_add`/`apply_modify` accept
the already-built `Host` and skip the unreachable `expect`.

### 4.2 `normalized_tags(csv: &str) -> Vec<String>` helper

The same `split(',') -> filter_map(normalize_tag) -> dedup` chain
appears in `apply_tags` and in `host_from_payload`. Extract a
single helper. Place it next to the `build_host` helper in
`forms.rs` (or `src/config/tags.rs` if the tags module is the more
natural home — judgment call at implementation time).

## 5. Module-boundary R-G gates

v0.4.3 baseline still passes R-G1..R-G9 (run them again before each
v0.5 commit). The refactor introduces no new gate. After the split,
update `docs/TESTING.md` §2 to mention the file layout (not as a
hard gate, just for orientation).

## 6. v0.5 feature candidates — pick one

Discussed during v0.4.3 review:

| Candidate | Sketch | Estimated cost |
|---|---|---|
| **Favourites toggle** | `★` key (or `f`) marks a host as favourite. Favourites float to top of the list, regardless of filter score. Stored in `state.toml` under a new `[favourites]` section (Vec<String>). | medium |
| **Sort / group** | `S` key cycles sort modes (alias / last_connected / probe_state). Or `g` toggles grouping by tag. | medium |
| **Host quick-clone** | On selected host, key `c` opens a form pre-populated with the current host's fields and an empty alias. Submit creates a sibling host. | small |
| **Multi-line "Options" textarea** | Replace the v0.4.2 semicolon-joined Options field with a real multi-line text widget (one extra option per line). Needs either tui-textarea dep or a small in-house implementation. | medium-large |
| **Custom mini-editor** | Replace external `$EDITOR` for sshc.conf-managed hosts with an in-TUI editor (line-based, no syntax highlight). User-suggested in v0.4.3 chat. **Recommended deferral**: a freeform textarea (#4 above) covers most of the value at much lower cost. | large |
| **Inline mode probe glyphs** | Light TCP-connect pool spun up only when inline mode launches with `--probe` flag. Probe column shows in inline. | medium |
| **`sshc <alias>` direct-connect** | Skip the TUI entirely when an alias is passed: parse, find, ssh. Useful for shell scripts. | small |

Architect recommendation: **Host quick-clone (`c`)** or **`sshc
<alias>` direct-connect** as the cheapest visible wins; favourites
toggle as the medium-cost option that genuinely changes daily use.
Custom mini-editor is large and the user already pre-flagged it
as a "maybe later" item.

User decides; this BRIEF places no constraint other than "pick one,
not all".

## 7. Risks and contingencies

| Risk | Mitigation |
|---|---|
| Splitting app.rs again breaks accessor surface (v0.4.3 attempt did) | One method per commit. After every commit: `cargo build && cargo test --release`. Revert is cheap. |
| Visibility `pub(super)` cascades farther than expected | Audit grep before each move: `rg "fn <method_name>"` then `rg "\.<method_name>\("` |
| `host_from_payload` `Self::sshc_conf_path_or_blank()` callsites move into forms.rs but the helper lives on mod.rs's `App` | `App` is the same type; the impl block in forms.rs picks up the `pub(super) fn sshc_conf_path_or_blank(&self) -> PathBuf` automatically. No fix needed. |
| Tests in tests.rs use `App::sshc_conf_path_or_blank()` (associated fn pattern) | v0.4.3 already converted this to instance method. Tests now call `crate::storage::sshc_conf_path().unwrap_or_default()` directly. |
| `#[cfg(test)]` module visibility from `tests.rs` to `super::` private methods | Tests inherit `super::*` access via `use super::*;` at the top of `tests.rs`. Same module tree as before. |

## 8. Definition of Done (v0.5.0)

All MUST be true to tag v0.5.0:

- [ ] `src/app.rs` split into mod.rs + input.rs + forms.rs + filter.rs + tests.rs
- [ ] No file in `src/app/` exceeds ~280 non-comment lines
- [ ] One v0.5 feature shipped from §6
- [ ] `cargo test --release` — all green (target ≥ 195 tests)
- [ ] R-G1..R-G9 clean
- [ ] `cargo clippy --all-targets -- -D warnings` 0 warnings
- [ ] `cargo fmt --check` clean
- [ ] Manual checklists (§3 v0.2 + §6 v0.3 + §8 v0.4 from `docs/TESTING.md`) still pass
- [ ] CHANGELOG.md v0.5.0 entry
- [ ] README.md updated if the new feature affects user surface
- [ ] `Cargo.toml` version 0.5.0
- [ ] `git tag v0.5.0` pushed
- [ ] cargo-dist publish-homebrew-formula succeeds (HOMEBREW_TAP_TOKEN is set since v0.4.2)

## 9. Out of scope

- Inline mode CRUD/probes (v0.6+).
- shell completion (zsh/bash `sshc <Tab>` — v0.6+).
- Windows support.
- Migrating off cargo-dist.
- Crates.io publication (`cargo install sshc` from crates.io requires a name reservation + sustained dependency on the published version).

## End of Brief.
