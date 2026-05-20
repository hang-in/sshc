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

## 10. Resolution — v0.8.2 (2026-05-21)

**Shipped.** `a` now actually writes on Windows, verified by the user
on the real `C:\Users\사자\.ssh\config.d\sshc.conf`. Root cause was
**not** what §4 suspected.

### What the §4 dbg! lines actually showed

Running `apply_form(FormContext::AddHost, FormPayload::Host{..})`
end-to-end (via a new `#[test]` rather than the TUI — `apply_form`,
`apply_add`, `persist_sshc_conf` are all reachable from
`src/app/tests.rs`) produced:

```
[DEBUG] apply_form ctx=AddHost payload_variant=Host
[DEBUG] apply_add called for alias="wintest"
[DEBUG]   host.source_file    = "...\sshc.conf"
[DEBUG]   self.sshc_conf_path = Some("...\sshc.conf")   ← byte-equal
[DEBUG]   already exists? false
[DEBUG] persist_sshc_conf path = "...\sshc.conf"
[DEBUG]   path bytes (lossy) = [...]
[DEBUG]   path wide  (WTF-16) = [...]                   ← also equal
[DEBUG]   host="wintest" source_file="...\sshc.conf" eq=true
[DEBUG]   owned_hosts.len() = 1
                                                         ← §4.3 line
                                                           never fired
status_message: "form apply failed: failed to read: ... (os error 33)"
```

§4.1 fine, §4.2 fine, §4.3 **never reached**. `os error 33` =
`ERROR_LOCK_VIOLATION`. Failure was inside `with_locked_write`
*before* the mutator ran.

### Real root cause

`src/storage/writer.rs::with_locked_write` did:

```rust
let file = OpenOptions::new()...open(path)?;
try_lock_exclusive(&file)?;          // LockFileEx, mandatory on Windows
let content = {
    let mut reader = File::open(path)?;   // ← second handle into locked range
    reader.read_to_string(...)?;
    ...
};
```

`LockFileEx` is **mandatory** — even the same process can't open a
second handle into the locked range without tripping
`ERROR_LOCK_VIOLATION`. `flock` is advisory, so Unix silently let the
second open succeed, which is why every release from v0.7 onward
looked fine on macOS/Linux while consistently failing on Windows.

### Why the user saw no error

`apply_form` *did* set `status_message =
"form apply failed: failed to read: ... (os error 33)"`. The modal
close redraw then fired immediately and overwrote the status bar
before the user could see it. So "no error message surfaces" was a
display-layer artifact, not a missing error path. Fixing the
underlying write makes the symptom moot, but worth knowing for any
future class of IO error in this flow — if it happens again the
status_message will be set and then silently disappear.

### Why §4's NFC/NFD theory was wrong

`path wide (WTF-16)` showed `사 = U+C0AC` and `자 = U+C790` as
single code units — i.e. **already NFC**. Both sides of the
comparison used the same `Option<PathBuf>` cache (v0.8.1's
unification), so there was no second-recompute opportunity for a
mismatch anyway. v0.8.1's path-cache unification was internally
correct as defense-in-depth but unrelated to the bug.

### The fix

`writer.rs` now reads from the already-locked handle via
`seek(SeekFrom::Start(0)) + read_to_string`. One handle, one lock,
no violation. v0.8.1's path-cache unification is left in place
unchanged. Added
`app::tests::test_apply_form_add_host_writes_through_locked_writer`
as a cross-platform regression guard that drives `apply_form` against
a temp path — Windows would have caught this within seconds had the
test existed in v0.7.

### Lessons for the next handoff

1. **Don't trust the previous changelog's diagnosis.** v0.8.1's
   NFC/NFD story was plausible and even seemed consistent with the
   Korean home-dir clue, but the WTF-16 print made it falsifiable in
   one test run. Print **bytes** when chasing a path-equality bug,
   not `Debug`-formatted paths.
2. **Status-bar messages are easy to miss on modal close.** When the
   bug report says "no error", verify by *setting a known
   status_message* before submitting and seeing whether it survives.
   In this case the error was always being set; the UI just ate it.
3. **`apply_form` is reachable from `src/app/tests.rs`** — all the
   suspect functions are `pub(super)`/`fn`, and `sshc_conf_path` can
   be overridden post-construction. You don't need to drive the TUI
   to reproduce the save path; an `#[test]` with `assert_fs::TempDir`
   is sufficient for everything below the modal/key-input layer.
