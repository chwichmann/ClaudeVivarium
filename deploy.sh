#!/usr/bin/env bash
# Deploy Claude Vivarium (code + config.ini) to Ubuntu machines over SSH, install, enable and restart.
# Usage: ./deploy.sh laptop1 laptop2 laptop3      (default: laptop3)
set -euo pipefail
cd "$(dirname "$0")"
HOSTS=("$@")
[ ${#HOSTS[@]} -eq 0 ] && HOSTS=(laptop3)
for HOST in "${HOSTS[@]}"; do
    echo "===== $HOST"
    ssh "$HOST" 'rm -rf ~/dev/claude-screensaver; mkdir -p ~/dev/claude-vivarium ~/.config/claude-vivarium; rm -f ~/dev/claude-vivarium/*.py'
    scp -q ./*.py install.sh claude-vivarium.service README.md "$HOST":dev/claude-vivarium/
    ssh "$HOST" 'cd ~/dev/claude-vivarium && chmod +x install.sh && ./install.sh'
    scp -q config.ini "$HOST":.config/claude-vivarium/config.ini
    ssh "$HOST" 'systemctl --user enable --now claude-vivarium.service \
        && systemctl --user restart claude-vivarium.service && sleep 2 \
        && systemctl --user is-active claude-vivarium.service \
        && journalctl --user -u claude-vivarium -n 1 --no-pager -o cat'
done
