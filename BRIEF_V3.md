# sshs v0.3.0 — Architect Brief (v3)

> Revision of the user-supplied v0.3 draft after architect review.
> Status: **implementation-ready spec**.
>
> Implementer (glm-5.1:cloud via tunaLlama) treats sections marked
> **CONTRACT** as binding. Sections marked **NOTE** are advisory.
> Reviewer of all implementation deliverables: this Claude session.

---

## 0. Revision summary (vs user draft)

| Change | Draft | This brief |
|---|---|---|
| Include direction | "prepend wins logically" | **append** (main wins; first-match-wins semantics corrected — §5 Q2) |
| modify key | `m` (decided) | confirmed `m` |
| Tags display | "column or inline" | **inline prefix** next to alias (e.g. `web [prod,api]`) |
| TCP probe semantic | "1-char indicator" | `●` = TCP-open, NOT "login ready"; legend in Help (§5 Q5 + §10) |
| Form UX | "form fields listed" | Tab/Shift-Tab/Enter/Esc state machine + validation rules (§6.6) |
| Modal component | "confirmation modal" | generalized base for confirmation / info / form (§6.5) |
| Empty-state | "UI when no hosts" | concrete message + which keys active (§3 row 1) |
| v0.2 transitional cleanup | (not mentioned) | **R0 / T0 cleanup task** before any v0.3 module lands (§13) |
| Column responsiveness | "Below minimum?" | priority order + graceful degrade (§5 Q6) |
| probe ↔ connect | (not mentioned) | successful connect bumps probe state to ●  (§3 row 3) |
| Migration | (not mentioned) | v0.2 users hit first-run flow on upgrade (§7) |
| CHANGELOG | (not mentioned) | start CHANGELOG.md with v0.3 entry (§13) |
| Module boundary gates | (not mentioned) | v0.3 extends BRIEF_V2 §9 grep matrix (§9) |
| ProxyJump compat | (not mentioned) | TCP probe ignores ProxyJump — Help notes it (§5 Q5) |

---

## 1. Context (recap)

sshs v0.2.0 ships:
- ratatui + crossterm
- Hand-rolled SSH config parser (cycle-protected Include, line tracking)
- `Host::fuzzy_score` via nucleo, `Match` directive isolated, quoted/inline-comment values
- spawn+wait `ssh_run` with `SshResult` classification
- `TerminalGuard` RAII + idempotent panic hook
- `App::last_connected`, `★` marker, transient status bar (3s)
- `r` reconnect, mock_ssh integration tests
- 73 automated tests, 5 regression-grep gates clean, anyhow eliminated

v0.3 transforms sshs from launcher into manager.

## 2. Goal (one sentence)

Make sshs a host **manager**: users add / edit / delete hosts via TUI
forms, tag them, see live TCP connectivity — with a single safely-managed
write file (`~/.ssh/config.d/sshs.conf`) and a one-time Include injection
into `~/.ssh/config`.

## 3. In Scope (clarified)

| # | Item | Clarification |
|---|---|---|
| 1 | First-run setup | Detect `~/.ssh/config` + `~/.ssh/config.d/`; create `config.d/` (0700) and `config.d/sshs.conf` (0600) if missing; if `~/.ssh/config` lacks Include for sshs.conf, prompt via modal once. Yes → **append** Include line to end of main config (see §5 Q2); No → record `declined_include_injection = true` in state.toml. **Empty-state UI**: when total host count == 0, render a centered "No hosts. Press `a` to add or `e` to open $EDITOR." Only `a`, `e`, `q` keys active. |
| 2 | Layout redesign | **5-column table**: Alias \| Account \| Host \| Port \| Status. Tags rendered as **inline `[t1,t2]` prefix on the Alias cell** (e.g. `[prod,api] web-1`). Column widths via ratatui `Constraint::Length` for Status/Port, `Constraint::Min` for Alias/Host. Account uses `Constraint::Length(12)` (truncate longer). Layout is **full-screen with 1-cell border** (no more centered_rect). Column priority for narrow terminals: Status > Alias > Host > Port > Account; columns below priority threshold are hidden left-to-right. |
| 3 | Connectivity indicator | TCP probe per host with 2s timeout. States: `●` (TCP open) / `✕` (refused/timeout) / `?` (in-flight) / `○` (not yet probed). Color: green / red / yellow / dim grey. Refresh every **30s**. Successful `ssh_run` (SshResult::Success) bumps that host's probe state to `●` immediately (no wait for next 30s). **Semantic note in Help**: `●` means TCP-open, not "login ready"; ProxyJump-only hosts will show `✕` even if reachable indirectly. |
| 4 | Host CRUD via TUI forms | `a` = add, `m` = modify, `d` = delete. All only operate on sshs.conf-owned hosts. External hosts: keys are no-ops; status bar shows `"Host '{alias}' defined in {path} — press 'e' to edit"` for ~3s. Form state machine in §6.6. Delete uses confirmation modal. |
| 5 | Tags | Parser recognizes `# @tags: foo, bar` comment **immediately before** a `Host` line (no blank lines between). Tag values lower-cased + trimmed during parse. TUI shows tags inline (item 2). `/` filter syntax: bare query matches alias/hostname/tag substring; `@tag` query filters only by tag. `t` key opens single-field form to edit tags on sshs.conf-owned hosts. |
| 6 | Source tracking | `Host::source_file` already populated in v0.2. UI: rows from non-sshs.conf files render with `Modifier::DIM` and bear a marker `· ` in the leading status column. Edit/Delete attempts on dim rows: rejected with status bar message. |

