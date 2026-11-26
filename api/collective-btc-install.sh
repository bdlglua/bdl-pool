#!/usr/bin/env bash
# Install the separate BTC collective CKPool, V1 proxy, share collector and
# Bitcoin Core maturity watcher. Run as the unprivileged bdl account.
set -euo pipefail
umask 077

[[ "$(id -un)" == bdl && "$HOME" == /home/bdl ]] || {
  echo 'Run this installer as the bdl account, without sudo.' >&2
  exit 1
}
root="$(cd -- "$(dirname -- "$0")/.." && pwd)"
[[ "$root" == /home/bdl/bdl-pool-site ]] || {
  echo 'Repository must be /home/bdl/bdl-pool-site.' >&2
  exit 1
}
for tool in cargo sudo systemctl ss python3 cmp curl; do
  command -v "$tool" >/dev/null || { echo "Missing tool: $tool" >&2; exit 1; }
done

ckpool_root="$HOME/mining/ckpool"
ckpool_config_source="$ckpool_root/local.conf"
ckpool_config="$ckpool_root/collective.conf"
share_logs="$ckpool_root/collective-logs"
coordinator_config="$HOME/.config/bdl-pool-collective/config.json"
ingest_config="$HOME/.config/bdl-pool-collective/btc-ingest.json"
watch_config="$HOME/.config/bdl-pool-collective/btc-watch.json"
state_directory=/var/lib/bdl-pool-collective
share_cursor_path=$state_directory/btc-share-cursors.json
block_cursor_path=$state_directory/btc-block-cursor.json
runtime_directory=/run/bdl-pool-ckpool-collective
ckpool_unit=/etc/systemd/system/bdl-pool-ckpool-collective.service
ingest_unit=/etc/systemd/system/bdl-pool-collective-ingest.service
watch_unit=/etc/systemd/system/bdl-pool-collective-btc-watch.service
ckpool_service=bdl-pool-ckpool-collective.service
ingest_service=bdl-pool-collective-ingest.service
watch_service=bdl-pool-collective-btc-watch.service

[[ -f "$ckpool_config_source" && ! -L "$ckpool_config_source" ]] || {
  echo "Missing CKPool config: $ckpool_config_source" >&2
  exit 1
}
[[ -f "$coordinator_config" && ! -L "$coordinator_config" ]] || {
  echo 'Coordinator config is missing. Install api/collective-install.sh first.' >&2
  exit 1
}
[[ -x "$ckpool_root/src/ckpool" ]] || {
  echo "CKPool binary not found at $ckpool_root/src/ckpool. Set up/build the existing CKPool installation first." >&2
  exit 1
}
ckpool_bin="$ckpool_root/src/ckpool"
service_bin="$root/services/target/release/bdl-pool-services"

if ! sudo systemctl is-active --quiet bdl-pool-collective.service; then
  echo 'The private collective coordinator service is not active.' >&2
  echo 'Start/install it first: bash api/collective-install.sh' >&2
  exit 1
fi
if ! curl --silent --show-error --fail \
    http://127.0.0.1:8082/api/v1/collective/status >/dev/null; then
  echo 'The private collective coordinator API is not healthy on 127.0.0.1:8082.' >&2
  exit 1
fi

if [[ -z "${BDL_COLLECTIVE_BTC_ADDRESS:-}" ]]; then
  read -r -p 'Common BTC mainnet reward address for the collective pool: ' BDL_COLLECTIVE_BTC_ADDRESS
fi
[[ -n "$BDL_COLLECTIVE_BTC_ADDRESS" ]] || {
  echo 'A common BTC payout address is required.' >&2
  exit 1
}

if [[ -z "${BDL_COLLECTIVE_PPLNS_WINDOW_WORK_UNITS:-}" ]]; then
  read -r -p 'PPLNS window in work units (1,000,000 = difficulty 1): ' BDL_COLLECTIVE_PPLNS_WINDOW_WORK_UNITS
fi
[[ "$BDL_COLLECTIVE_PPLNS_WINDOW_WORK_UNITS" =~ ^[1-9][0-9]{0,19}$ ]] || {
  echo 'PPLNS window must be a positive integer (up to 64 bits).' >&2
  exit 1
}

mkdir -p "$HOME/.local/state"
chmod 700 "$HOME/.local/state"
temporary="$(mktemp -d "$HOME/.local/state/bdl-collective-btc-install.XXXXXX")"
trap 'rm -rf -- "$temporary"' EXIT
config_candidate="$temporary/collective.conf"
ingest_candidate="$temporary/btc-ingest.json"
watch_candidate="$temporary/btc-watch.json"
ckpool_verify_log="$temporary/ckpool-config-check.log"

CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-1}" cargo build --locked --release --manifest-path "$root/services/Cargo.toml"
[[ -x "$service_bin" ]] || { echo 'Collective Rust service binary is missing after build.' >&2; exit 1; }

