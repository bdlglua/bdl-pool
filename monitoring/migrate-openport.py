#!/usr/bin/env python3
"""Move the pool's DuckDNS record to its prepared OpenPort relays, keeping HTTPS."""
import argparse
import fcntl
import getpass
import importlib.util
import ipaddress
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import time

spec = importlib.util.spec_from_file_location('duckdns', Path(__file__).with_name('duckdns-openport.py'))
dns = importlib.util.module_from_spec(spec)
spec.loader.exec_module(dns)
DOMAIN = 'bdl-pool.duckdns.org'
HOME_BDL = Path('/home/bdl')
STATE = HOME_BDL/'.local/state/bdl-pool-openport'
ROOT = Path('/etc/bdl-pool-openport')
HOOK = Path('/usr/local/libexec/bdl-pool-duckdns.py')
FRAGMENT = Path('/etc/caddy/conf.d/bdl-pool-api.caddy')
API_CONFIG = HOME_BDL/'.config/bdl-pool-api/config.json'
LOG = Path('/var/log/bdl-pool-openport-migration.log')


class MigrationError(Exception):
    pass


def run(command, label):
    print(label, flush=True)
    with LOG.open('ab') as log:
        result = subprocess.run(command, stdout=log, stderr=subprocess.STDOUT)
    if result.returncode:
        raise MigrationError(label+' a échoué. Journal local : '+str(LOG))


def routes():
    result = {}
    for name in ('v1', 'v2', 'api'):
        state = json.loads((STATE/name/'public.json').read_text())
        identity = json.loads((STATE/name/'identity.json').read_text())
        if (state.get('host') != 'openport.pw' or type(state.get('port')) is not int
                or not 1024 <= state['port'] <= 65535 or state['port'] != identity.get('remote_port')):
            raise MigrationError('Profil OpenPort '+name.upper()+' incohérent ; DNS conservé.')
        if subprocess.run(['systemctl', 'is-active', '--quiet', 'bdl-pool-openport@'+name+'.service']).returncode:
            raise MigrationError('Le relais '+name.upper()+' n’est pas actif ; DNS conservé.')
        result[name] = state['port']
    if len(set(result.values())) != 3:
        raise MigrationError('Les trois ports publics doivent être distincts.')
    return result


def reject_old_updaters():
    # The prior installer found none. Refuse a competing writer rather than
    # disabling unrelated DDNS services or exposing their command-line tokens.
    names = []
    for directory in (Path('/etc/systemd/system'), Path('/usr/lib/systemd/system')):
        for path in directory.glob('*'):
            if path.suffix not in ('.service', '.timer') or path.name.startswith('bdl-pool-duckdns.'):
                continue
            if ('duckdns' in path.name.lower() or 'ddns' in path.name.lower()) and not subprocess.run(
                    ['systemctl', 'is-active', '--quiet', path.name]).returncode:
                names.append(path.name)
    if names:
        raise MigrationError('Ancienne mise à jour DNS active à adapter : '+', '.join(sorted(set(names))))
    for user in ('bdl', 'root'):
        if shutil.which('crontab'):
            data = subprocess.run(['crontab', '-u', user, '-l'], capture_output=True, text=True).stdout
            if any(line.strip() and not line.lstrip().startswith('#') and ('duckdns' in line.lower() or 'ddns' in line.lower()) for line in data.splitlines()):
                raise MigrationError('Ancienne mise à jour DNS repérée dans la crontab '+user+'. Son contenu reste privé ; DNS conservé.')
    for path in (Path('/etc/crontab'), *Path('/etc/cron.d').glob('*')):
        if not path.is_file():
            continue
        try:
            lines = path.read_text().splitlines()
        except (OSError, UnicodeError):
            continue
        if any(line.strip() and not line.lstrip().startswith('#') and 'duckdns' in line.lower() for line in lines):
            raise MigrationError('Ancienne tâche DuckDNS repérée dans '+str(path)+'. DNS conservé.')


def config(port, reset=False):
    path = ROOT/'duckdns.json'
    saved = json.loads(path.read_text()) if path.exists() else {}
    token = saved.get('token', '')
    if reset or not token:
        token = getpass.getpass('Jeton DuckDNS (saisie masquée, ne pas envoyer dans le chat) : ').strip()
    if not re.fullmatch(r'[A-Za-z0-9-]{20,80}', token):
        raise MigrationError('Format du jeton DuckDNS invalide.')
    result = dict(saved, domain=DOMAIN, token=token, relayHost='openport.pw', apiPort=port)
    dns.private_write(path, (json.dumps(result)+'\n').encode())
    return result


