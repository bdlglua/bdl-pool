#!/usr/bin/env bash
# Run as bdl on the pool server; sudo is used only for packages and services.
set -euo pipefail
umask 077
[[ "$(id -un)" == bdl && "$HOME" == /home/bdl ]] || { echo 'Lancer avec le compte bdl, sans sudo.' >&2; exit 1; }
script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
[[ "$script_dir" == /home/bdl/bdl-pool-site/api ]] || { echo 'Le dépôt doit être dans /home/bdl/bdl-pool-site.' >&2; exit 1; }
for tool in cargo sudo systemctl flock ss; do
  command -v "$tool" >/dev/null || { echo "Outil absent : $tool" >&2; exit 1; }
done
cargo build --locked --release --manifest-path "$script_dir/../services/Cargo.toml"
bdl_services_bin="$script_dir/../services/target/release/bdl-pool-services"
state_dir="$HOME/.local/state/bdl-pool-monitor"
[[ -r "$state_dir/repository/status.json" ]] || { echo 'Installer d’abord le suivi automatique du pool.' >&2; exit 1; }
exec 9>"$state_dir/api-install.lock"
flock -n 9 || { echo 'Une installation de l’API est déjà en cours.' >&2; exit 1; }
for service in nginx apache2; do
  if systemctl is-active --quiet "$service"; then
    echo "Un serveur $service est actif. Configuration HTTPS à adapter avant installation." >&2
    exit 1
  fi
done
# Refuse occupied ports belonging to another service; never stop them.
"$bdl_services_bin" install ports
temporary="$(mktemp -d)"
trap 'rm -rf -- "$temporary"' EXIT
mkdir -p "$HOME/.config/bdl-pool-api"
"$bdl_services_bin" install prepare "$script_dir" "$HOME/.config/bdl-pool-api/config.json" "$temporary/site.caddy"
sudo -v
if ! command -v caddy >/dev/null; then
  sudo apt-get update
  sudo apt-get install -y --no-install-recommends caddy
fi
sudo install -m 644 "$temporary/bdl-pool-api.service" /etc/systemd/system/bdl-pool-api.service
sudo install -m 644 "$temporary/bdl-pool-presence.service" /etc/systemd/system/bdl-pool-presence.service
sudo systemctl daemon-reload
sudo systemctl enable bdl-pool-presence.service
sudo systemctl enable bdl-pool-api.service
sudo systemctl restart bdl-pool-presence.service
sudo systemctl restart bdl-pool-api.service
"$bdl_services_bin" install health
sudo install -d -m 755 /etc/caddy/conf.d /etc/caddy/bdlminer-routes
stamp="$(date -u +%Y%m%dT%H%M%SZ)"
backup="/etc/caddy/Caddyfile.before-bdl-api-$stamp"
fragment=/etc/caddy/conf.d/bdl-pool-api.caddy
sudo cp -p -- /etc/caddy/Caddyfile "$backup"
had_fragment=false
if sudo test -e "$fragment"; then
  sudo cp -p -- "$fragment" "${backup}.fragment"
  had_fragment=true
fi
sudo install -m 644 "$temporary/site.caddy" "$fragment"
sudo "$bdl_services_bin" install caddy-import /etc/caddy/Caddyfile
if ! sudo -u caddy caddy validate --config /etc/caddy/Caddyfile >"$state_dir/api-caddy-validation.log" 2>&1; then
  sudo cp -p -- "$backup" /etc/caddy/Caddyfile
  if "$had_fragment"; then sudo cp -p -- "${backup}.fragment" "$fragment"; else sudo rm -f -- "$fragment"; fi
  echo 'Configuration Caddy refusée ; configuration précédente restaurée.' >&2
  echo "Journal local : $state_dir/api-caddy-validation.log" >&2
  exit 1
fi
sudo systemctl enable --now caddy.service
sudo systemctl reload caddy.service
"$bdl_services_bin" install describe "$HOME/.config/bdl-pool-api/config.json"
