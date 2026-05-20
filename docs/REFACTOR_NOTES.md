# Refactor Notes — v0.4.3 attempt + deepseek-v4-pro:cloud review

Captured 2026-05-20 so the next session picks up without re-deriving
context.

Three sections:
- **§A** What was tried in v0.4.3 and what worked / what didn't.
- **§B** How to run the ollama-cloud deepseek-v4-pro review again.
- **§C** Verbatim review notes from that pass, cleaned of ANSI noise.

---

## §A v0.4.3 split attempts

### A.1 The target shape (BRIEF_V5 §3)

```
src/app/
├── mod.rs       struct App + AppAction/AppMode/FormContext + new + accessors
├── input.rs     handle_key + handle_list_key + handle_modal_key +
│                dispatch_modal_action + activate_selected
├── forms.rs     open_*_form + apply_form / apply_add / apply_modify /
│                apply_delete / apply_tags + persist_sshc_conf +
│                host_from_payload (free fn)
├── filter.rs    apply_filter                          ← landed in v0.4.3
└── tests.rs     all unit tests
```

### A.2 What landed in v0.4.3

- `src/app.rs` (944 LOC) → `src/app/mod.rs` (`git mv`). Build green.
- `src/app/filter.rs` created. `apply_filter` moved here as a single
  `impl super::App { ... }` block. mod.rs declares `mod filter;`.
  Build + tests green.
- `App.sshc_conf_path: Option<PathBuf>` field added, cached at
  construction. `sshc_conf_path_or_blank` became an **instance**
  method (was a `Self::` associated fn). All four call sites and
  the two test call sites updated. Build + tests green.
  (Reason: deepseek pointed out that `unwrap_or_default()`
  on `crate::storage::sshc_conf_path()` produced an empty `PathBuf`
  sentinel that *would* falsely match a host with an unset
  `source_file` — degenerate but real edge case.)

### A.3 What was attempted and reverted

**Goal**: extract `input.rs` and `forms.rs` in the same session.

**What broke**: a single python-driven line-range deletion
(`del lines[86:354]`) tried to excise an enormous block from
`mod.rs` covering the input methods. But the same range also held
**accessor methods** like `selected_host()`, `host_count()`,
`total_host_count()`, `exit_modal()`, `try_reconnect()` —
that were needed elsewhere (`ui/mod.rs` and `runtime.rs`). The
build cascade was 25 errors.

A separate attempt to generate `forms.rs` via line-range extraction
included the `impl App` closing `}` brace at line 649 of the
original `app.rs`, and then appended another closing `}` as a
"footer". Result: unbalanced braces, parse failure.

**Revert**: `git checkout HEAD -- src/app.rs` (the rename was still
in the working tree), `rm -rf src/app/`, restart from a clean state.

### A.4 Lessons codified in PLAN_V0.5.md §3

1. **Move one method at a time.** Never a line range. The protocol is
   copy → build (expect duplicate-definition error only) →
   delete-the-exact-body → build + test → commit.
2. **Audit accessor surface before slicing.** `wc -l src/app/mod.rs`
   should drop in predictable chunks. Anything over ~100 lines
   removed per commit is a red flag.
3. **Visibility surgery is per-method.** `pub(super) fn open_add_form`
   is one quick `sed`; `pub(super) fn open_modify_form` is another.
   Don't batch. Each one triggers compiler messages you want to see.
4. **`Self::` → instance method requires test-site fixes.** The
   v0.4.3 cache work caught this once (`App::sshc_conf_path_or_blank()`
   in tests → `crate::storage::sshc_conf_path().unwrap_or_default()`).
   If v0.5 promotes any other `Self::` method, audit
   `rg "App::<name>"` in `tests/` and `examples/`.

---

## §B Running the deepseek-v4-pro:cloud review

You need to be signed in to `ollama.com` (one-time
`ollama signin`). Cloud models don't need to be `pull`ed first; the
`run` invocation fetches metadata on demand.

