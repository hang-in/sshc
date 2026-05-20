#!/usr/bin/env bash
# Prepares /tmp/sshc-demo-home so the vhs tapes have a deterministic
# environment to record against. Idempotent — wipes and re-creates.
set -euo pipefail

DEMO_HOME="/tmp/sshc-demo-home"
REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
FIX="$REPO_ROOT/docs/demos/fixtures"

rm -rf "$DEMO_HOME"
mkdir -p "$DEMO_HOME/.ssh/config.d"
mkdir -p "$DEMO_HOME/.config/sshc"
mkdir -p "$DEMO_HOME/bin"

cp "$FIX/ssh-config" "$DEMO_HOME/.ssh/config"
cp "$FIX/sshc.conf"  "$DEMO_HOME/.ssh/config.d/sshc.conf"
cp "$FIX/state.toml" "$DEMO_HOME/.config/sshc/state.toml"

chmod 0700 "$DEMO_HOME/.ssh" "$DEMO_HOME/.ssh/config.d" "$DEMO_HOME/.config" "$DEMO_HOME/.config/sshc"
chmod 0600 "$DEMO_HOME/.ssh/config" "$DEMO_HOME/.ssh/config.d/sshc.conf" "$DEMO_HOME/.config/sshc/state.toml"

# Wire up: fake ssh wrapper + real sshc binary on the demo PATH.
cp "$REPO_ROOT/docs/demos/bin/ssh" "$DEMO_HOME/bin/ssh"
chmod +x "$DEMO_HOME/bin/ssh"

if [ -x "$HOME/.cargo/bin/sshc" ]; then
  cp "$HOME/.cargo/bin/sshc" "$DEMO_HOME/bin/sshc"
elif [ -x "$REPO_ROOT/target/release/sshc" ]; then
  cp "$REPO_ROOT/target/release/sshc" "$DEMO_HOME/bin/sshc"
else
  echo "error: neither ~/.cargo/bin/sshc nor target/release/sshc exists. Run cargo install --path . first." >&2
  exit 1
fi
chmod +x "$DEMO_HOME/bin/sshc"

echo "Demo environment ready at $DEMO_HOME"
