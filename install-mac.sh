#!/usr/bin/env bash
# Install Claude Vivarium on macOS for the current user (no admin rights needed).
#   ./install-mac.sh             install / update and start the agent
#   ./install-mac.sh --uninstall stop and remove the agent (keeps the settings)
# Needs Homebrew (https://brew.sh) and the Xcode command line tools (xcode-select --install).
set -euo pipefail
SRC="$(cd "$(dirname "$0")" && pwd)"
APPDIR="$HOME/Library/Application Support/ClaudeVivarium"
VENV="$APPDIR/venv"
PLIST="$HOME/Library/LaunchAgents/io.github.claudevivarium.plist"
LOG="$HOME/Library/Logs/ClaudeVivarium.log"
DOMAIN="gui/$(id -u)"

if [ "${1:-}" = "--uninstall" ]; then
    launchctl bootout "$DOMAIN" "$PLIST" 2>/dev/null || true
    rm -f "$PLIST"
    rm -rf "$APPDIR/app" "$VENV"
    echo "Uninstalled. Settings kept in $APPDIR/config.ini"
    exit 0
fi

command -v brew >/dev/null || { echo "Please install Homebrew first: https://brew.sh"; exit 1; }
xcode-select -p >/dev/null 2>&1 || { echo "Please run: xcode-select --install"; exit 1; }
for pkg in cairo pkg-config python@3.12; do
    brew list "$pkg" >/dev/null 2>&1 || brew install "$pkg"
done
PY="$(brew --prefix python@3.12)/bin/python3.12"

mkdir -p "$APPDIR/app" "$HOME/Library/LaunchAgents" "$HOME/Library/Logs"
[ -x "$VENV/bin/python" ] || "$PY" -m venv "$VENV"
"$VENV/bin/pip" install -q --disable-pip-version-check --upgrade pip
PKG_CONFIG_PATH="$(brew --prefix)/lib/pkgconfig" "$VENV/bin/pip" install -q --disable-pip-version-check \
    pycairo pyobjc-framework-Cocoa pyobjc-framework-Quartz

rm -f "$APPDIR/app"/*.py
for f in "$SRC"/*.py; do
    case "$(basename "$f")" in
        vivarium_gtk.py|vivariumd.py|gnome.py|vivarium_win.py) ;;   # Linux / Windows only
        *) cp "$f" "$APPDIR/app/" ;;
    esac
done
"$VENV/bin/python" -c "import sys; sys.path.insert(0, '$APPDIR/app'); import config; config.load(); print('Settings:', config.PATH)"

cat > "$PLIST" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key><string>io.github.claudevivarium</string>
    <key>ProgramArguments</key>
    <array>
        <string>$VENV/bin/python</string>
        <string>$APPDIR/app/vivariumd_mac.py</string>
    </array>
    <key>RunAtLoad</key><true/>
    <key>KeepAlive</key><true/>
    <key>LimitLoadToSessionType</key><string>Aqua</string>
    <key>ProcessType</key><string>Interactive</string>
    <key>StandardOutPath</key><string>$LOG</string>
    <key>StandardErrorPath</key><string>$LOG</string>
</dict>
</plist>
EOF
launchctl bootout "$DOMAIN" "$PLIST" 2>/dev/null || true
launchctl bootstrap "$DOMAIN" "$PLIST"

cat <<EOF

Claude Vivarium is installed and the agent is running.

  Test first:   "$VENV/bin/python" "$APPDIR/app/vivarium_mac.py" --selftest
                "$VENV/bin/python" "$APPDIR/app/vivarium_mac.py" --windowed --demo
  Log:          tail -f "$LOG"
  Settings:     $APPDIR/config.ini
  Remove:       ./install-mac.sh --uninstall

In System Settings please set:
  - Lock Screen > "Start Screen Saver when inactive": longer than idle_seconds (or Never)
  - Lock Screen > "Turn display off when inactive":   longer than idle_seconds
  - Lock Screen > "Require password after screen saver begins or display is turned off": Immediately
EOF
