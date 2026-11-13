#!/usr/bin/env bash
# Install Claude Vivarium for the current user on Ubuntu/GNOME (no root needed except for the apt line).
# Also migrates an older "claude-screensaver" installation (settings are carried over).
set -euo pipefail
SRC="$(cd "$(dirname "$0")" && pwd)"
DEST="$HOME/.local/share/claude-vivarium"
CONF="$HOME/.config/claude-vivarium"
UNIT="$HOME/.config/systemd/user/claude-vivarium.service"

if ! python3 -c 'import gi; gi.require_foreign("cairo")' 2>/dev/null; then
    echo "Installing python3-gi-cairo (needs sudo)..."
    sudo apt-get install -y python3-gi-cairo
fi

# --- migrate the old name
OLD_UNIT="$HOME/.config/systemd/user/claude-screensaver.service"
if [ -f "$OLD_UNIT" ]; then
    echo "Migrating claude-screensaver -> claude-vivarium"
    systemctl --user disable --now claude-screensaver.service 2>/dev/null || true
    rm -f "$OLD_UNIT"
    rm -rf "$HOME/.local/share/claude-screensaver"
    rm -f "$HOME/.local/bin/claude-screensaver"
fi
if [ -f "$HOME/.config/claude-screensaver/config.ini" ] && [ ! -f "$CONF/config.ini" ]; then
    mkdir -p "$CONF"
    cp "$HOME/.config/claude-screensaver/config.ini" "$CONF/config.ini"
fi
rm -rf "$HOME/.config/claude-screensaver"

# --- install
mkdir -p "$DEST" "$HOME/.config/systemd/user" "$HOME/.local/bin"
rm -f "$DEST"/*.py
install -m 644 "$SRC"/*.py "$DEST"/
chmod 755 "$DEST/vivarium_gtk.py" "$DEST/vivariumd.py"
rm -f "$DEST/vivarium_win.py" "$DEST/vivarium_mac.py" "$DEST/vivariumd_mac.py"   # other platforms
python3 -c "import sys; sys.path.insert(0, '$DEST'); import config; config.load(); print('Settings:', config.PATH)"
ln -sf "$DEST/vivarium_gtk.py" "$HOME/.local/bin/claude-vivarium"
sed "s|@DEST@|$DEST|g" "$SRC/claude-vivarium.service" > "$UNIT"
systemctl --user daemon-reload
if systemctl --user is-active --quiet claude-vivarium.service; then
    systemctl --user restart claude-vivarium.service
    echo "Updated and restarted."
else
    echo "Installed. Enable with:  systemctl --user enable --now claude-vivarium.service"
fi