def caddy_fragment():
    return DOMAIN+''' {
    tls /etc/caddy/bdl-pool-tls/current/fullchain.pem /etc/caddy/bdl-pool-tls/current/privkey.pem
    handle /api/v1/* {
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
'''


def install_dns_timer():
    Path('/etc/systemd/system/bdl-pool-duckdns.service').write_text('''[Unit]
Description=BDL Pool DNS points only to the verified OpenPort relay
Wants=network-online.target
After=network-online.target

[Service]
Type=oneshot
ExecStart=/usr/bin/python3 /usr/local/libexec/bdl-pool-duckdns.py sync
TimeoutStartSec=90
UMask=0077
NoNewPrivileges=true
PrivateTmp=true
PrivateDevices=true
ProtectHome=true
ProtectSystem=strict
RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX
''')
    Path('/etc/systemd/system/bdl-pool-duckdns.timer').write_text('''[Unit]
Description=Keep BDL Pool DNS on OpenPort every five minutes

[Timer]
OnBootSec=90s
OnUnitActiveSec=5min
Persistent=true

[Install]
WantedBy=timers.target
''')
    os.chmod('/etc/systemd/system/bdl-pool-duckdns.service', 0o644)
    os.chmod('/etc/systemd/system/bdl-pool-duckdns.timer', 0o644)
    run(['systemctl', 'daemon-reload'], 'Chargement du suivi DNS')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--reset-token', action='store_true')
    args = parser.parse_args()
    if os.geteuid() != 0:
        parser.error('Lancer avec sudo python3, depuis le compte bdl.')
    if Path(__file__).resolve().parent != HOME_BDL/'bdl-pool-site/monitoring':
        parser.error('Le dépôt doit être dans /home/bdl/bdl-pool-site.')
    os.umask(0o077)
    ROOT.mkdir(exist_ok=True, mode=0o755)
    ROOT.chmod(0o755)
    LOG.touch(mode=0o600, exist_ok=True)
    LOG.chmod(0o600)
    try:
        with (ROOT/'migration.lock').open('a') as lock:
            try:
                fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except BlockingIOError:
                raise MigrationError('Une migration est déjà en cours.') from None
            ports = routes()
            reject_old_updaters()
            if DOMAIN not in FRAGMENT.read_text():
                raise MigrationError('Fragment Caddy du pool inattendu.')
            settings = config(ports['api'], args.reset_token)
            if not dns.health(settings):
                raise MigrationError('HTTPS n’est plus confirmé à travers OpenPort. DNS conservé.')
            if not shutil.which('certbot') or not shutil.which('dig'):
                run(['apt-get', 'update'], 'Préparation des paquets DNS et certificats')
                run(['apt-get', 'install', '-y', '--no-install-recommends', 'certbot', 'dnsutils'], 'Installation de Certbot et dig')
            HOOK.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(Path(__file__).with_name('duckdns-openport.py'), HOOK)
            HOOK.chmod(0o755)
            os.chown(HOOK, 0, 0)
            original = FRAGMENT.read_bytes()
            backup = ROOT/('caddy-before-'+time.strftime('%Y%m%dT%H%M%SZ', time.gmtime()))
            dns.private_write(backup, original)
            # DNS-only issuance and renewal do not depend on inbound ports.
            run(['certbot', 'certonly', '--non-interactive', '--agree-tos', '--register-unsafely-without-email',
                 '--cert-name', 'bdl-pool-openport', '--keep-until-expiring', '-d', DOMAIN,
                 '--manual', '--preferred-challenges', 'dns',
                 '--manual-auth-hook', '/usr/bin/python3 '+str(HOOK)+' auth',
                 '--manual-cleanup-hook', '/usr/bin/python3 '+str(HOOK)+' cleanup',
                 '--deploy-hook', '/usr/bin/python3 '+str(HOOK)+' deploy'], 'Obtention du certificat par validation DNS (quelques minutes possibles)')
            # Also stage the certificate when Certbot kept an existing lineage.
            run(['/usr/bin/python3', str(HOOK), 'deploy'], 'Préparation du certificat Caddy')
            run(['certbot', 'renew', '--cert-name', 'bdl-pool-openport', '--dry-run', '--no-random-sleep-on-renew'],
                'Vérification du renouvellement automatique par DNS')
            try:
                dns.private_write(FRAGMENT, caddy_fragment().encode(), 0o644)
                run(['runuser', '-u', 'caddy', '--', 'caddy', 'validate', '--config', '/etc/caddy/Caddyfile'], 'Validation de la configuration Caddy')
                run(['systemctl', 'reload', 'caddy.service'], 'Chargement du certificat dans Caddy')
                if not dns.health(settings):
                    raise MigrationError('Le certificat du relais n’est pas confirmé après rechargement.')
            except (MigrationError, OSError):
                dns.private_write(FRAGMENT, original, 0o644)
                subprocess.run(['systemctl', 'reload', 'caddy.service'], capture_output=True)
                raise
            old = API_CONFIG.stat()
            data = json.loads(API_CONFIG.read_text())
            if not (ROOT/'api-before.json').exists():
                dns.private_write(ROOT/'api-before.json', API_CONFIG.read_bytes())
            data.update(host=DOMAIN, v1Port=ports['v1'], v2Port=ports['v2'], apiPublicPort=ports['api'])
            dns.private_write(API_CONFIG, (json.dumps(data, indent=2)+'\n').encode())
            os.chown(API_CONFIG, old.st_uid, old.st_gid)
            run(['systemctl', 'restart', 'bdl-pool-api.service'], 'Actualisation des adresses Stratum dans l’API')
            if not dns.health(settings):
                raise MigrationError('L’API ne répond pas après sa mise à jour. DNS conservé.')
            install_dns_timer()
            run(['systemctl', 'enable', '--now', 'certbot.timer'], 'Activation du renouvellement des certificats')
            public = dict(domain=DOMAIN, v1Port=ports['v1'], v2Port=ports['v2'], apiPublicPort=ports['api'], managedTLS=True)
            # Persist the working certificate mode before DNS. A future API
            # reinstall must never revert to port-based ACME challenges.
            dns.private_write(ROOT/'public.json', (json.dumps(public, indent=2)+'\n').encode(), 0o644)
            if not (ROOT/'dns-before.json').exists():
                previous = dict(A=dns.dig(DOMAIN, 'A'), AAAA=dns.dig(DOMAIN, 'AAAA'))
                dns.private_write(ROOT/'dns-before.json', (json.dumps(previous)+'\n').encode())
            # Enable repair before the first DNS write; a temporary provider
            # failure must never cause a fallback to the residential address.
            run(['systemctl', 'enable', '--now', 'bdl-pool-duckdns.timer'], 'Activation de la mise à jour DNS vers le relais')
            try:
                dns.sync(settings, first=not (ROOT/'dns-migrated').exists())
            except dns.DNSError:
                print('Certificat et relais préparés. Le timer DNS réessaiera la bascule ; vérifier son résultat avant de modifier le site.', flush=True)
                raise
            dns.private_write(ROOT/'dns-migrated', b'OpenPort\n')
            print('Migration DNS et HTTPS terminée.', flush=True)
            print('V1 : '+DOMAIN+':'+str(ports['v1']), flush=True)
            print('V2 : '+DOMAIN+':'+str(ports['v2']), flush=True)
            print('API : https://'+DOMAIN+':'+str(ports['api'])+'/api/v1/health', flush=True)
            print('Renouvellement DNS du certificat : test réussi et timer actif.', flush=True)
            print('Les caches DNS peuvent retarder la visibilité du changement.', flush=True)
            print('Bitcoin Core et CKPool n’ont pas été redémarrés.', flush=True)
        return 0
    except (MigrationError, dns.DNSError) as error:
        print(str(error), flush=True)
        return 1
    except (OSError, ValueError, KeyError):
        print('Migration interrompue. État précédent et journaux conservés dans /etc/bdl-pool-openport et '+str(LOG), flush=True)
        return 1
    except KeyboardInterrupt:
        print('\nMigration interrompue ; les identifiants restent sur le serveur.', flush=True)
        return 130


if __name__ == '__main__':
    raise SystemExit(main())
