#!/usr/bin/env bash
# Install a systemd timer on the pool server. Run as bdl, not with sudo.
set -euo pipefail
[[ "$(id -un)" == bdl && "$HOME" == /home/bdl ]] || { echo 'Lancer ce script avec le compte bdl, sans sudo.' >&2; exit 1; }
for tool in git python3 ssh flock systemctl sudo; do
  command -v "$tool" >/dev/null || { echo "Outil absent : $tool" >&2; exit 1; }
done
script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
[[ "$script_dir" == /home/bdl/bdl-pool-site/monitoring ]] || { echo 'Cloner le dépôt dans /home/bdl/bdl-pool-site avant installation.' >&2; exit 1; }
[[ -f "$HOME/.ssh/bdl-pool-stats" ]] || { echo 'Clé privée bdl-pool-stats absente.' >&2; exit 1; }
# Foreground SSH allows the first host fingerprint confirmation; never accept it silently.
GIT_SSH_COMMAND="ssh -i $HOME/.ssh/bdl-pool-stats -o IdentitiesOnly=yes -o ConnectTimeout=15"
export GIT_SSH_COMMAND
git ls-remote --exit-code git@github.com:bdlglua/bdl-pool.git refs/heads/stats >/dev/null
# Publish once successfully before enabling any background task.
bash "$script_dir/publish.sh"
sudo -v
unit_dir="$(mktemp -d)"
trap 'rm -rf -- "$unit_dir"' EXIT
cat > "$unit_dir/bdl-pool-monitor.service" <<'UNIT'
[Unit]
Description=BDL Pool public statistics snapshot
Wants=network-online.target
After=network-online.target bitcoin.service

[Service]
Type=oneshot
User=bdl
Group=bdl
Environment=HOME=/home/bdl
Environment=PATH=/usr/local/bin:/usr/bin:/bin
WorkingDirectory=/home/bdl
ExecStart=/bin/bash /home/bdl/bdl-pool-site/monitoring/publish.sh
TimeoutStartSec=120
UMask=0077
NoNewPrivileges=true
PrivateTmp=true
UNIT
cat > "$unit_dir/bdl-pool-monitor.timer" <<'UNIT'
[Unit]
Description=Update BDL Pool statistics every five minutes

[Timer]
OnBootSec=2min
OnUnitActiveSec=5min
AccuracySec=15s
Unit=bdl-pool-monitor.service

[Install]
WantedBy=timers.target
UNIT
sudo install -m 644 "$unit_dir/bdl-pool-monitor.service" /etc/systemd/system/bdl-pool-monitor.service
sudo install -m 644 "$unit_dir/bdl-pool-monitor.timer" /etc/systemd/system/bdl-pool-monitor.timer
sudo systemctl daemon-reload
sudo systemctl enable --now bdl-pool-monitor.timer
systemctl list-timers bdl-pool-monitor.timer --no-pager
echo 'Suivi installé : publication toutes les cinq minutes, y compris après redémarrage.'
echo 'Les caches GitHub peuvent retarder l’affichage de quelques minutes.'
