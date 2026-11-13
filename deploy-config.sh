#!/usr/bin/env bash
# Push config.ini to Ubuntu machines over SSH.  Usage: ./deploy-config.sh [host ...]   (default: laptop3)
# On Windows the settings live in %APPDATA%\ClaudeVivarium\config.ini (the "Settings..." button opens it).
set -euo pipefail
cd "$(dirname "$0")"
HOSTS=("$@")
[ ${#HOSTS[@]} -eq 0 ] && HOSTS=(laptop3)
for HOST in "${HOSTS[@]}"; do
    ssh "$HOST" 'mkdir -p ~/.config/claude-vivarium'
    scp -q config.ini "$HOST":.config/claude-vivarium/config.ini
    echo "== $HOST:"
    ssh "$HOST" 'grep -E "^[a-z_]+ *=" ~/.config/claude-vivarium/config.ini'
done
