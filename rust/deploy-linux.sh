#!/usr/bin/env bash
# Deploy the Rust version to Ubuntu machines over SSH and install it there.
# Usage: ./deploy-linux.sh path/to/claude-vivarium-rs host1 [host2 ...]
#   The binary must be a Linux build (from the GitHub build, or "cargo build --release" on Ubuntu).
set -euo pipefail
cd "$(dirname "$0")"
[ $# -ge 2 ] || { echo "usage: $0 <linux binary> <host> [host ...]"; exit 1; }
BINARY="$1"; shift
file "$BINARY" | grep -q ELF || { echo "$BINARY is not a Linux program"; exit 1; }
for HOST in "$@"; do
    echo "===== $HOST"
    ssh "$HOST" 'mkdir -p ~/dev/claude-vivarium-rs ~/.config/claude-vivarium'
    scp -q "$BINARY" "$HOST":dev/claude-vivarium-rs/claude-vivarium-rs
    scp -q install-linux.sh claude-vivarium-rs.service "$HOST":dev/claude-vivarium-rs/
    ssh "$HOST" 'cd ~/dev/claude-vivarium-rs && chmod +x install-linux.sh claude-vivarium-rs && ./install-linux.sh \
        && journalctl --user -u claude-vivarium-rs -n 2 --no-pager -o cat'
done