```sh
# Build a review prompt — the file + a few framing questions
cat > /tmp/review_prompt.md <<'PROMPT'
You are a senior Rust reviewer. Read this src/app.rs from a TUI SSH
host manager (sshc, ratatui+crossterm).

Answer in ≤ 350 words, with three sections:
1. Blind spots in the proposed split (responsibilities likely to
   ping-pong or where boundaries leak)
2. Naming / ordering you'd improve
3. Lurking smaller refactors visible from this file (free functions
   to colocate, places that need expect() vs Result, etc.)

Do NOT rewrite code. Be specific to this file.

--- BEGIN src/app/mod.rs ---
PROMPT
cat src/app/mod.rs >> /tmp/review_prompt.md

# Run (background so the architect can keep working). --hidethinking
# suppresses the chain-of-thought output; the model still uses it
# internally.
cat /tmp/review_prompt.md \
  | ollama run --hidethinking deepseek-v4-pro:cloud \
  > /tmp/review_v5.txt 2> /tmp/review_v5_err.txt &
```

Expect ~2 minutes for a 900-LOC input. Result lands in
`/tmp/review_v5.txt`.

**Pitfall**: without `--hidethinking`, the model emits its raw
chain-of-thought first, interleaved with ANSI cursor control codes
that look like noise when you `cat` the file. The flag is needed.

**Alternative models**:
- `qwen3-coder:480b-cloud` — 262K context (larger), MoE.
- `deepseek-v3.1:671b-cloud` — older deepseek, 163K context.
- `deepseek-v4-pro:cloud` — 1M context, reasoning. Used in v0.4.3.

For sshc's whole-file reviews (<1000 LOC), any of the three works.
v4-pro produced the cleanest output in v0.4.3.

---

## §C deepseek-v4-pro review of v0.4.2 app.rs — verbatim, cleaned

> Cleaned from `/tmp/review_v4.txt` (the live capture had cursor
> control codes from the interactive `--hidethinking` output stream).
> Wording is preserved; only the ANSI noise was stripped.

### 1. Blind spots in the split

Private field visibility is the biggest leak waiting to happen.
`forms.rs` and `input.rs` will need to read/write `hosts`, `mode`,
`filter_mode`, `pending_action`, `active_form_context`, etc., but
these are currently private to mod.rs. You'll have to either make
them `pub(super)` (exposing internals to siblings) or add
accessor/mutator methods — both erode the boundary.

`apply_filter` is called from `forms.rs` (after add/modify/delete/
tags), `input.rs` (filter keystrokes), and `mod.rs`
(`replace_hosts`). Placing its definition in `filter.rs` is fine,
but the call graph crosses three modules; the "filter state"
(`filter_mode`, `query`) will be **mutated in input, consumed in
filter, and triggered from forms**. Without a clear ownership
contract, you'll get ping-ponging responsibilities and hard-to-
trace side effects.

`host_from_payload` and `persist_sshc_conf` are lumped into
`forms.rs` even though one is a pure conversion and the other is
storage logic. This forces `forms.rs` to know about
`crate::storage`, diluting the "forms" concept.

### 2. Naming / ordering you'd improve

- `forms.rs` → `forms_and_persistence.rs`, or split into
  `host_form.rs` + `conf_writer.rs` if it grows.
- `input.rs` → `key_handler.rs` to avoid confusion with general
  "input" (the file handles *key* input).
- `activate_selected` → `handle_enter_action` — it does more than
  activation; it's the Enter-key behaviour.
- The `filter.rs` module could be `filter_engine.rs` to highlight
  that `next`/`previous`/`adjust_scroll` are tightly coupled to
  the filtered list and not general navigation.

### 3. Lurking smaller refactors

- **Remove the redundant `host_from_payload` inside `apply_add` /
  `apply_modify`.** `apply_form` already matched
  `FormPayload::Host`. Pass the parsed fields directly, eliminating
  the `expect` and the double conversion.
- **Extract tag normalization**
  (`split(','), filter_map(normalize_tag), dedup`) into a
  `fn normalized_tags(csv: &str) -> Vec<String>`. It's duplicated in
  `apply_tags` and `host_from_payload`.
