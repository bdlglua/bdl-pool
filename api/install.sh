#!/usr/bin/env bash
# Run as bdl on the pool server; sudo is used only for packages and services.
set -euo pipefail
umask 077
[[ "$(id -un)" == bdl && "$HOME" == /home/bdl ]] || { echo 'Lancer avec le compte bdl, sans sudo.' >&2; exit 1; }
script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
[[ "$script_dir" == /home/bdl/bdl-pool-site/api ]] || { echo 'Le dépôt doit être dans /home/bdl/bdl-pool-site.' >&2; exit 1; }
for tool in python3 sudo systemctl flock ss; do
  command -v "$tool" >/dev/null || { echo "Outil absent : $tool" >&2; exit 1; }
done
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
python3 - <<'PY'
import subprocess
for port, unit in ((8081, 'bdl-pool-api.service'), (80, 'caddy.service'), (443, 'caddy.service')):
    if subprocess.run(['systemctl', 'is-active', '--quiet', unit]).returncode == 0:
        continue
    probe = subprocess.run(['ss', '-H', '-ltn', f'sport = :{port}'], capture_output=True, text=True)
    if probe.returncode:
        raise SystemExit(f'Impossible de vérifier le port {port} avec ss. Installation interrompue.')
    if probe.stdout.strip():
        raise SystemExit(f'Le port {port} est occupé par un autre service. Installation interrompue.')
PY
temporary="$(mktemp -d)"
trap 'rm -rf -- "$temporary"' EXIT
mkdir -p "$HOME/.config/bdl-pool-api"
python3 - "$script_dir" "$HOME/.config/bdl-pool-api/config.json" "$temporary/site.caddy" <<'PY'
from pathlib import Path
import json, os, re, sys
script, destination, caddy = map(Path, sys.argv[1:])
home = Path('/home/bdl')
text = (script.parent/'config.js').read_text()
match = re.search(r'publicHost\s*:\s*"([a-z0-9.-]+)"', text)
host = match[1] if match else ''
if not re.fullmatch(r'[a-z0-9](?:[a-z0-9.-]{0,251}[a-z0-9])?', host) or '.' not in host:
    raise SystemExit('Nom de domaine public invalide dans config.js.')
source = home/'mining/ckpool/local.conf'
config, _ = json.JSONDecoder().raw_decode(source.read_text().lstrip())
logs = Path(config.get('logdir') or 'logs')
if not logs.is_absolute():
    logs = source.parent/logs
logs = logs.resolve()
if not logs.is_relative_to(home) or logs == home or any(c in str(logs) for c in '\n\r"\\:'):
    raise SystemExit('Dossier de statistiques CKPool inattendu.')
logs.mkdir(parents=True, exist_ok=True)
settings = dict(snapshot=str(home/'.local/state/bdl-pool-monitor/repository/status.json'),
                logs=str(logs), sv2Public=str(home/'.local/state/bdl-pool-monitor/sv2-public.json'),
                onlineSnapshot='/var/lib/bdl-pool-presence/online.json',
                ckpoolSocket='/run/ckpool/stratifier',
                host=host, v1Port=3333, v2Port=3336)
tls_line = ''
api_host = host
relay_file = Path('/etc/bdl-pool-openport/public.json')
if relay_file.exists():
    relay = json.loads(relay_file.read_text())
    domain = relay.get('domain', '')
    if (not isinstance(domain, str) or not re.fullmatch(r'[a-z0-9](?:[a-z0-9.-]{0,251}[a-z0-9])?', domain)
            or host not in (domain, relay.get('previousDomain')) or relay.get('managedTLS') is not True
            or any(type(relay.get(key)) is not int or not 1 <= relay[key] <= 65535
                   for key in ('v1Port', 'v2Port', 'apiPublicPort'))):
        raise SystemExit('Configuration publique OpenPort invalide ; configuration HTTPS conservée.')
    api_host = relay.get('apiDomain', domain)
    directory = relay.get('tlsDirectory', '/etc/caddy/bdl-pool-tls')
    if (not isinstance(api_host, str) or not re.fullmatch(r'[a-z0-9](?:[a-z0-9.-]{0,251}[a-z0-9])?', api_host)
            or directory not in ('/etc/caddy/bdl-pool-tls', '/etc/caddy/bdl-pool-domain-tls')):
        raise SystemExit('Configuration HTTPS publique invalide ; configuration précédente conservée.')
    settings.update(host=domain, apiHost=api_host, v1Port=relay['v1Port'], v2Port=relay['v2Port'], apiPublicPort=relay['apiPublicPort'])
    tls_line = '    tls '+directory+'/current/fullchain.pem '+directory+'/current/privkey.pem\n'
