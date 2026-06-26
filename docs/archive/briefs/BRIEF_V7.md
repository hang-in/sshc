# sshc v0.7.0 — Architect Brief

> Companion docs:
> - `PLAN_V0.7.md` — round breakdown
> - `BRIEF_V6.md` §9.2 anti-features — carry over verbatim
> - Previous BRIEF: `BRIEF_V6.md`

## 1. Context

v0.6.0 (2026-05-20) landed favorites / recent history / preview panel /
`ssh -G` validation / modal inline picker. Solid daily-use cycle for
macOS and Linux.

The project's origin story (per maintainer 2026-05-20) is itself a
Windows-environment frustration: tabby was too heavy, hand-edited
`~/.ssh/config` got unwieldy, sshc was the answer — but built on macOS,
shipped Unix-only. v0.7 closes that loop: **native Windows support**.

## 2. v0.7 Goal

Make `sshc` build and run natively on Windows 10 (1809+) and Windows 11
without WSL. Existing macOS / Linux behavior unchanged.

Out-of-scope deferrals to v0.8+:
- Windows-specific features (Pageant integration, Windows OpenSSH agent
  socket discovery). v0.7 just stops *breaking* on Windows; ssh-agent
  integration stays at "use whatever your environment provides".

Anti-features (`BRIEF_V6.md §9.2 / BRIEF_V5.md §9.2`) carry over
unchanged. v0.7 does not relax any of the five anti-features.

## 3. Compatibility matrix

| OS | Build | Run | Notes |
|---|---|---|---|
| macOS (Intel + Apple Silicon) | cargo-dist already shipping | unchanged | reference target |
| Linux x86_64 / aarch64 | cargo-dist already shipping | unchanged | |
| Windows 10 1809+ / 11 (x86_64) | **new — v0.7** | **new** | depends on built-in OpenSSH (or PATH-installed OpenSSH) |
| Windows arm64 | maybe — v0.7+ | maybe | cargo-dist target, but no test runner yet |
| WSL2 | works today via Linux build | works today | README mentions as fallback; v0.7 also recommends native |

## 4. Cross-platform decision points

### 4.1 File permissions

Unix code uses `std::os::unix::fs::PermissionsExt::mode(0o600/0o700)`
in:
- `src/state/mod.rs::save_to` (0600 on state.toml)
- `src/storage/` (0600 on sshc.conf — verify via grep)
- `src/setup/` (0700 on ~/.ssh, 0600 on ~/.ssh/config)
- `src/doctor.rs` (read-only check on ~/.ssh)

**v0.7 policy**: wrap all `mode()` calls in `#[cfg(unix)]`. Windows
gets a no-op. The doctor on Windows reports `~/.ssh permissions` as
`PASS (Windows ACLs not checked)` — explicit message, not silent skip.

Rationale: Windows ACLs are real but enforcement model (inheritance,
deny-rules) is entirely different. Replicating Unix `mode == 0700`
intent on Windows is a project of its own. Out of scope.

### 4.2 File locking (flock)

Current: `src/storage/with_locked_write` uses `nix::fcntl::flock` with
`LOCK_EX | LOCK_NB`. Returns `LockHeldByOther` on contention.

**v0.7 policy**: cfg-split at the lock site.

```rust
#[cfg(unix)]
fn try_lock(file: &File) -> Result<(), StorageError> { /* nix flock */ }

#[cfg(windows)]
fn try_lock(file: &File) -> Result<(), StorageError> {
    use windows_sys::Win32::Storage::FileSystem::{
        LockFileEx, LOCKFILE_EXCLUSIVE_LOCK, LOCKFILE_FAIL_IMMEDIATELY,
    };
    // …
}
```

Use `windows-sys` (Microsoft-maintained, leaner than `winapi`).
Surface error mapping: `ERROR_LOCK_VIOLATION (33)` → `LockHeldByOther`.

### 4.3 `$EDITOR`

`src/exec/editor.rs` reads `EDITOR` env var. Windows users rarely set
it.

**v0.7 policy**: on Windows, fall back to `notepad.exe` if `EDITOR` is
unset. Document in README.

