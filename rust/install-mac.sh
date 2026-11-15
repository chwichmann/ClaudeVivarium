#!/usr/bin/env bash
# Install the Rust version of Claude Vivarium on macOS for the current user (no admin rights needed).
#   ./install-mac.sh             build (or use a ready binary), install and start the idle agent
#   ./install-mac.sh --uninstall stop and remove the agent (keeps the settings)
# Builds with cargo (https://rustup.rs). To install a binary downloaded from the GitHub build instead,
# put it next to this script as "claude-vivarium-rs".
# The Python version's agent is stopped, because only one of the two should start the screensaver.
set -euo pipefail
SRC="$(cd "$(dirname "$0")" && pwd)"
APPDIR="$HOME/Library/Application Support/ClaudeVivarium"
BIN="$APPDIR/bin/claude-vivarium-rs"
PLIST="$HOME/Library/LaunchAgents/io.github.claudevivarium.rs.plist"
OLD_PLIST="$HOME/Library/LaunchAgents/io.github.claudevivarium.plist"
LOG="$HOME/Library/Logs/ClaudeVivarium.log"
DOMAIN="gui/$(id -u)"

if [ "${1:-}" = "--uninstall" ]; then
    launchctl bootout "$DOMAIN" "$PLIST" 2>/dev/null || true
    rm -f "$PLIST" "$BIN"
    echo "Uninstalled. Settings kept in $APPDIR/config.ini"
    [ -f "$OLD_PLIST" ] && echo "To go back to the Python version: launchctl bootstrap $DOMAIN \"$OLD_PLIST\""
    exit 0
fi

if [ -x "$SRC/claude-vivarium-rs" ]; then
    BUILT="$SRC/claude-vivarium-rs"
else
    command -v cargo >/dev/null || { echo "Please install Rust first: https://rustup.rs"; exit 1; }
    (cd "$SRC" && cargo build --release)
    BUILT="$SRC/target/release/claude-vivarium-rs"
fi

mkdir -p "$APPDIR/bin" "$HOME/Library/LaunchAgents" "$HOME/Library/Logs"
launchctl bootout "$DOMAIN" "$PLIST" 2>/dev/null || true
if launchctl bootout "$DOMAIN" "$OLD_PLIST" 2>/dev/null; then
    echo "Stopped the Python version's agent (its files stay installed)."
fi
install -m 755 "$BUILT" "$BIN"
xattr -d com.apple.quarantine "$BIN" 2>/dev/null || true   # a downloaded binary would be blocked otherwise

cat > "$PLIST" <<PLIST_EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key><string>io.github.claudevivarium.rs</string>
    <key>ProgramArguments</key>
    <array>
        <string>$BIN</string>
        <string>daemon</string>
    </array>
    <key>RunAtLoad</key><true/>
    <key>KeepAlive</key><true/>
    <key>LimitLoadToSessionType</key><string>Aqua</string>
    <key>ProcessType</key><string>Interactive</string>
    <key>StandardOutPath</key><string>$LOG</string>
    <key>StandardErrorPath</key><string>$LOG</string>
</dict>
</plist>
PLIST_EOF
launchctl bootstrap "$DOMAIN" "$PLIST"

cat <<MSG_EOF

Claude Vivarium (Rust version) is installed and the agent is running.

  Start it now: press Ctrl+Option+V (any key or click then locks the Mac)
  Test first:   "$BIN" selftest
                "$BIN" run --windowed --demo
  Log:          tail -f "$LOG"
  Settings:     $APPDIR/config.ini
  Remove:       ./install-mac.sh --uninstall

In System Settings please set:
  - Lock Screen > "Start Screen Saver when inactive": longer than idle_seconds (or Never)
  - Lock Screen > "Turn display off when inactive":   longer than idle_seconds
  - Lock Screen > "Require password after screen saver begins or display is turned off": Immediately
MSG_EOF
