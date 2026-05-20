# sshc

> Manage and connect to your SSH hosts from a fast Rust TUI.

[![release](https://img.shields.io/github/v/release/hang-in/sshc?sort=semver)](https://github.com/hang-in/sshc/releases)
[![downloads](https://img.shields.io/github/downloads/hang-in/sshc/total)](https://github.com/hang-in/sshc/releases)
[![brew tap](https://img.shields.io/badge/brew-hang--in%2Ftap%2Fsshc-blue)](https://github.com/hang-in/homebrew-tap)
[![license](https://img.shields.io/badge/license-MIT-green)](LICENSE)
[![rust](https://img.shields.io/badge/rust-1.85%2B-orange)](https://www.rust-lang.org)
[![platform](https://img.shields.io/badge/platform-macOS%20%7C%20Linux-lightgrey)](https://github.com/hang-in/sshc/releases)

```sh
brew install hang-in/tap/sshc
```

> 한국어 문서: [README.ko.md](README.ko.md)

## Demo

![sshc inline mode: filter, pick, ssh, back to the shell](docs/demos/demo.gif)

## Why sshc?

After 30+ entries in `~/.ssh/config`, remembering which alias maps to
which host gets old. Editing the file by hand to add or rename a server
is tedious, and grepping aliases for ssh autocomplete loses tags and
context. sshc opens an fzf-style picker right under your shell prompt
— type, Enter, you're in.

## Quickstart

```sh
brew install hang-in/tap/sshc
sshc
```

That's it. sshc reads your existing `~/.ssh/config` and the files it
`Include`s — nothing extra to configure. For add / edit / delete +
tags, run `sshc -m`. Already know the alias? `sshc <alias>` skips the
TUI and `ssh`s straight in — handy for shell aliases and scripts.

## Two modes

| Command | Mode | What it does |
|---|---|---|
| `sshc` | Inline | Key-driven host picker → ssh → back to shell. No alternate-screen takeover. |
| `sshc <alias>` | Direct | Skip the picker, look up `<alias>` in your config, `ssh` immediately. Unknown alias → exit 1. |
| `sshc -m` | Manage | Full TUI: add/edit/delete hosts, tags, probe glyphs, `$EDITOR` jump. |
| `sshc --doctor` | Doctor | Read-only environment report (`~/.ssh`, sshc.conf, Include line, `ssh` binary, `SSH_AUTH_SOCK`). Exits non-zero only on `FAIL`. |

Inline mode never leaves your scrollback view. Manage mode opens the
classic full-screen TUI behind a flag because that's the right surface
for editing.

## Keybindings

### Inline

Inline is modal: navigation by default, `/` to start a search, Esc to
leave the search. Pinned hosts (via manage mode `f`) and recently-
connected hosts float to the top of the list, so the common case is
"open `sshc`, Enter".

| Key | Action |
|---|---|
| ↑ / ↓, j / k | navigate |
| `/` | enter search mode |
| Enter | ssh to highlighted host → exit |
| q, Esc | quit (in nav mode) |
| Ctrl+C | quit (any mode) |
| (in search) any char | append to filter query |
| (in search) Backspace | delete one filter char |
| (in search) Esc | exit search mode (keeps picker open) |

### Manage

| Key | Action |
|---|---|
| ↑/↓, j/k | navigate |
| `/` | enter filter mode (`@tag` for tag-only) |
| Enter | open edit form (managed host) / `$EDITOR` jump (external) |
| s | ssh |
| a | add host |
| d | delete (with confirm) |
| t | edit tags |
| f | toggle favorite / pin (★) |
| v | `ssh -G <alias>` validation modal |
| e | open `$EDITOR` at the host's line |
| i | inject `Include` line into `~/.ssh/config` |
| ? | help modal |
| q, Esc | quit / cancel |

Inside a form: Tab/Shift+Tab move fields, Enter submits, Esc cancels,
Ctrl+U clears the active field.

## Installation

### Homebrew (macOS + Linux)

```sh
brew install hang-in/tap/sshc
```

`brew upgrade sshc` follows new releases automatically.

### Cargo from git

```sh
cargo install --git https://github.com/hang-in/sshc --tag v0.7.2
```

### From source

```sh
git clone https://github.com/hang-in/sshc
cd sshc
cargo install --path .
```

Requires Rust 1.85+.

### Windows

Native Windows support landed in v0.7.0. Use the PowerShell
installer or grab the `sshc-x86_64-pc-windows-msvc.zip` artifact
from the [Releases](https://github.com/hang-in/sshc/releases)
page directly.

```powershell
# PowerShell — installs into %CARGO_HOME%\bin and updates PATH
irm https://github.com/hang-in/sshc/releases/latest/download/sshc-installer.ps1 | iex
```

Requires Windows 10 1809+ or Windows 11, plus the OpenSSH client
(`Settings → Apps → Optional features → OpenSSH Client`). File
permissions are not enforced — Windows ACLs use a different model
than Unix mode bits, so sshc skips the 0600/0700 checks and leaves
inheritance to the parent directory. SSH agent integration uses
whatever your environment provides (Windows OpenSSH agent,
Pageant); `SSH_AUTH_SOCK` is not consulted.

WSL2 also works the same way as v0.6 — the Linux build is still
shipped and behaves identically inside a WSL distribution.

## Configuration

sshc never edits your hand-written `~/.ssh/config` beyond a single
optional `Include` line.

- Hosts you add via `sshc -m` are written to
  `~/.ssh/config.d/sshc.conf` (mode 0600). File starts with:
  `# Managed by sshc. Manual edits inside Host blocks may be overwritten on next save.`
- The first run of manage mode prompts to append
  `Include ~/.ssh/config.d/sshc.conf` to `~/.ssh/config`, with a
  dated `.bak.sshc-YYYYMMDD` backup. Decline and manage mode goes
  read-only (browse + connect still work).
- State (Include decision + last-connected alias) is stored at
  `~/.config/sshc/state.toml`.
- Tags live in `# @tags: a, b, c` comments immediately above each
  `Host` block. Filter with `@<tag>` (e.g. `@prod`).

## How it compares

A small Rust TUI that's friendlier than rolling your own `fzf`
snippets, safer than inline-editing tools like `storm`, and lighter
than full SSH GUIs — opinionated about respecting your existing
`~/.ssh/config`.

- **Plain `ssh <alias>`**: still the source of truth. sshc reads
  your existing config; nothing about your current workflow changes.
- **[storm](https://github.com/emre/storm)**: a Python CLI that
  edits `~/.ssh/config` directly. sshc keeps a separate file and
  only injects one Include line.

## License

MIT. See [LICENSE](LICENSE).

## Contributing

Issues and PRs welcome. I aim to reply within 24 hours.
