# sshs

A minimal TUI for managing and connecting to SSH hosts defined in `~/.ssh/config`.

## Install

```sh
cargo install --path .
```

## Usage

```sh
sshs
```

Opens a centered TUI listing all SSH hosts from `~/.ssh/config`.

### Keybindings

| Key | Action |
|-----|--------|
| `↑` / `k` | Move selection up |
| `↓` / `j` | Move selection down |
| `/` | Enter fuzzy filter mode |
| `Enter` | Connect to selected host (SSH) |
| `e` | Open `$EDITOR` at selected host's config block |
| `Esc` | Exit filter mode / Quit |
| `q` | Quit |

### Editor Jump

When pressing `e`, the editor opens `~/.ssh/config` at the line where the selected `Host` block starts. The `+<line>` flag works with `vi`, `vim`, `nvim`, and `nano`. Other editors (e.g., `code`, `emacs`) will open the file but may ignore the line specifier.

## Testing

```sh
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
cargo build --release
```

## Limitations

- Unix-only (uses `exec()` to replace process with SSH)
- Read-only: edit mode delegates to `$EDITOR`, no direct file writes
- No Windows support
- Fuzzy search is inline (sufficient for <500 hosts)