### 4.4 `SSH_AUTH_SOCK` (doctor)

Doctor check is meaningless on Windows — agent forwarding there uses
named pipes (Windows OpenSSH agent) or Pageant.

**v0.7 policy**: on Windows, the doctor line becomes
`SSH_AUTH_SOCK   WARN  not applicable on Windows; use Windows OpenSSH agent or Pageant`.
Status remains `WARN` (informational, doesn't fail the run).

### 4.5 Terminal raw mode

crossterm is already cross-platform. Nothing to do.

### 4.6 Path separators

`dirs::home_dir()` returns `%USERPROFILE%` on Windows, and Rust's
`Path::join` handles the separator. Validate that doctor + setup
output show backslashes correctly in user-facing strings.

### 4.7 cargo-dist target

Add `x86_64-pc-windows-msvc` to `dist-workspace.toml`. ARM64 deferred
until there's a way to actually exercise the binary.

## 5. Module-boundary R-G gates (additions)

No new R-G gates. Existing ones still hold — and `#[cfg(unix)]` blocks
don't affect the greps. After R5, re-run the full matrix from
`docs/TESTING.md §2`.

## 6. New dependencies

| Crate | Purpose | Target | Version |
|---|---|---|---|
| `windows-sys` | `LockFileEx` | `cfg(windows)` only | `0.59` (current stable) |

`nix` keeps `cfg(unix)` only — moved from unconditional to
target-gated in Cargo.toml.

## 7. Risks

| Risk | Mitigation |
|---|---|
| Windows path-case-insensitivity breaks alias comparisons | Aliases are SSH-config strings, OpenSSH itself is case-sensitive on the alias key. No change. |
| `~/.ssh/config.d/` not standard on Windows OpenSSH | Verify: Microsoft's port honors `Include`. Document the path in README's Windows section. |
| LockFileEx semantics ≠ flock (range vs whole file, byte vs advisory) | Lock the whole file (`offset = 0, length = MAX`) — equivalent enough for sshc's single-writer use case. Document in code comment. |
| GitHub Actions Windows runner slow / flaky | First R4 cycle: see if it adds >5 min CI. If so, gate Windows CI on tag push only, not every PR. |
| cargo-dist Homebrew formula generation breaks when Windows artifacts join | Homebrew tap is macOS/Linux-only; cargo-dist already filters per-platform. Verify on first dry-run. |
| Existing Unix users on a future release see Cargo.toml feature-flag warnings | All Windows-specific deps are `target = "cfg(windows)"`; Unix `cargo install` paths see zero new transitive deps. |

## 8. Definition of Done

- [ ] `cargo check --target x86_64-pc-windows-msvc` clean (cross-compile or VM).
- [ ] `cargo test` clean on macOS + Linux (Unix-only tests #[cfg(unix)] gated).
- [ ] `cargo test` clean on Windows for non-permission tests.
- [ ] `cargo clippy --all-targets -- -D warnings` clean both platforms.
- [ ] `cargo fmt --check` clean.
- [ ] R-G1..R-G9 still clean.
- [ ] `cargo-dist` Windows artifact builds + uploads on tag.
- [ ] README + README.ko Windows section rewritten: native first, WSL2 as alternative.
- [ ] CHANGELOG `[0.7.0]` entry.
- [ ] Manual smoke on a real Windows install: `sshc`, `sshc -m`, `sshc --doctor`, `sshc <alias>`. Confirm `f` pin and `v` validate work.
- [ ] cargo-dist publish-homebrew-formula still succeeds (Windows artifact ignored by Homebrew).

## 9. Out of scope

### 9.1 Deferred to v0.8+
- Windows arm64.
- Pageant / Windows OpenSSH agent socket discovery (the doctor stays at "WARN — not applicable").
- Windows ACL enforcement of "private key files must be private".

### 9.2 Project-wide anti-features (carry from BRIEF_V6 §9.2)

1. Self-built SSH client.
2. Secret / key management.
3. Team-shared catalogs.
4. Web UI / daemon.
5. Complete OpenSSH `config(5)` parser.

Unchanged. v0.7 is platform expansion, not feature expansion.

## End of Brief.
