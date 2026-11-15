#!/usr/bin/env bash
# Install the Rust version of Claude Vivarium for the current user on Ubuntu/GNOME (no root needed).
#   ./install-linux.sh             build (or use a ready binary), install the idle watcher service
#   ./install-linux.sh --uninstall stop and remove it (keeps the settings)
# Builds with cargo (https://rustup.rs, plus "sudo apt install build-essential"). To install a binary
# downloaded from the GitHub build instead, put it next to this script as "claude-vivarium-rs".
# The Python version's service is disabled, because only one of the two should start the screensaver.
set -euo pipefail
SRC="$(cd "$(dirname "$0")" && pwd)"
BIN="$HOME/.local/bin/claude-vivarium-rs"
UNIT="$HOME/.config/systemd/user/claude-vivarium-rs.service"

if [ "${1:-}" = "--uninstall" ]; then
    systemctl --user disable --now claude-vivarium-rs.service 2>/dev/null || true
    rm -f "$UNIT" "$BIN"
    systemctl --user daemon-reload
    echo "Uninstalled. Settings kept in ~/.config/claude-vivarium/config.ini"
    echo "To go back to the Python version: systemctl --user enable --now claude-vivarium.service"
    exit 0
fi

if [ -x "$SRC/claude-vivarium-rs" ]; then
    BUILT="$SRC/claude-vivarium-rs"
else
    command -v cargo >/dev/null || { echo "Please install Rust first: https://rustup.rs"; exit 1; }
    (cd "$SRC" && cargo build --release)
    BUILT="$SRC/target/release/claude-vivarium-rs"
fi

mkdir -p "$HOME/.local/bin" "$HOME/.config/systemd/user"
if systemctl --user is-enabled --quiet claude-vivarium.service 2>/dev/null; then
    systemctl --user disable --now claude-vivarium.service
    echo "Disabled the Python version's service (its files stay installed)."
fi
systemctl --user stop claude-vivarium-rs.service 2>/dev/null || true
install -m 755 "$BUILT" "$BIN"
sed "s|@BIN@|$BIN|g" "$SRC/claude-vivarium-rs.service" > "$UNIT"
systemctl --user daemon-reload
systemctl --user enable --now claude-vivarium-rs.service
sleep 1
systemctl --user is-active claude-vivarium-rs.service

cat <<MSG_EOF

Claude Vivarium (Rust version) is installed and the idle watcher is running.

  Test first:   claude-vivarium-rs selftest --out /tmp/vivarium-selftest
                claude-vivarium-rs run --windowed --demo
  Log:          journalctl --user -u claude-vivarium-rs -f
  Settings:     ~/.config/claude-vivarium/config.ini
  Remove:       ./install-linux.sh --uninstall
MSG_EOF