---

## 4. Out of Scope (defer to v0.4+)

- Connection history / per-host access count persistence
- `Match` directive evaluation (only isolation, v0.1 already done)
- ProxyJump / LocalForward / Advanced options
- Beyond-Include multi-config-file management
- Cross-platform (Unix only continues)
- i18n of form labels and error messages (English only in v0.3)
- ssh-agent / key management
- Connection profiles / aliases beyond ssh's own Host blocks

## 5. Risk-Area Decisions (architect answers)

### Q1 — Write safety for sshs.conf

**CONTRACT**:
- **flock**: acquire LOCK_EX on the sshs.conf fd before any read-modify-write; release on Drop. Single-instance lock; if another sshs holds it, refuse the write with status bar `"sshs.conf locked by another instance"`. Implementation: `nix::fcntl::flock` (new dep) or raw libc.
- **Atomic write**: write to `sshs.conf.tmp.<pid>` in the same directory, fsync, then `rename` to `sshs.conf`. Rename is atomic on same filesystem.
- **Backup**: keep single `sshs.conf.bak`, overwritten on each write. (No versioned rotation; rely on git/Time Machine for history.)
- **Stale-temp cleanup**: on startup, remove `sshs.conf.tmp.*` older than 1 hour in the config.d/ directory.
- **Failure surfacing**: write error → `StatusMessage::new("Failed to save: {e}")`. No modal — error message in status bar is enough for transient I/O issues.
- **Recovery**: if rename fails after tempfile write, return Err. Caller (App) keeps in-memory state; user can retry. tempfile is not deleted (so user can manually salvage).

### Q2 — Main config Include injection

**CONTRACT**:
- Direction: **append** (NOT prepend). SSH config is first-match-wins, so appending preserves user-defined main-config hosts as authoritative when aliases collide.
- Method: read main config, scan for any line matching `Include\s+\S+` whose resolved path canonicalizes to `~/.ssh/config.d/sshs.conf`. If found → no-op. Else: append a 2-line block at end of file:
  ```
  # Added by sshs; do not remove.
  Include ~/.ssh/config.d/sshs.conf
  ```
