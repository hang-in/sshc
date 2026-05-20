# Windows manage-mode `a` debug handoff

> **Date**: 2026-05-21
> **Branch**: master (commit `981dcbf` v0.8.1, but the bug is unfixed)
> **Goal**: figure out why `sshc -m` → `a` → fill form → Enter still
> leaves `sshc.conf` empty on Windows and ship a fix.

## 1. What you are debugging — and ONLY this

A single failure: on Windows, pressing `a` in `sshc -m`, filling the
form, pressing Enter, the form closes cleanly, **no error message
surfaces, and `~/.ssh/config.d/sshc.conf` ends up empty**. The new
host never appears in the list either.

This is a silent data loss in the add-host save path. Everything
else — connecting, validating, doctor, the `M` promote you wired in
v0.8 G2, the GitHub update check, Windows agent pipe detection —
**works** and is out of scope for this handoff.

The fix you ship from Windows will be v0.8.2 (or v0.8.x — increment
by one patch, see §7).

## 2. Reproduce on Windows

```powershell
# 1. Pull the latest master.
git pull --ff-only

# 2. Build a debug binary; release is fine too but debug surfaces
#    asserts and lets `dbg!()` / `eprintln!()` show through.
cargo build

# 3. Make a clean sshc.conf so any difference is obvious.
ni $HOME\.ssh\config.d\sshc.conf -Force

# 4. Run sshc.
.\target\debug\sshc.exe -m

# 5. Press `a`, fill alias = "wintest", hostname = "1.2.3.4",
#    leave the rest at defaults, IdentityFile via ↑/↓ if you want
#    to mirror your real case. Press Enter.

# 6. Form closes. Inspect:
gc $HOME\.ssh\config.d\sshc.conf
ls $HOME\.ssh\config.d\
```

Expected: sshc.conf contains a `Host wintest` block.
Actual: empty file, no orphan tmp.

## 3. What's already been tried — DO NOT redo these

These were all "fixes" that closed *some* failure mode but did not
make `a` write to disk on your machine. Don't re-derive them; build
on top.

