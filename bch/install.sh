#!/usr/bin/env bash
# Prepare only: starting the node and exposing a real relay are explicit next steps.
set -euo pipefail
umask 077
[[ $(id -un) == bdl && "$HOME" == /home/bdl ]] || { echo 'Lancer avec le compte bdl, sans sudo.' >&2; exit 1; }
script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
[[ "$script_dir" == /home/bdl/bdl-pool-site/bch ]] || { echo 'Chemin du dépôt inattendu.' >&2; exit 1; }
[[ $# -gt 0 ]] || { python3 "$script_dir/configure.py" --help; exit 1; }
node_bin="$HOME/.local/opt/bchn-29.2.0/bin"
pool_bin="$HOME/.local/opt/cashstratum-1.7.1/bin/cashstratum"
[[ -x "$node_bin/bitcoind" && -x "$node_bin/bitcoin-cli" && -x "$pool_bin" ]] || { echo 'Compiler d’abord les binaires : bash bch/build.sh' >&2; exit 1; }
case "$("$node_bin/bitcoind" --version)" in
  *"Bitcoin Cash Node"*) ;;
  *) echo 'Le binaire fourni n’est pas BCHN.' >&2; exit 1 ;;
esac
python3 "$script_dir/configure.py" "$@"
units="$HOME/.config/bdl-pool-bch/units"
sudo install -m 644 "$units/bdl-bchn.service" "$units/bdl-pool-bch.service" /etc/systemd/system/
sudo systemctl daemon-reload
echo 'BCH préparé. Pour démarrer la synchronisation puis le pool :'
echo 'sudo systemctl enable --now bdl-bchn bdl-pool-bch'
echo 'Puis mettre à jour le suivi et l’API selon bch/README.md.'