- Idempotency: canonicalize Include paths via `dirs::home_dir()` expansion + `Path::canonicalize`. Compare to canonicalized sshs.conf path. Regex match alone is insufficient (handles `~`, relative paths, symlinks).
- Backup: before write, copy main config to `~/.ssh/config.bak.sshs-YYYYMMDD`. Single backup per day (overwrite if exists).
- `parser.rs` line_start impact: append at EOF does NOT shift existing line numbers — safe.
- `$XDG_CONFIG_HOME`: SSH config location is fixed at `~/.ssh/config`; state.toml uses XDG.
- **Collision detection**: when user adds via TUI a host whose alias exists in any other parsed file, modal warns `"Alias '{alias}' already exists in {path}. Main config will take precedence. Continue?"` with Yes/No. (Append + first-match-wins means sshs's new entry is shadowed by the existing one.)

### Q3 — Comment-based tag parsing

**CONTRACT**:
- Only the comment line immediately above `Host` (no blank line between) counts as the tag line.
- Tag line format: `# @tags: tag1, tag2, tag3`. Whitespace-tolerant. `# @tags:` (empty list) → empty tags vec.
- Tags normalized to lowercase, trimmed, deduplicated on parse.
- Multiple consecutive comments above `Host`: only the bottom-most one is the tag candidate; if it matches `# @tags:` syntax, it's the tag line. Earlier comments are preserved on rewrite only as part of the in-memory representation if and only if the block is in sshs.conf (see below).
- **Write strategy**: sshs.conf-owned `Host` blocks are completely rewritten by sshs on every write. The block is regenerated as:
  ```
  # @tags: t1, t2
  Host alias
      HostName ...
      User ...
      Port ...
      IdentityFile ...
  ```
- This means **user-added comments inside sshs-managed blocks are NOT preserved**. sshs.conf header banner: `# Managed by sshs. Manual edits inside Host blocks may be overwritten on next save.`
- External files (main config, other Include targets) are never rewritten — manual comments preserved trivially.
- Manual user edits with non-`# @tags:` syntax in sshs.conf: ignored on parse. Next sshs save overwrites them.

### Q4 — Source-aware editing

**CONTRACT**:
- UI: external-file rows rendered with `Modifier::DIM` and a `· ` marker in the Status column (visually distinct from `●/✕/?/○`).
- Keys `a` (add) always allowed (creates in sshs.conf regardless of selection).
- Keys `m` (modify), `d` (delete), `t` (tags) on external rows: status bar message
  `"Host '{alias}' defined in {path} — press 'e' to edit"` for 3s.
- Add collision: when committing form, scan all hosts; if same alias exists anywhere, show confirmation modal (§5 Q2). User can rename or proceed with override-warning.

### Q5 — Probe scaling and cleanup

**CONTRACT**:
- **Thread pool**: fixed-size workers, `min(8, host_count)`. Implementation: bounded mpsc queue of probe jobs; N worker threads dequeue and execute. Results pushed to App via separate `Sender<ProbeUpdate>`.
- Per-probe timeout: 2s via `TcpStream::connect_timeout`.
- **Generation counter**: when host list changes (add/delete/replace_hosts), increment a global `AtomicU64`. Each probe carries its generation; results with stale generation are discarded by App's recv loop.
- **Shutdown**: on quit, App drops its `Sender<JobRequest>` and waits up to 500ms for workers to drain. After timeout, threads are detached (OS reclaims). No `Sender::send` panic risk because workers handle `recv` Err = "shutting down".
- **No persistence**: probe state is `Vec<ProbeState>` indexed parallel to `hosts`. Cleared on `replace_hosts`.
- **TCP-only semantic**: `●` = TCP three-way handshake succeeded to (HostName, Port). Does NOT verify SSH banner, auth, or ProxyJump reachability. `?` Help line explicitly states this.
- **ProxyJump-only hosts**: probe will likely show `✕` (direct TCP fails). User must understand `✕` ≠ "host broken". Acceptable trade-off — probe is a hint, not gospel.
- **Successful ssh_run bumps to `●`**: in `App::on_ssh_finished(_, SshResult::Success)`, also update the corresponding entry in the probe state vec to `Open`. Stale generation race avoided because App holds the canonical state vec.

### Q6 — Layout responsiveness

**CONTRACT**:
- Minimum supported: **80x24** (full functionality). Below 80 wide: hide columns in priority order Account → Port → Host (always keep Status, Alias). Below 24 tall: still render; just less of the list visible. Below ~60x10: render "terminal too small" message.
- **Full-screen**: dropping centered_rect. `f.area()` directly used. 1-cell border for visual breathing room.
- Column constraints (ratatui):
  ```
  Status:  Constraint::Length(2)    // "● " or "· "
  Alias:   Constraint::Min(12)
  Account: Constraint::Length(12)   // hidden first when narrow
  Host:    Constraint::Min(15)
  Port:    Constraint::Length(6)
  ```
- Dynamic recomputation each draw call (cheap for ≤ 500 rows).
- Probe glyph Unicode fallback: if `LANG` lacks UTF-8 (rare) — out of scope, document; nothing dynamic.

### Q7 — First-run state persistence

**CONTRACT**:
- File: `$XDG_CONFIG_HOME/sshs/state.toml` (fallback `~/.config/sshs/state.toml`).
- Format: TOML via `serde` + `toml` crate.
- Schema:
  ```toml
  version = 1

  [setup]
  include_check_done = false       # true once we've verified / injected
  declined_include_injection = false

  [memory]
  last_connected_alias = ""         # optional cross-session memory
  ```
- Schema versioning: `version` field at top level. On future bump, sshs migrates or refuses to start with a clear message.
- File permission: 0600.
- Created lazily on first state mutation.
- Read on startup. Absent file = first run = trigger setup flow.
- **Migration from v0.2**: state.toml absent for v0.2 users → triggers first-run setup. Users may see "Include line" prompt the first time they launch v0.3. Acceptable UX.

---

## 6. Module Architecture and Public API

### 6.1 Target module tree

```
src/
├── main.rs                        — unchanged from v0.2 (≤80 LOC)
├── lib.rs
├── error.rs                       — extend with StorageError, SetupError, ProbeError
├── app.rs                         — extend: tags, probe state, mode (List/Form/Modal)
├── config/
│   ├── mod.rs
│   ├── model.rs                   — add `tags: Vec<String>` field to Host
│   ├── parser.rs                  — recognize `# @tags:` line
│   └── tags.rs                    — NEW: tag parsing + normalization helpers
├── tui/
│   ├── lifecycle.rs
│   └── runtime.rs                 — extend: pipe probe receiver into loop
├── ui/
│   ├── layout.rs
│   ├── list.rs                    — extend: 5-col table; tag prefix; source marker
│   ├── status_bar.rs
│   ├── modal.rs                   — NEW: generalized confirmation/info/form base
│   └── forms/                     — NEW
│       ├── mod.rs
│       ├── host_form.rs           — add/edit Host form (6 fields)
│       └── tag_form.rs            — single-field tag editor
├── exec/
│   ├── ssh.rs
│   └── editor.rs
├── storage/                       — NEW
│   ├── mod.rs
│   ├── path.rs                    — resolve_sshs_conf_path, resolve_state_path
│   ├── writer.rs                  — flock + atomic write + backup
│   ├── serializer.rs              — host_blocks_to_text (with @tags)
│   └── include_injector.rs        — idempotent append of Include line
├── setup/                         — NEW
│   ├── mod.rs                     — first_run_flow() orchestration
│   ├── detect.rs                  — check files/perms/include presence
│   └── permissions.rs             — perm check + warn
├── probe/                         — NEW
│   ├── mod.rs                     — start_probe_pool() public API
│   ├── worker.rs                  — TCP connect with timeout
│   └── state.rs                   — ProbeState enum + per-host vec
└── state/                         — NEW
    ├── mod.rs                     — load / save state.toml
    └── schema.rs                  — serde structs + version migration
```

### 6.2 `src/error.rs` — extensions

**CONTRACT** (additions, keep existing v0.2 variants):

```rust
#[derive(Debug)]
pub enum StorageError {
    LockFailed(std::io::Error),
    LockHeldByOther,
    ReadFailed(std::io::Error),
    WriteFailed(std::io::Error),
    RenameFailed(std::io::Error),
    BackupFailed(std::io::Error),
    PermissionMismatch { path: std::path::PathBuf, expected: u32, actual: u32 },
}

#[derive(Debug)]
pub enum SetupError {
    HomeDirMissing,
    MkdirFailed(std::io::Error),
    Storage(StorageError),
    StateParseFailed(toml::de::Error),
    StateWriteFailed(StorageError),
}

#[derive(Debug)]
pub enum ProbeError {
    ResolveFailed(std::io::Error),
    ConnectTimeout,
    ConnectRefused(std::io::Error),
}

// Extend AppError to include these via From impls.
```

### 6.3 `src/config/model.rs` — Host extension

**CONTRACT**: add one field, `tags: Vec<String>`. Existing fields unchanged. `Default` impl optional.

```rust
pub struct Host {
    // existing fields ...
    pub tags: Vec<String>,  // normalized: lowercase, trimmed, deduped
}
```

### 6.4 `src/config/tags.rs` — NEW

```rust
/// Parse a "# @tags: a, b, c" line into a Vec<String>.
/// Returns None if the line does not match the @tags pattern.
pub fn parse_tag_line(line: &str) -> Option<Vec<String>>;

/// Render tags back to a "# @tags: a, b" line (empty input → empty string).
pub fn render_tag_line(tags: &[String]) -> String;

/// Normalize a tag: trim + lowercase + reject empty.
pub fn normalize_tag(raw: &str) -> Option<String>;
```

### 6.5 `src/ui/modal.rs` — NEW (generalized)

```rust
pub enum ModalKind {
    Confirmation { prompt: String, on_yes: ModalAction, on_no: ModalAction },
    Info { message: String, dismiss: ModalAction },
    Form(Box<dyn FormState>),
}

pub enum ModalAction {
    None,
    AppAction(crate::app::AppAction),
    Custom(String), // dispatched via App::handle_modal_action
}

pub trait FormState: Send {
    fn render(&self, area: ratatui::layout::Rect, f: &mut ratatui::Frame);
    fn handle_key(&mut self, key: crossterm::event::KeyEvent) -> FormOutcome;
}

pub enum FormOutcome {
    Stay,
    Cancel,
    Submit(/* form-specific payload type */),
}
```

**NOTE**: implementer may choose between trait-objects (above) and an enum
of concrete form types. Simpler enum often outperforms trait-object for
small N. Architect's preference: enum, but glm has latitude.

### 6.6 Form state machine (host_form + tag_form)

**CONTRACT** (applies to both host_form and tag_form):

| Key | Behavior |
|---|---|
| `Tab` | next field; wraps from last to first |
| `Shift+Tab` | prev field; wraps from first to last |
| `Enter` | if on last field OR cursor at end: submit; else next field |
| `Esc` | cancel; discard buffer; close modal |
| `Char(c)` | append to active field buffer |
| `Backspace` | delete last char of active field |
| `Ctrl+U` | clear active field |

**host_form fields** (in order):
1. Alias (required, no whitespace, no `*`/`?`)
2. HostName (required)
3. User (optional)
4. Port (optional, parse as u16; 1-65535 or empty)
5. IdentityFile (optional, path-like)
6. Tags (optional, comma-separated; normalized)

**Validation** runs on Submit:
- Required fields non-empty
- Alias regex `^[A-Za-z0-9._-]+$`
- Port parse OK or empty
- IdentityFile contains no shell-metachar (paranoia)
- Tag count ≤ 16 (sanity)

Failed validation: don't close form; display error at modal bottom.

**tag_form**: single field; Enter submits.

### 6.7 `src/storage/writer.rs`

```rust
/// Acquire LOCK_EX on the sshs.conf, read it, hand the parsed content to
/// `mutator`, write the new content atomically (tempfile + rename), then
/// drop the lock.
///
/// PRE: sshs.conf exists (or will be created if create=true).
/// POST: file content is replaced; permissions normalized to 0600.
pub fn with_locked_write<F>(
    path: &Path,
    create: bool,
    mutator: F,
) -> Result<(), StorageError>
where F: FnOnce(&str) -> String;
```

`with_locked_write` is the single point of truth for sshs.conf
modification. Form-submit handlers in App push a re-rendered content
string into this helper.

### 6.8 `src/storage/include_injector.rs`

```rust
pub enum InjectionResult {
    AlreadyPresent,
    Injected,
    Skipped,            // user declined
}

/// Check whether ~/.ssh/config already includes sshs.conf.
/// PRE: main config path exists. POST: no side effect.
pub fn is_include_present(main_config: &Path, sshs_conf: &Path) -> Result<bool, StorageError>;

/// Append the Include line (with header comment) to main config.
/// Creates a dated .bak before mutating.
pub fn inject_include(main_config: &Path, sshs_conf: &Path) -> Result<(), StorageError>;
```

### 6.9 `src/probe/mod.rs`

```rust
pub struct ProbePool {
    job_tx: Sender<ProbeJob>,
    result_rx: Receiver<ProbeUpdate>,
    generation: Arc<AtomicU64>,
}

pub struct ProbeJob {
    host_idx: usize,
    host: String,        // resolved hostname or alias-as-host
    port: u16,
    generation: u64,
}

pub struct ProbeUpdate {
    host_idx: usize,
    state: ProbeState,
    generation: u64,
}

pub enum ProbeState { Open, Failed, InFlight, Unknown }

impl ProbePool {
    /// Spawn min(8, host_count) workers + a 30s tick thread.
    /// Returns immediately. Pool runs until ProbePool is dropped.
    pub fn start(initial: &[Host]) -> Self;

    /// Submit a fresh round of probes. Increments generation; older
    /// updates are discarded by the consumer (App).
    pub fn refresh(&self, hosts: &[Host]);

    /// Non-blocking poll: drain currently-available updates.
    pub fn poll_updates(&self) -> Vec<ProbeUpdate>;

    /// Current generation; consumer compares to filter stale.
    pub fn current_generation(&self) -> u64;
}

impl Drop for ProbePool {
    /// Drops job_tx, signaling workers to stop. Waits up to 500ms; then
    /// detaches remaining threads.
    fn drop(&mut self);
}
```

### 6.10 `src/setup/mod.rs`

```rust
pub enum SetupOutcome {
    Ready,                          // hosts loaded, ready to use sshs.conf
    AwaitingIncludeChoice,          // need user response via modal
    ReadOnly,                       // user declined; sshs.conf writes disabled
}

/// Run the first-run / startup checks. Returns the outcome that drives
/// the App into either Ready, modal-prompting, or read-only mode.
pub fn run_first_run_checks(state: &mut crate::state::State) -> Result<SetupOutcome, SetupError>;
```

### 6.11 `src/state/mod.rs`

```rust
pub struct State {
    pub version: u32,
    pub setup: SetupSection,
    pub memory: MemorySection,
}

pub struct SetupSection {
    pub include_check_done: bool,
    pub declined_include_injection: bool,
}

pub struct MemorySection {
    pub last_connected_alias: Option<String>,
}

/// Load state from $XDG_CONFIG_HOME/sshs/state.toml or default.
pub fn load() -> Result<State, SetupError>;

/// Atomically save state via storage::with_locked_write.
pub fn save(s: &State) -> Result<(), SetupError>;
```

### 6.12 `src/app.rs` — extensions

**CONTRACT**:
- New field `mode: AppMode { List, Modal(ModalKind) }`. handle_key dispatches differently per mode.
- New field `probe_states: Vec<ProbeState>` parallel-indexed to `hosts`.
- New field `state: crate::state::State` (in-memory mirror of state.toml).
- Filter syntax: `@tag` prefix filters by tag substring; otherwise existing fuzzy logic also matches tags.
- New methods: `enter_form_mode(form)`, `exit_modal()`, `apply_form(submit)`, `handle_modal_key(key)`.
- New AppAction variants: `AddHost`, `EditHost(alias)`, `DeleteHost(alias)`, `EditTags(alias)`, `SaveState`, `InjectInclude`.

**NOTE for architect**: this is a large modification to an existing file.
Per v0.2 retrospective, this work goes to the **architect**, not glm.
Subdivide into surgical patches (one per new field/method).

### 6.13 `src/tui/runtime.rs` — extension

**CONTRACT**:
- run_event_loop pulls probe updates from ProbePool each tick.
- Modal mode short-circuits regular event handling.
- handle_connect already exists; extend post-success path to bump probe state.

---

## 7. Migration from v0.2 to v0.3

- v0.2 users have no `state.toml` → first run treats them as fresh.
- First launch of v0.3: parser still reads `~/.ssh/config` as before, plus any Includes. Then first-run flow detects no sshs.conf, creates `config.d/sshs.conf` (empty + banner) and `config.d/`, then prompts Include injection.
- Existing v0.2 functionality (round-trip, `r`, `★`) continues unchanged. Users opting "No" to Include get a read-only experience equal to v0.2 (just with new layout + probe).
- `App::last_connected_alias` can optionally be hydrated from `state.toml memory` (NICE-TO-HAVE; ship if straightforward).

## 8. Test Strategy (5 layers, extends v0.2's 4)

### 8.1 Unit (no real filesystem)
- `tags::parse_tag_line` / `render_tag_line` / `normalize_tag`
- `include_injector::is_include_present` (path canonicalization)
- `state` load/save round-trip with tempfile
- `probe::worker` direct TCP probe vs a TcpListener bound to 127.0.0.1 (yes filesystem-free)
- `host_form` validation matrix
- App mode transitions (List ↔ Modal)

### 8.2 Integration (existing patterns + new)
- mock_ssh (carry-over from v0.2)
- `storage/writer` round-trip on tempdir: write, lock contention (spawn second flock attempt and assert LockHeldByOther)
- `setup::run_first_run_checks` flow against tempdir filesystem
- Render snapshots of list/form/modal via ratatui's `TestBackend`

### 8.3 Probe live test
- Bind ephemeral TcpListener on 127.0.0.1; probe it; assert ProbeState::Open within timeout.
- Probe to 192.0.2.1 (TEST-NET-1, always unreachable); assert ProbeState::Failed within timeout.

### 8.4 Regression
- All v0.2 73 tests pass unchanged.

### 8.5 Manual checklist (extends docs/TESTING.md §3)
- First-run prompt accept → main config has Include line; sshs.conf with banner
- First-run prompt decline → state.toml has decline flag; subsequent launches skip
- Add host via form → appears in list; sshs.conf written; correct perms
- Modify host → updated; backup .bak exists
- Delete host → confirmation modal; row removed
- External host: `m`/`d` rejected with status message
- Probe states cycle: `?` → `●` or `✕` within 2s after startup
- Connect to host → probe state immediately becomes `●`
- Resize terminal narrower → columns hide in priority order

## 9. Module Responsibility Boundaries (extends BRIEF_V2 §9)

Same v0.2 rules carry over. Additions:

| Module | Owns | Forbidden |
|---|---|---|
| `storage/*` | sshs.conf I/O + flock + atomic write | TUI rendering, App state, exec |
| `setup/*` | first-run orchestration | direct UI, Command spawns |
| `probe/*` | TCP connect probes, worker threads | App state mutation, TUI, exec |
| `state/*` | state.toml load/save | terminal, App state mutation |
| `ui/modal.rs` | modal compositing | App state mutation; FormState submit is the only callback edge |
| `ui/forms/*` | form rendering + per-key state | direct App state mutation; submit returns payload via FormOutcome |

### Regression-grep gates (v0.3 additions)

```bash
# R-G6: storage modules must not touch TUI
grep -lE "use crossterm|use ratatui" src/storage/*.rs src/setup/*.rs \
  src/probe/*.rs src/state/*.rs 2>/dev/null && echo FAIL || echo PASS

# R-G7: probe must not depend on App or UI
grep -lE "crate::app|crate::ui" src/probe/*.rs 2>/dev/null && echo FAIL || echo PASS

# R-G8: ui/forms must not import std::fs / std::process
grep -lE "std::fs|std::process::Command" src/ui/forms/*.rs src/ui/modal.rs 2>/dev/null \
  && echo FAIL || echo PASS

# v0.2 gates R-G1..R-G5 continue to apply.
```

## 10. Help text (legend + warnings)

Pressed via `?` key. Renders an info modal:

```
sshs — keys
  j/k or arrows  navigate
  /              filter (use @tag to filter by tag)
  Enter          ssh into selected host
  r              reconnect to last host
  a              add new host (sshs.conf)
  m              modify selected host
  d              delete selected host
  t              edit tags
  e              open $EDITOR on selected host's source file
  ?              this help
  q or Esc       quit (also exits filter / modal)

Probe states
  ●  TCP port reachable. NOT a guarantee that ssh login will succeed.
  ✕  TCP refused or timed out.
  ?  probe in flight.
  ○  not yet probed.

Read-only rows are dim and marked "·". They are defined in files
other than ~/.ssh/config.d/sshs.conf; use 'e' to edit them.

ProxyJump-only hosts cannot be probed directly and will show ✕.
```

## 11. Implementation Notes (pitfalls)

1. **flock holds a file descriptor**: keep the lock-holding `File` in scope until rename is complete.
2. **Tempfile in same dir as target**: rename across filesystems is not atomic.
3. **`canonicalize` requires the file to exist**: when checking Include presence for a path that may not exist, canonicalize the *parent dir* + manually compose the basename.
4. **`fcntl::flock` is advisory**: cooperative — only sshs respects it. If user opens sshs.conf in vi while sshs writes, vi will overwrite on save. Document but don't engineer around.
5. **Probe `TcpStream::connect_timeout`**: requires `SocketAddr`. DNS resolution via `format!("{}:{}", host, port).to_socket_addrs()` — itself synchronous, so do it inside the worker. Resolution failure → ProbeState::Failed.
6. **ratatui `TestBackend` for snapshot tests**: pin the backend size; otherwise snapshot is flaky.
7. **AppMode::Modal short-circuits handle_key**: ensure `q` / `Esc` reach the modal first; only the modal decides whether to exit.
8. **State.toml on first run**: it doesn't exist; load() returns the default. save() creates it lazily on first mutation.
9. **`StorageError::LockHeldByOther`** is NOT fatal: just surface as status message and let the user retry.
10. **glm-5.1 retrospective**: greenfield modules (`storage/*`, `setup/*`, `probe/*`, `state/*`, `ui/modal.rs`, `ui/forms/*`) and the new `config/tags.rs` go to glm. Existing-file modifications (`app.rs`, `ui/list.rs`, `config/parser.rs`, `config/model.rs`, `tui/runtime.rs`) are architect-direct surgical patches.

## 12. Pre-decisions (final state)

| Decision | Status | Note |
|---|---|---|
| sshs.conf only writable file | ✅ kept |
| `# @tags:` syntax | ✅ kept |
| std::thread + mpsc | ✅ kept |
| Channel-pushed probe results | ✅ kept |
| Keys a/d/m/t | ✅ confirmed (`m` decided) |
| Confirmation modals | ✅ kept |
| Permissions: warn, don't fix | ✅ kept |
| Include direction | ⚠️ **revised: append (not prepend)** |
| Tags rendering | ⚠️ **revised: inline prefix in Alias cell** |
| Probe glyph semantic | ⚠️ **revised: TCP-only, documented** |

## 13. Handoff Spec (task units → PLAN_V0.3.md)

| ID | Title | Files | Depends | Owner | Parallel-w |
|---|---|---|---|---|---|
| T0 | Cleanup v0.2 transitional code (should_*, refresh_hosts alias) | src/app.rs, src/tui/runtime.rs, tests/* | – | architect | – |
| T1 | error.rs extensions (StorageError, SetupError, ProbeError) | src/error.rs | T0 | glm | T2, T3 |
| T2 | config/tags.rs (parse/render/normalize) + Host.tags field | src/config/tags.rs, src/config/model.rs | T0 | glm (tags.rs), architect (model.rs) | T1, T3 |
| T3 | state/* (load/save state.toml) | src/state/* | T1 | glm | T1, T2 |
| T4 | storage/* (writer/path/serializer/include_injector) | src/storage/* | T1 | glm | T5 |
| T5 | probe/* (pool/worker/state) | src/probe/* | T1 | glm | T4 |
| T6 | ui/modal.rs (generalized base) | src/ui/modal.rs | – | glm | T7 |
| T7 | ui/forms/* (host_form, tag_form) | src/ui/forms/* | T6, T2 | glm | T8 |
| T8 | ui/list.rs 5-col table + tag prefix + source marker | src/ui/list.rs, src/ui/layout.rs | T2 | architect | – |
| T9 | parser.rs `# @tags:` recognition | src/config/parser.rs | T2 | architect | – |
| T10 | setup/* (first_run_flow) | src/setup/* | T3, T4 | glm | – |
| T11 | app.rs extensions (mode, probe_states, state, new actions) | src/app.rs | T1..T10 | architect | – |
| T12 | tui/runtime.rs probe wiring + modal dispatch | src/tui/runtime.rs | T5, T6, T11 | architect | – |
| T13 | Integration tests (storage round-trip, setup flow, probe live) | tests/*_test.rs | T4, T5, T10 | glm | – |
| T14 | docs/TESTING.md update + CHANGELOG.md v0.3 entry + README update | docs/, CHANGELOG.md, README.md | T12 | architect | – |

**Parallel rounds** (Ollama 3-slot):

```
R0: T0 (architect-only cleanup, serial)
R1: T1, T2-tags-file, T3 (3 glm slots)
R2: T2-model-patch, T9 (architect serial, then T8 architect)
R3: T4, T5, T6 (3 glm slots)
R4: T7, T10 (2 glm slots)
R5: T11 (architect, serial)
R6: T12 (architect, serial)
R7: T13 (glm)
R8: T14 (architect)
```

Refer to PLAN_V0.3.md (to be written) for delegation payload templates and per-round verification gates.

---

## End of Brief v3.
