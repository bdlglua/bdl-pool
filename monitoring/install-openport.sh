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
temporary="$(mktemp -d)"
trap 'rm -rf -- "$temporary"' EXIT
python3 - "$script_dir" "$temporary/bdl-pool-openport@.service" <<'PY'
import fcntl, importlib.util, subprocess, sys
from pathlib import Path
source, destination = map(Path, sys.argv[1:])
spec = importlib.util.spec_from_file_location('client', source/'try-openport.py')
client = importlib.util.module_from_spec(spec); spec.loader.exec_module(client)
root = Path.home()/'.local/state/bdl-pool-openport'
if not (root/'v1/identity.json').is_file() or not (root/'v1/public.json').is_file():
    raise SystemExit('Effectuer d’abord le test OpenPort V1 pour garder son port.')
# Do not reserve new profiles while a test still holds a profile lock.
for name in ('v1', 'v2', 'api'):
    if not (root/name).is_dir():
        continue
    active = subprocess.run(['systemctl', 'is-active', '--quiet', 'bdl-pool-openport@'+name+'.service']).returncode == 0
    if not active:
        with (root/name/'trial.lock').open('a') as lock:
            try:
                fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except BlockingIOError:
                raise SystemExit('Un essai '+name.upper()+' est encore actif. Attendre son arrêt, puis relancer l’installation.')
client.verified_binary()
identifiers, ports = set(), set()
for name in ('v1', 'v2', 'api'):
    profile = root/name; profile.mkdir(mode=0o700, exist_ok=True); profile.chmod(0o700)
    identity = client.credentials(profile)
    if identity['client_id'] in identifiers or identity['remote_port'] in ports:
        raise SystemExit('OpenPort n’a pas attribué trois ports distincts. Aucun nouveau service n’a été démarré.')
    identifiers.add(identity['client_id']); ports.add(identity['remote_port'])
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
for profile in v1 v2 api; do
  sudo systemctl enable "bdl-pool-openport@$profile.service"
  sudo systemctl restart "bdl-pool-openport@$profile.service"
done
python3 - "$HOME/.local/state/bdl-pool-openport" <<'PY'
from pathlib import Path
import http.client, json, socket, ssl, subprocess, sys, time
root = Path(sys.argv[1])
host = 'bdl-pool.duckdns.org'
deadline = time.monotonic()+40
routes = {}
while time.monotonic() < deadline:
    for name in ('v1', 'v2', 'api'):
        try:
            data = json.loads((root/name/'public.json').read_text())
            if isinstance(data.get('host'), str) and type(data.get('port')) is int:
                routes[name] = data
        except (OSError, ValueError):
            pass
    if len(routes) == 3:
        break
    time.sleep(.5)
for name in ('v1', 'v2', 'api'):
    if name in routes:
        route = routes[name]
        print(name.upper()+' : '+route['host']+':'+str(route['port']))
    else:
        print(name.upper()+' : adresse non confirmée. Vérifier systemctl status bdl-pool-openport@'+name+'.')
# Validate the current certificate with the real hostname, while connecting to
# the relay directly. This checks HTTPS before any DNS migration.
confirmed = False
if 'api' in routes:
    route = routes['api']
    for attempt in range(5):
        try:
            context = ssl.create_default_context()
            with socket.create_connection((route['host'], route['port']), timeout=4) as raw:
                with context.wrap_socket(raw, server_hostname=host) as tls:
                    tls.sendall(('GET /api/v1/health HTTP/1.1\r\nHost: '+host+'\r\nConnection: close\r\n\r\n').encode())
                    response = http.client.HTTPResponse(tls); response.begin()
                    data = json.loads(response.read(4096))
                    confirmed = response.status == 200 and data.get('status') == 'ok'
            if confirmed:
                break
        except (OSError, ValueError, http.client.HTTPException):
            time.sleep(1)
print('API HTTPS à travers OpenPort : '+('validée (certificat actuel).' if confirmed else 'non confirmée.'))
print('DNS à migrer après préparation du renouvellement HTTPS.')
# Print only updater names, never unit commands, cron contents or tokens.
found = set()
for directory in (Path('/etc/systemd/system'), Path('/usr/lib/systemd/system')):
    for path in directory.glob('*'):
        if path.suffix not in ('.service', '.timer'):
            continue
        if 'duckdns' in path.name.lower() or 'ddns' in path.name.lower():
            found.add(path.name)
        elif path.is_file():
            try:
                text = path.read_text()
                if 'duckdns.org/update' in text or ('duckdns' in text.lower() and 'bdl-pool' in text):
                    found.add(path.name)
            except (OSError, UnicodeError):
                pass
print('Mise à jour DuckDNS repérée : '+(', '.join(sorted(found)) if found else 'aucune unité systemd trouvée.'))
try:
    cron = subprocess.run(['crontab', '-l'], capture_output=True, text=True, timeout=3).stdout
    present = any(line.strip() and not line.lstrip().startswith('#') and ('duckdns' in line.lower() or 'ddns' in line.lower()) for line in cron.splitlines())
    print('Tâche cron DuckDNS : '+('repérée (contenu conservé localement).' if present else 'non repérée.'))
except (OSError, subprocess.TimeoutExpired):
    pass
print('Bitcoin Core et CKPool n’ont pas été redémarrés. Le site et DuckDNS gardent leurs adresses actuelles.')
PY
