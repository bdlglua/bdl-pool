#!/usr/bin/env bash
# Prepare persistent relays. DNS, router, Caddy and pool services stay untouched.
set -euo pipefail
umask 077
[[ "$(id -un)" == bdl && "$HOME" == /home/bdl ]] || { echo 'Lancer avec le compte bdl, sans sudo.' >&2; exit 1; }
script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
[[ "$script_dir" == /home/bdl/bdl-pool-site/monitoring ]] || { echo 'Le dépôt doit être dans /home/bdl/bdl-pool-site.' >&2; exit 1; }
for tool in python3 sudo systemctl flock; do
  command -v "$tool" >/dev/null || { echo "Outil absent : $tool" >&2; exit 1; }
done
state_dir="$HOME/.local/state/bdl-pool-openport"
mkdir -p "$state_dir"
chmod 700 "$state_dir"
exec 9>"$state_dir/install.lock"
flock -n 9 || { echo 'Une installation OpenPort est déjà en cours.' >&2; exit 1; }
profiles=(v1 v2 api collective)
if (($#)); then
  profiles=("$@")
fi
for profile in "${profiles[@]}"; do
  case "$profile" in v1|v2|api|bch|collective) ;; *)
    echo "Profil OpenPort inconnu : $profile" >&2
    exit 1
    ;;
  esac
done

temporary="$(mktemp -d)"
trap 'rm -rf -- "$temporary"' EXIT
python3 - "$script_dir" "$temporary/bdl-pool-openport@.service" "${profiles[@]}" <<'PY'
import fcntl, importlib.util, json, sys
from pathlib import Path
source, destination = Path(sys.argv[1]), Path(sys.argv[2])
selected = sys.argv[3:] or ['v1', 'v2', 'api', 'collective']
spec = importlib.util.spec_from_file_location('client', source/'try-openport.py')
client = importlib.util.module_from_spec(spec); spec.loader.exec_module(client)
root = Path.home()/'.local/state/bdl-pool-openport'
client.verified_binary()
identities, ports = set(), set()
all_profiles = ('v1', 'v2', 'api', 'bch', 'collective')
for name in all_profiles:
    profile = root/name
    identity_path = profile/'identity.json'
    if name in selected:
        profile.mkdir(mode=0o700, parents=True, exist_ok=True); profile.chmod(0o700)
        active = __import__('subprocess').run(
            ['systemctl', 'is-active', '--quiet', 'bdl-pool-openport@'+name+'.service']
        ).returncode == 0
        if not active:
            with (profile/'trial.lock').open('a') as lock:
                try:
                    fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
                except BlockingIOError:
                    raise SystemExit('Un essai '+name.upper()+' est encore actif. Attendre son arrêt.')
        identity = client.credentials(profile)
    elif identity_path.is_file():
        try:
            identity = json.loads(identity_path.read_text())
        except (OSError, ValueError):
            raise SystemExit('Identité OpenPort '+name.upper()+' invalide ; aucun port ne sera réattribué.') from None
    else:
        continue
    if (
        not isinstance(identity.get('client_id'), str)
        or not identity['client_id']
        or type(identity.get('remote_port')) is not int
        or not 1024 <= identity['remote_port'] <= 65535
    ):
        raise SystemExit('Identité/port OpenPort invalide pour '+name.upper()+'.')
    if identity['client_id'] in identities or identity['remote_port'] in ports:
        raise SystemExit('OpenPort a attribué un identifiant ou port déjà utilisé. Aucun relais n’a été démarré.')
    identities.add(identity['client_id']); ports.add(identity['remote_port'])
    if name in selected:
        print(name.upper()+': identité sauvegardée, port '+str(identity['remote_port'])+'.')
destination.write_text('''[Unit]
Description=BDL Pool OpenPort relay %i
Wants=network-online.target
After=network-online.target
StartLimitIntervalSec=0

[Service]
Type=simple
User=bdl
Group=bdl
Environment=PYTHONDONTWRITEBYTECODE=1
WorkingDirectory=/home/bdl/bdl-pool-site/monitoring
ExecStart=/usr/bin/python3 /home/bdl/bdl-pool-site/monitoring/openport-service.py %i
Restart=on-failure
RestartSec=20
TimeoutStopSec=25
UMask=0077
NoNewPrivileges=true
PrivateTmp=true
PrivateDevices=true
ProtectSystem=strict
ProtectHome=tmpfs
BindReadOnlyPaths=/home/bdl/bdl-pool-site/monitoring
BindReadOnlyPaths=/home/bdl/.local/share/bdl-pool-openport
BindPaths=/home/bdl/.local/state/bdl-pool-openport/%i
RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX
MemoryMax=128M
TasksMax=256
LimitNOFILE=1024

[Install]
WantedBy=multi-user.target
''')
PY
sudo -v
sudo install -m 644 "$temporary/bdl-pool-openport@.service" /etc/systemd/system/bdl-pool-openport@.service
sudo systemctl daemon-reload
for profile in "${profiles[@]}"; do
  service="bdl-pool-openport@$profile.service"
  sudo systemctl enable "$service"
  if sudo systemctl is-active --quiet "$service"; then
    echo "$profile : already running; not restarted."
  else
    sudo systemctl start "$service"
  fi
done
python3 - "$HOME/.local/state/bdl-pool-openport" "${profiles[@]}" <<'PY'
from pathlib import Path
import http.client, json, socket, ssl, sys, time
root = Path(sys.argv[1])
profiles = sys.argv[2:]
host = 'bdl-pool.duckdns.org'
routes = {}
for name in profiles:
    try:
        data = json.loads((root/name/'public.json').read_text())
        if isinstance(data.get('host'), str) and type(data.get('port')) is int:
            routes[name] = data
            print(name.upper()+' : '+data['host']+':'+str(data['port']))
        else:
            print(name.upper()+' : route non confirmée.')
    except (OSError, ValueError):
        print(name.upper()+' : route non confirmée. Vérifier systemctl status bdl-pool-openport@'+name+'.')
if 'api' in profiles and 'api' in routes:
    route = routes['api']
    confirmed = False
    for _ in range(5):
        try:
            context = ssl.create_default_context()
            with socket.create_connection((route['host'], route['port']), timeout=4) as raw:
                with context.wrap_socket(raw, server_hostname=host) as tls:
                    tls.sendall(('GET /api/v1/health HTTP/1.1\\r\\nHost: '+host+'\\r\\nConnection: close\\r\\n\\r\\n').encode())
                    response = http.client.HTTPResponse(tls); response.begin()
                    data = json.loads(response.read(4096))
                    confirmed = response.status == 200 and data.get('status') == 'ok'
            if confirmed:
                break
        except (OSError, ValueError, http.client.HTTPException):
            time.sleep(1)
    print('API HTTPS à travers OpenPort : '+('validée.' if confirmed else 'non confirmée.'))
print('Les profils déjà actifs n’ont pas été redémarrés.')
PY