4. **`LockFileEx` is mandatory on Windows.** Any future "lock then
   reopen the same path" pattern in this codebase will fail the same
   way. The fix in `writer.rs` is the local cure; the systemic
   principle is "lock the handle you'll read/write through, don't
   re-open by path while holding a lock."

## 11. Residual follow-ups (out of scope for v0.8.2, for the next round)

Two items the v0.8.2 verification run surfaced. Neither is in the
`a` save path — they're separate issues that just happened to be
adjacent. Safe for the macOS-side Opus session to pick up; both
fixes are platform-portable code that the Windows side only needs
to verify, not write.

### 11.1 `sshc.conf` ACL on Windows — user-visible

**Symptom**: after sshc writes `~/.ssh/config.d/sshc.conf` on
Windows, running `ssh -G <alias>` (or `sshc -v` which shells out to
the same path) errors with:

```
Bad owner or permissions on C:\Users\<user>\.ssh\config.d\sshc.conf
```

Windows OpenSSH enforces a strict ACL check on `Include`d files
(owner = user, no group/world read). sshc's
`src/storage/writer.rs::set_owner_only_perms` is currently a no-op
on `#[cfg(not(unix))]` (see line ~132 in v0.8.2), so newly written
sshc.conf inherits whatever ACL its parent directory has — usually
broader than what `ssh.exe` will accept.

**Reproduction (Windows, after v0.8.2)**:

```powershell
ni $HOME\.ssh\config.d\sshc.conf -Force
sshc -m         # 'a', fill in, Enter, q
ssh -G wintest  # → "Bad owner or permissions on ..."
```

**Fix sketch (do on macOS-side Opus, verify on Windows side)**:
- Replace the Windows `set_owner_only_perms` no-op with a real
  implementation: set the file owner to the current user and DACL
  to `SYSTEM:F + Administrators:F + <user>:F`, no inherited entries.
- The Win32 path is `SetNamedSecurityInfoW` + `SetEntriesInAclW`
  from `windows-sys` (`Win32_Security`, `Win32_Security_Authorization`).
  The package already has `Win32_Foundation` and
  `Win32_Storage_FileSystem` in `Cargo.toml`; add the two security
  features as needed.
- Existing test `test_writer_atomic_roundtrip` is `#[cfg(unix)]`;
  add a `#[cfg(windows)]` companion that creates a file via
  `with_locked_write` and asserts the resulting ACL contains exactly
  the three expected ACEs (or at minimum: BUILTIN\Users / Everyone
  is absent).
- Verify on Windows side: `icacls $HOME\.ssh\config.d\sshc.conf`
  shows only SYSTEM / Administrators / current user. `ssh -G` no
  longer errors.

**Why not in v0.8.2**: handoff §6 explicitly forbade touching
`src/storage/writer.rs` semantics beyond the immediate
lock-after-open fix; this is a separate change with its own
test/verify cycle. Also requires Windows-side verification that
this Opus session can't perform end-to-end.

### 11.2 `test_editor_command_construction` flaky on Windows env

**Symptom**: `exec::editor::tests::test_editor_command_construction`
asserts `args.contains(&"+42".to_string())`. Passes only when
`$EDITOR` is set to vim/nvim/nano/nano-tiny (see
`src/exec/editor.rs::is_vim_like`, ~line 41). On a Windows host
without `$EDITOR` set, the test resolves to `notepad.exe` and the
`+LINE` argument convention doesn't apply, so the assertion fails.

**Fix sketch** (purely test-side, no production code change):
- In `src/exec/editor.rs::tests`, set `EDITOR=vim` (or any vim-like
  shim) at the top of `test_editor_command_construction` *before*
  calling `build_editor_command`, and restore on test exit. Mirror
  the pattern of `test_editor_fallback_to_platform_default` (~line
  64), which already manipulates `EDITOR`.
- Or split the assertion: only check `+42` when the resolved editor
  is vim-like; otherwise just assert the file path is present in
  the args.

**Why not in v0.8.2**: handoff §6 forbade touching `src/exec/*.rs`.
Not user-visible either way — it's only a CI/local-test annoyance.

## End of handoff.