python3 "$root/api/collective_btc_config.py" \
  --source "$ckpool_config_source" \
  --destination "$config_candidate" \
  --btc-address "$BDL_COLLECTIVE_BTC_ADDRESS" \
  --log-directory "$share_logs"

python3 "$root/api/collective_btc_ingest_config.py" \
  --coordinator-config "$coordinator_config" \
  --destination "$ingest_candidate" \
  --share-log-directory "$share_logs" \
  --cursor-path "$share_cursor_path"

python3 "$root/api/collective_btc_watch_config.py" \
  --pool-config "$config_candidate" \
  --coordinator-config "$coordinator_config" \
  --destination "$watch_candidate" \
  --cursor-path "$block_cursor_path" \
  --window-work-units "$BDL_COLLECTIVE_PPLNS_WINDOW_WORK_UNITS"

# Never silently replace a different reward address, RPC credential set,
# PPLNS policy or cursor path. Keep the original start height on reinstall.
if [[ -e "$ckpool_config" ]]; then
  cmp -s "$config_candidate" "$ckpool_config" || {
    echo "$ckpool_config already exists and differs from the generated config." >&2
    echo 'No file or service was changed. Review the existing file manually first.' >&2
    exit 1
  }
else
  install -m 600 "$config_candidate" "$ckpool_config"
fi
if [[ -e "$ingest_config" ]]; then
  cmp -s "$ingest_candidate" "$ingest_config" || {
    echo "$ingest_config already exists and differs from the coordinator configuration." >&2
    echo 'No file or service was changed. Review it manually first.' >&2
    exit 1
  }
else
  install -m 600 "$ingest_candidate" "$ingest_config"
fi
if [[ -e "$watch_config" ]]; then
  python3 - "$watch_candidate" "$watch_config" <<'PY'
import json, sys
with open(sys.argv[1], encoding="utf-8") as stream:
    candidate = json.load(stream)
with open(sys.argv[2], encoding="utf-8") as stream:
    existing = json.load(stream)
stable = (
    "rpcUrl", "rpcUser", "rpcPassword", "coordinatorUrl", "backendToken",
    "payoutAddress", "windowWorkUnits", "cursorPath", "pollIntervalSeconds",
)
if any(candidate.get(key) != existing.get(key) for key in stable):
    raise SystemExit("Existing BTC watcher config differs; no file or service was changed.")
if type(existing.get("startHeight")) is not int or existing["startHeight"] < 0:
    raise SystemExit("Existing BTC watcher startHeight is invalid.")
print("Existing BTC watcher config and original start height retained.")
PY
else
  install -m 600 "$watch_candidate" "$watch_config"
fi
chmod 600 "$ckpool_config" "$ingest_config" "$watch_config"
mkdir -p "$share_logs"
chmod 700 "$share_logs"

# CKPool validates the new instance before any new systemd service is installed.
"$ckpool_bin" -T -n bdlcollective \
  -c "$ckpool_config" -L -s "$runtime_directory" \
  >"$ckpool_verify_log" 2>&1 || {
    echo 'CKPool rejected collective.conf. Inspect the private local validation log:' >&2
    echo "  $ckpool_verify_log" >&2
    exit 1
  }

for port in 3337 3338; do
  if ss -H -ltn "sport = :$port" | grep -q .; then
    echo "TCP port $port is already occupied. Existing services were left untouched." >&2
    exit 1
  fi
done
if sudo systemctl is-active --quiet "$ckpool_service" \
  || sudo systemctl is-active --quiet "$ingest_service" \
  || sudo systemctl is-active --quiet "$watch_service"; then
  echo 'A collective BTC service is already running. Stop it explicitly before changing these units.' >&2
  exit 1
fi

cat >"$temporary/ckpool.service" <<EOF
[Unit]
Description=BDL BTC collective CKPool (separate pool-mode instance)
Wants=network-online.target
After=network-online.target

[Service]
Type=simple
User=bdl
Group=bdl
WorkingDirectory=/
ExecStart=$ckpool_bin -n bdlcollective -c $ckpool_config -L -s $runtime_directory
Restart=on-failure
RestartSec=5
UMask=0007
RuntimeDirectory=bdl-pool-ckpool-collective
RuntimeDirectoryMode=0750
NoNewPrivileges=true
PrivateTmp=true
PrivateDevices=true
ProtectSystem=strict
ProtectHome=tmpfs
BindReadOnlyPaths=$ckpool_bin
BindReadOnlyPaths=$ckpool_config
BindPaths=$share_logs
ReadWritePaths=$runtime_directory
RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX
MemoryMax=256M
TasksMax=64
LimitNOFILE=512

[Install]
WantedBy=multi-user.target
EOF