- **Cache `sshc_conf_path`** in `App` instead of calling
  `sshc_conf_path_or_blank` repeatedly. The fallback
  `unwrap_or_default()` produces an empty path that accidentally
  matches hosts with missing source files — a latent bug.
- **Review Esc-on-empty-filter.** In `handle_list_key`, `Esc` with
  empty query triggers `Quit` rather than just leaving filter mode;
  this may be intentional, but it's surprising and worth a comment
  or a dedicated guard.
- The `matcher` field is reused correctly, but its
  `nucleo::Config::DEFAULT` could be tuned (e.g., ignore case) and
  deserves a named constant.

### C.1 Architect triage of these comments

| # | Comment | v0.4.3 status | v0.5 status |
|---|---|---|---|
| Blind-spot 1 (visibility) | accepted as known cost | will hit during R2/R3 | use `pub(super)` per BRIEF_V5 §3.1 |
| Blind-spot 2 (apply_filter call graph) | acknowledged | no action — three callers is OK | revisit if it grows to five |
| Blind-spot 3 (storage in forms.rs) | accepted | low-priority | leave as is; forms.rs is the natural owner of writes |
| Naming 1 (forms.rs split) | rejected | — | premature; revisit if forms.rs > 280 LOC after R3 |
| Naming 2 (input.rs → key_handler.rs) | rejected | — | "input" is conventional; ratatui itself uses the term |
| Naming 3 (activate_selected) | rejected | — | name conveys the abstract intent; "handle_enter_action" is mechanical |
| Naming 4 (filter_engine.rs) | rejected | — | overspecified |
| Smaller (5) host_from_payload expect redundancy | **deferred to v0.5** | — | **accepted; BRIEF_V5 §4.1** |
| Smaller (6) normalized_tags helper | **deferred to v0.5** | — | **accepted; BRIEF_V5 §4.2** |
| Smaller (7) sshc_conf_path cache | **shipped in v0.4.3** | done | — |
| Smaller Esc-on-empty-filter comment | acknowledged | already intentional per BRIEF_V3 | add a `// intentional: BRIEF…` comment when forms.rs lands |
| Smaller nucleo Config constant | rejected | — | unlikely to ever tune; YAGNI |

---

## §D Sanity recipe: did the split work?

After R4 (split complete, deepseek fixes folded in), run:

```sh
# Boundaries
for f in src/app/*.rs; do
  loc=$(grep -cvE '^\s*($|//)' "$f")
  printf "%4d  %s\n" "$loc" "$f"
done

# Should print:
#   ~50   src/app/mod.rs    (only if accessors + struct kept it small)
#  150-280 src/app/forms.rs
#  150-200 src/app/input.rs
#   ~60    src/app/filter.rs
#  ~250    src/app/tests.rs

# Full gate
cargo test --release && cargo clippy --all-targets -- -D warnings && cargo fmt --check

# Boundaries
bash docs/regression-greps.sh   # (or whichever runner you keep R-G1..R-G9 in)
```

If `wc -l src/app/mod.rs` is still > 350, something didn't move that
should have. The most likely culprits:
- `try_reconnect` and `on_ssh_finished` are unmoved; they're
  small but they belong in mod.rs because they fall outside
  input/forms/filter responsibilities.
- `replace_hosts` and `apply_probe_updates` likewise belong in
  mod.rs.
- The `impl Default for X` blocks from any sub-type belong wherever
  the type lives.

If you find code that doesn't fit input / forms / filter / mod /
tests, that's a sign for a fifth module — but try hard not to
introduce one in v0.5. Prefer leaving it in `mod.rs` for now.

---

## §E Pointers to v0.4.3 commits

- `7697807` — README demo GIF consolidation (one combined demo).
- `8db893a` (in `hang-in/homebrew-tap`) — first v0.4.2 tap update.
- `b87df2e` (in `hang-in/homebrew-tap`) — axo bot's automated v0.4.2
  push after `HOMEBREW_TAP_TOKEN` was set.
- `ef8c532` — v0.4.3 cut: filter.rs split + sshc_conf_path cache.

For the v0.4.x sequence as a whole: `git log --oneline
v0.4.0..v0.4.3 src/app.rs src/app/`.

---

## End of notes.