staged = destination.with_suffix('.tmp')
staged.write_text(json.dumps(settings, indent=2)+'\n')
staged.chmod(0o600)
os.replace(staged, destination)
caddy.write_text(api_host+' {\n'+tls_line+'''    handle /api/v1/* {
        reverse_proxy 127.0.0.1:8081 {
            transport http {
                dial_timeout 3s
                response_header_timeout 10s
            }
        }
    }
    handle {
        redir https://bdlglua.github.io/bdl-pool/ 302
    }
}
''')
# The API process sees only its code, its public configuration and statistics.
unit = '''[Unit]
Description=BDL Pool read-only public API
After=network.target

[Service]
Type=simple
User=bdl
Group=bdl
Environment=PYTHONDONTWRITEBYTECODE=1
WorkingDirectory=/home/bdl/bdl-pool-site/api
ExecStart=/usr/bin/python3 /home/bdl/bdl-pool-site/api/server.py --config /home/bdl/.config/bdl-pool-api/config.json
Restart=on-failure
RestartSec=5
UMask=0077
NoNewPrivileges=true
PrivateTmp=true
PrivateDevices=true
ProtectSystem=strict
ProtectHome=tmpfs
BindReadOnlyPaths=/home/bdl/bdl-pool-site/api
BindReadOnlyPaths=/home/bdl/.config/bdl-pool-api
BindReadOnlyPaths=/home/bdl/.local/state/bdl-pool-monitor/repository
BindReadOnlyPaths=-/home/bdl/.local/state/bdl-pool-monitor/sv2-public.json
BindReadOnlyPaths="'''+str(logs)+'''"
RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX
MemoryMax=192M
TasksMax=48
LimitNOFILE=256

[Install]
WantedBy=multi-user.target
'''
caddy.with_name('bdl-pool-api.service').write_text(unit)
caddy.with_name('bdl-pool-presence.service').write_text('''[Unit]
Description=BDL Pool filtered live connection statistics
After=network.target

[Service]
Type=simple
User=bdl
Group=bdl
Environment=PYTHONDONTWRITEBYTECODE=1
WorkingDirectory=/home/bdl/bdl-pool-site/api
ExecStart=/usr/bin/python3 /home/bdl/bdl-pool-site/api/presence.py --config /home/bdl/.config/bdl-pool-api/config.json
Restart=on-failure
RestartSec=5
StateDirectory=bdl-pool-presence
StateDirectoryMode=0700
UMask=0077
NoNewPrivileges=true
PrivateTmp=true
PrivateDevices=true
ProtectSystem=strict
ProtectHome=tmpfs
BindReadOnlyPaths=/home/bdl/bdl-pool-site/api
BindReadOnlyPaths=/home/bdl/.config/bdl-pool-api
RestrictAddressFamilies=AF_UNIX
MemoryMax=128M
TasksMax=16

[Install]
WantedBy=multi-user.target
''')
PY
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
python3 - <<'PY'
import json, time
from urllib.request import urlopen
for attempt in range(10):
    try:
        with urlopen('http://127.0.0.1:8081/api/v1/health', timeout=2) as response:
            if json.load(response).get('status') == 'ok':
                print('API locale : ok.')
                break
    except (OSError, ValueError):
        time.sleep(.5)
else:
    raise SystemExit('L’API locale ne répond pas. Vérifier : journalctl -u bdl-pool-api -n 20 --no-pager')
PY
sudo install -d -m 755 /etc/caddy/conf.d
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
sudo python3 - <<'PY'
from pathlib import Path
path = Path('/etc/caddy/Caddyfile')
text = path.read_text()
line = 'import /etc/caddy/conf.d/*.caddy'
if line not in text.splitlines():
    path.write_text(text.rstrip()+'\n\n'+line+'\n')
PY
if ! sudo -u caddy caddy validate --config /etc/caddy/Caddyfile >"$state_dir/api-caddy-validation.log" 2>&1; then
  sudo cp -p -- "$backup" /etc/caddy/Caddyfile
  if "$had_fragment"; then sudo cp -p -- "${backup}.fragment" "$fragment"; else sudo rm -f -- "$fragment"; fi
  echo 'Configuration Caddy refusée ; configuration précédente restaurée.' >&2
  echo "Journal local : $state_dir/api-caddy-validation.log" >&2
  exit 1
fi
sudo systemctl enable --now caddy.service
sudo systemctl reload caddy.service
python3 - "$HOME/.config/bdl-pool-api/config.json" <<'PY'
import json, sys
settings = json.load(open(sys.argv[1]))
host = settings.get('apiHost', settings['host'])
if settings.get('apiPublicPort', 443) != 443:
    host += ':'+str(settings['apiPublicPort'])
    print('API installée. Certificat HTTPS renouvelé par validation DNS.')
else:
    print('API installée. Certificat HTTPS demandé automatiquement par Caddy.')
print('Test public : https://'+host+'/api/v1/health')
print('Les endpoints par adresse apparaissent après la première share enregistrée par CKPool.')
print('Présence et identification des appareils : collecte locale toutes les 15 secondes.')
print('Bitcoin Core et CKPool n’ont pas été redémarrés.')
PY