cat >"$temporary/collective-ingest.service" <<EOF
[Unit]
Description=BDL BTC Stratum V1 collective auth proxy and accepted-share collector
Requires=$ckpool_service bdl-pool-collective.service
After=$ckpool_service bdl-pool-collective.service network-online.target

[Service]
Type=simple
User=bdl
Group=bdl
WorkingDirectory=/
ExecStart=$service_bin collective-ingest --config $ingest_config
Restart=on-failure
RestartSec=5
UMask=0077
ReadWritePaths=$state_directory
NoNewPrivileges=true
PrivateTmp=true
PrivateDevices=true
ProtectSystem=strict
ProtectHome=tmpfs
BindReadOnlyPaths=$service_bin
BindReadOnlyPaths=$ingest_config
BindReadOnlyPaths=$share_logs
RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX
MemoryMax=96M
TasksMax=32
LimitNOFILE=128

[Install]
WantedBy=multi-user.target
EOF

cat >"$temporary/collective-btc-watch.service" <<EOF
[Unit]
Description=BDL BTC collective Bitcoin Core maturity watcher
Requires=$ckpool_service $ingest_service bdl-pool-collective.service
After=$ckpool_service $ingest_service bdl-pool-collective.service network-online.target

[Service]
Type=simple
User=bdl
Group=bdl
WorkingDirectory=/
ExecStart=$service_bin collective-btc-watch --config $watch_config
Restart=on-failure
RestartSec=10
UMask=0077
StateDirectory=bdl-pool-collective
StateDirectoryMode=0700
NoNewPrivileges=true
PrivateTmp=true
PrivateDevices=true
ProtectSystem=strict
ProtectHome=tmpfs
BindReadOnlyPaths=$service_bin
BindReadOnlyPaths=$watch_config
ReadWritePaths=$state_directory
RestrictAddressFamilies=AF_INET AF_INET6
MemoryMax=64M
TasksMax=32
LimitNOFILE=128

[Install]
WantedBy=multi-user.target
EOF

sudo -v
sudo install -m 644 "$temporary/ckpool.service" "$ckpool_unit"
sudo install -m 644 "$temporary/collective-ingest.service" "$ingest_unit"
sudo install -m 644 "$temporary/collective-btc-watch.service" "$watch_unit"
sudo systemd-analyze verify "$ckpool_unit" "$ingest_unit" "$watch_unit"
sudo systemctl daemon-reload

# Start only the new collective services. Existing solo CKPool, Bitcoin Core,
# BCHN/CashStratum, site and public statistics services remain untouched.
sudo systemctl enable --now "$ckpool_service"
for _ in $(seq 1 30); do
  if ss -H -ltn 'sport = :3338' | grep -q .; then
    break
  fi
  if ! sudo systemctl is-active --quiet "$ckpool_service"; then
    sudo journalctl -u "$ckpool_service" -n 40 --no-pager
    exit 1
  fi
  sleep 1
done
ss -H -ltn 'sport = :3338' | grep -q . || {
  echo 'Collective CKPool did not open 127.0.0.1:3338.' >&2
  sudo journalctl -u "$ckpool_service" -n 40 --no-pager
  exit 1
}

sudo systemctl enable --now "$ingest_service"
for _ in $(seq 1 30); do
  if ss -H -ltn 'sport = :3337' | grep -q .; then
    break
  fi
  if ! sudo systemctl is-active --quiet "$ingest_service"; then
    sudo journalctl -u "$ingest_service" -n 40 --no-pager
    exit 1
  fi
  sleep 1
done
ss -H -ltn 'sport = :3337' | grep -q . || {
  echo 'Collective V1 auth proxy did not open 127.0.0.1:3337.' >&2
  sudo journalctl -u "$ingest_service" -n 40 --no-pager
  exit 1
}

sudo systemctl enable --now "$watch_service"
if ! sudo systemctl is-active --quiet "$watch_service"; then
  sudo journalctl -u "$watch_service" -n 40 --no-pager
  exit 1
fi

echo
echo 'BTC collective backend and block watcher installed.'
echo 'Stratum route: 127.0.0.1:3337 (authenticated V1 proxy)'
echo 'Private CKPool: 127.0.0.1:3338 (pool mode, common BTC payout address)'
echo 'Accepted shares: CKPool .sharelog entries with result=true'
echo "PPLNS window: $BDL_COLLECTIVE_PPLNS_WINDOW_WORK_UNITS work units"
echo 'Block watcher: local Bitcoin Core RPC, 100 confirmations, pool-address match'
echo "Watcher state: $block_cursor_path"
echo
echo 'The existing BTC solo CKPool service and its ports 3333/3336 were not restarted.'
echo 'Mature blocks create pending allocations only; transactions are not signed or broadcast.'
echo 'The OpenPort collective profile still needs a connectivity test and separate enablement.'
