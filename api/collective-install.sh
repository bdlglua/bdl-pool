#!/usr/bin/env bash
# Install the private BDL collective coordinator beside the read-only pool API.
# Run as the existing unprivileged bdl account, from /home/bdl/bdl-pool-site.
set -euo pipefail
umask 077

[[ "$(id -un)" == bdl && "$HOME" == /home/bdl ]] || {
  echo 'Run as the bdl account, without sudo.' >&2
  exit 1
}
root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
[[ "$root" == /home/bdl/bdl-pool-site ]] || {
  echo 'Repository must be /home/bdl/bdl-pool-site.' >&2
  exit 1
}
bin="$root/services/target/release/bdl-pool-services"
config="$HOME/.config/bdl-pool-collective/config.json"
ledger_dir=/var/lib/bdl-pool-collective
unit=/etc/systemd/system/bdl-pool-collective.service
route=/etc/caddy/bdlminer-routes/collective.caddy
temporary="$(mktemp -d)"
trap 'rm -rf -- "$temporary"' EXIT

for tool in cargo sudo systemctl ss curl; do
  command -v "$tool" >/dev/null || { echo "Missing tool: $tool" >&2; exit 1; }
done

cargo build --locked --release --manifest-path "$root/services/Cargo.toml"
[[ -x "$bin" ]] || { echo 'Coordinator binary is missing after build.' >&2; exit 1; }

if [[ ! -e "$config" ]]; then
  "$bin" collective-init --config "$config" --ledger "$ledger_dir/ledger.jsonl"
  echo
  echo 'The ADMIN_TOKEN and BACKEND_TOKEN above were generated once.'
  echo 'Save them now in a password manager. Do not put either token in firmware.'
  echo
else
  echo "Coordinator config already exists: $config (secrets were not replaced)."
fi

if ss -H -ltn 'sport = :8082' | grep -q .; then
  if ! systemctl is-active --quiet bdl-pool-collective.service; then
    echo 'Port 8082 is occupied by another service; installation stopped.' >&2
    exit 1
  fi
fi

sudo -v
sudo install -d -o bdl -g bdl -m 700 "$ledger_dir"
sudo install -d -m 755 /etc/caddy/bdlminer-routes

cat >"$temporary/bdl-pool-collective.service" <<EOF
[Unit]
Description=BDL private collective mining coordinator
Wants=network-online.target
After=network-online.target

[Service]
Type=simple
User=bdl
Group=bdl
WorkingDirectory=/
ExecStart=$bin collective --config $config --port 8082
Restart=on-failure
RestartSec=5
UMask=0077
NoNewPrivileges=true
PrivateTmp=true
PrivateDevices=true
ProtectSystem=strict
ProtectHome=tmpfs
BindReadOnlyPaths=$bin
BindReadOnlyPaths=$config
StateDirectory=bdl-pool-collective
StateDirectoryMode=0700
RestrictAddressFamilies=AF_INET AF_INET6
MemoryMax=512M
TasksMax=64
LimitNOFILE=128

[Install]
WantedBy=multi-user.target
EOF
cat >"$temporary/collective.caddy" <<'EOF'
handle /api/v1/collective/* {
    reverse_proxy 127.0.0.1:8082 {
        transport http {
            dial_timeout 3s
            response_header_timeout 10s
        }
    }
}
EOF

# Preserve existing files and restore them if Caddy validation fails.
stamp="$(date -u +%Y%m%dT%H%M%SZ)"
unit_backup="$temporary/unit.before"
route_backup="$temporary/route.before"
had_unit=false
had_route=false
if sudo test -e "$unit"; then sudo cp -p "$unit" "$unit_backup"; had_unit=true; fi
if sudo test -e "$route"; then sudo cp -p "$route" "$route_backup"; had_route=true; fi

sudo install -m 644 "$temporary/bdl-pool-collective.service" "$unit"
sudo install -m 644 "$temporary/collective.caddy" "$route"
sudo systemctl daemon-reload

if ! sudo -u caddy caddy validate --config /etc/caddy/Caddyfile >"$temporary/caddy-validate.log" 2>&1; then
  if $had_unit; then sudo cp -p "$unit_backup" "$unit"; else sudo rm -f "$unit"; fi
  if $had_route; then sudo cp -p "$route_backup" "$route"; else sudo rm -f "$route"; fi
  sudo systemctl daemon-reload
  cat "$temporary/caddy-validate.log" >&2
  echo 'Caddy validation failed; previous configuration restored.' >&2
  exit 1
fi

sudo systemctl enable --now bdl-pool-collective.service
sudo systemctl reload caddy.service

for _ in $(seq 1 20); do
  if curl --silent --show-error --fail http://127.0.0.1:8082/api/v1/collective/status >"$temporary/status.json"; then
    break
  fi
  sleep 0.5
done
python3 - "$temporary/status.json" <<'PY'
import json, sys
with open(sys.argv[1], encoding="utf-8") as stream:
    value = json.load(stream)
if value.get("status") != "ok" or value.get("schemaVersion") != 1:
    raise SystemExit("Coordinator health response is invalid.")
print(json.dumps(value, indent=2))
PY

echo
echo 'Coordinator service installed on loopback 127.0.0.1:8082.'
echo 'HTTPS route: https://api.bdl-pool.com:36018/api/v1/collective/status'
echo 'No Bitcoin Core, CKPool, BCHN or CashStratum service was restarted.'
echo 'The Stratum adapters are not connected yet; no live collective shares are accepted yet.'