| Version | Diagnosis (at the time)                                      | Why it didn't fix you                                                                                                       |
| ------- | ------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------- |
| v0.7.2  | IdentityFile validator rejected `\` on every platform.       | Real bug, real fix — but it only blocked the form from submitting. Once submission worked, the silent failure took over.    |
| v0.7.3  | `storage::with_locked_write` held the file handle open through `fs::rename`, MoveFileW returned ERROR_SHARING_VIOLATION. | Real bug too, but on macOS only; on Windows the rename path now succeeds *and writes empty content* — the saving step itself is wrong.|
| v0.8.1  | `persist_sshc_conf` recomputed `sshc_conf_path()`; suspected PathBuf normalization mismatch (NFC vs NFD on `C:\Users\사자`). | The recompute is gone; both filter and write target now use the App::new-cached `self.sshc_conf_path`. You report this still doesn't fix it. So the cache itself, OR the host's `source_file` set at form-build time, must hold a value that mismatches the cache. |

## 4. The most likely remaining causes — and the dbg! you need

The cached `self.sshc_conf_path` is set in `App::new` from
`storage::sshc_conf_path()` which is just
`dirs::home_dir().map(|h| h.join(".ssh").join("config.d").join("sshc.conf"))`.

`build_host` (`src/app/forms.rs`) sets `source_file =
self.sshc_conf_path_or_blank()` which clones from the same cache.

So `host.source_file` and the path used in `persist_sshc_conf` are
the **same `PathBuf` clone**. They should be byte-equal.

But on your machine they aren't, OR the host never reaches
`persist_sshc_conf` at all. There are exactly three suspect spots —
hit each with a `dbg!` and you'll see which one is lying.

### 4.1 Did `apply_add` even run?

In `src/app/forms.rs` at the top of `apply_add`:

```rust
fn apply_add(&mut self, host: Host) -> Result<(), AppError> {
    eprintln!("[DEBUG] apply_add called for alias={:?}", host.alias);
    eprintln!("[DEBUG]   host.source_file = {:?}", host.source_file);
    eprintln!("[DEBUG]   self.sshc_conf_path = {:?}", self.sshc_conf_path);
    eprintln!(
        "[DEBUG]   already exists? {}",
        self.hosts.iter().any(|h| h.alias == host.alias)
    );
    if self.hosts.iter().any(|h| h.alias == host.alias) {
        // ... existing duplicate-alias branch ...
    }
    // ... rest unchanged ...
}
```

Run sshc inside a shell that lets you see stderr:

```powershell
.\target\debug\sshc.exe -m 2> sshc.log
# do the repro, then:
gc sshc.log
```

If you don't see "apply_add called for alias=…", the form's payload
isn't being routed to `apply_add` at all — look at `apply_form`'s
match arms in the same file (the catch-all `_ => Ok(())` is the
silent-success suspect).

If you DO see it, move to §4.2.

### 4.2 Does the filter drop the host?

In `src/app/forms.rs::persist_sshc_conf`, before the `with_locked_write`
call:

```rust
fn persist_sshc_conf(&self) -> Result<(), AppError> {
    let path = self
        .sshc_conf_path
        .clone()
        .ok_or(AppError::Setup(SetupError::HomeDirMissing))?;
    eprintln!("[DEBUG] persist_sshc_conf path = {:?}", path);
    eprintln!(
        "[DEBUG]   path bytes = {:?}",
        path.as_os_str().to_string_lossy().as_bytes()
    );
    for h in &self.hosts {
        let eq = h.source_file == path;
        eprintln!(
            "[DEBUG]   host={:?} source_file={:?} eq={}",
            h.alias, h.source_file, eq
        );
    }
    let owned_hosts: Vec<Host> = self
        .hosts
        .iter()
        .filter(|h| h.source_file == path)
        .cloned()
        .collect();
    eprintln!("[DEBUG]   owned_hosts.len() = {}", owned_hosts.len());
    // ... unchanged ...
}
```

If `eq=false` for the just-added host, the cache itself is somehow
not matching the host's `source_file`. Print the raw bytes (the
`.as_bytes()` line above) — if they differ, you have your culprit.
On Windows you can also dump the WTF-16 representation:

```rust
use std::os::windows::ffi::OsStrExt;
let wide: Vec<u16> = path.as_os_str().encode_wide().collect();
eprintln!("[DEBUG]   path wide = {:?}", wide);
```

### 4.3 Is `with_locked_write` writing empty content?

In `src/storage/writer.rs` after the mutator runs:

```rust
let new_content = mutator(&content);
eprintln!(
    "[DEBUG] with_locked_write: writing {} bytes to {:?}",
    new_content.len(),
    path
);
```

If you see `writing 0 bytes`, the mutator (filter + serializer)
returned an empty string — the host was dropped in §4.2 or the
serializer itself is dropping it.

If you see a sensible size (a few hundred bytes), the write itself
is failing or being undone. Look at the `fs::rename` step and check
for tmp orphans after the run:

```powershell
ls $HOME\.ssh\config.d\
# any sshc.conf.tmp.* present? if yes, rename failed silently.
```

## 5. Files you MAY edit

Stay within these:

- `src/app/forms.rs` — `apply_add`, `apply_form`, `build_host`,
  `persist_sshc_conf`. This is where the bug lives.
- `src/app/mod.rs` — only if you discover the cache itself
  (`App::new` initialization of `sshc_conf_path`) is wrong.
- `src/storage/path.rs` — only if `sshc_conf_path()` /
  `dirs::home_dir()` is the source of disagreement.
- `src/storage/serializer.rs` — only if you trace the empty-content
  problem to `host_blocks_to_text`.
- `src/storage/writer.rs` — only to add diagnostics or to fix a
  Windows-specific write failure. **Do NOT change the
  `drop(file)` → `fs::rename(...)` ordering**: that's the v0.7.3
  fix and reverting it brings ERROR_SHARING_VIOLATION back.
- `Cargo.toml` — only to bump the version when shipping.
- `CHANGELOG.md` — add a `[0.8.x]` block when shipping.

## 6. Files you MUST NOT edit (out of scope for this debug round)

These are stable and unrelated to the add-host save path. Touching
them widens the change set and risks regressions outside the bug.

- `src/doctor.rs` — v0.8 G1 (Windows agent pipe probe) + G3
  (GitHub update check) are settled.
- `src/app/input.rs` — manage-mode keymap is settled (no new keys).
- `src/ui/forms/host_form.rs` — the form widget itself is fine; the
  bug is on the *save* side, not the *render/validate* side. The
  v0.7.2 backslash split + v0.7.1 one-row-per-field layout +
  IdentityFile picker are all known good. Don't add `std::fs` here
  (R-G8).
- `src/ui/list.rs`, `src/ui/preview.rs`, `src/ui/modal.rs` — UI
  surface, not save-path.
- `src/inline_app.rs` — inline mode is read-only by design (R-G9);
  it can't write sshc.conf anyway.
- `src/exec/*.rs` — connection and `ssh -G` validation surfaces,
  not save-path.
- `.github/workflows/release.yml` — has a `concurrency` block we
  hand-edited; if you ever run `dist init` / `dist generate` to
  regenerate, you must re-add it. Better: don't touch the workflow.
- `dist-workspace.toml` — `allow-dirty = ["ci"]` is intentional.
- Any `BRIEF_V*.md` / `PLAN_V*.md` — those are gitignored anyway.

## 7. Validation before you commit

Before pushing your fix:

```powershell
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --release

# R-G grep matrix (PASS check — adapt to PowerShell if needed)
# (docs/TESTING.md §2 — paste the 9 greps in a script and confirm PASS x9.)

# Actual repro:
ni $HOME\.ssh\config.d\sshc.conf -Force
.\target\release\sshc.exe -m
# press a, fill in a new host, Enter.
gc $HOME\.ssh\config.d\sshc.conf
# Expected: a Host block, not empty.
```

Once the repro passes, REMOVE all `eprintln!("[DEBUG] ...)` lines you
added — those are diagnostic crutches, not shippable code. Keep only
the real fix.

## 8. Shipping

1. Bump `Cargo.toml` version: `0.8.1` → `0.8.2`.
2. Add a `[0.8.2] — 2026-05-21` block to `CHANGELOG.md` describing
   the actual root cause you found (not a recap of v0.8.1).
3. Bump `cargo install --tag` line in `README.md` + `README.ko.md`
   to `v0.8.2`.
4. Commit message format (see PLAN_V0.8 §7) — keep it under 72-col
   subject line, body explains the *why* including what made v0.8.1
   miss the mark.
5. `git tag -a v0.8.2 -m "..."` and `git push origin master &&
   git push origin v0.8.2`.
6. The cargo-dist Release workflow should fire exactly once
   (concurrency guard). Watch with `gh run list --limit 3`. If a
   `dist plan` error mentions "out of date contents", it means the
   workflow file diverged from the template again — see
   `.github/workflows/release.yml` MANUAL EDIT note for context.

## 9. If you genuinely get stuck

Stop and write what you learned to this file (append a §10), then
hand it back. Don't ship a "maybe-fix" that disables the form
collision check or silently catches errors — the previous handoff
cycle (v0.7.2, v0.7.3, v0.8.1) each shipped a confident "this is
the fix" that closed only the *visible* layer of the failure. We
have to actually see the host land in `sshc.conf` from a real
Windows session this time.

## End of handoff.
