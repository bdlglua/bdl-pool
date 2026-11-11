#!/usr/bin/env python3
"""Use dedicated Stratum/API hosts so bdl-pool.com can serve GitHub Pages."""
import argparse
import fcntl
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

SITE_DOMAIN = 'bdl-pool.com'
DOMAIN = 'stratum.bdl-pool.com'
API_DOMAIN = 'api.bdl-pool.com'
OLD_DOMAIN = 'bdl-pool.duckdns.org'
TARGET = '_acme-challenge.'+OLD_DOMAIN
HOME = Path('/home/bdl')
ROOT = Path('/etc/bdl-pool-openport')
HELPER = Path('/usr/local/libexec/bdl-pool-duckdns.py')
HOOK = Path('/usr/local/libexec/bdl-pool-domain.py')
LINEAGE = Path('/etc/letsencrypt/live/bdl-pool-domain')
TLS = Path('/etc/caddy/bdl-pool-domain-tls')
FRAGMENT = Path('/etc/caddy/conf.d/bdl-pool-api.caddy')
LEGACY = Path('/etc/caddy/conf.d/bdl-pool-legacy-api.caddy')
API_CONFIG = HOME/'.config/bdl-pool-api/config.json'
LOG = Path('/var/log/bdl-pool-domain-migration.log')


class DomainError(Exception):
    pass


def load_dns():
    spec = importlib.util.spec_from_file_location('pool_domain_dns', HELPER)
    dns = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(dns)
    return dns


def run(command, label):
    print(label, flush=True)
    with LOG.open('ab') as log:
        result = subprocess.run(command, stdout=log, stderr=subprocess.STDOUT)
    if result.returncode:
        raise DomainError(label+' a échoué. Journal privé : '+str(LOG))


def nameservers(dns):
    result = []
    for name in dns.dig(SITE_DOMAIN, 'NS'):
        if not re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9.-]{0,252}', name):  # Never a dig option.
            continue
        for address in dns.dig(name, 'A'):
            try:
                value = ipaddress.ip_address(address)
                if value.version == 4 and value.is_global:
                    result.append(str(value))
            except ValueError:
                pass
    if not result:
        raise DomainError('Serveurs DNS de '+SITE_DOMAIN+' introuvables. Attendre la propagation DNS.')
    return list(dict.fromkeys(result))[:12]


def check_delegation(dns):
    # Check every authoritative server, not a cached TXT from another hostname.
    for server in nameservers(dns):
        records = [value.lower().rstrip('.') for value in dns.dig('_acme-challenge.'+API_DOMAIN, 'CNAME', server)]
        if records != [TARGET]:
            raise DomainError('CNAME manquant : _acme-challenge.api → '+TARGET+'. Attendre la propagation DNS.')


def check_addresses(dns, settings):
    address = dns.relay_ipv4(settings)  # Includes a verified TLS/health probe.
    for server in nameservers(dns):
        for host in (DOMAIN, API_DOMAIN):
            aliases = [v.lower().rstrip('.') for v in dns.dig(host, 'CNAME', server)]
            if aliases != ['openport.pw']:
                raise DomainError('CNAME manquant : '+host+' → openport.pw.')
            addresses = []
            for value in dns.dig(host, 'A', server):
                try:
                    parsed = ipaddress.ip_address(value)
                    if parsed.version == 4:
                        addresses.append(str(parsed))
                except ValueError:
                    pass  # dig may also print the CNAME followed by its A record.
            if not addresses:
                # An authoritative server returns the alias but may not recurse
                # into openport.pw. Check the alias there, then its public answer.
                aliases = [v.lower().rstrip('.') for v in dns.dig(host, 'CNAME', server)]
                if aliases == ['openport.pw']:
                    addresses = [v for v in dns.dig(host, 'A') if v == address]
            if set(addresses) != {address}:
                raise DomainError(host+' ne pointe pas encore uniquement vers le relais '+address+'.')
            for value in dns.dig(host, 'AAAA', server):
                try:
                    ipaddress.IPv6Address(value)
                except ValueError:
                    continue
                raise DomainError('Enregistrement IPv6 inattendu pour '+host+'. Vérifier son DNS.')
    check_delegation(dns)


def challenge(dns, cleanup=False):
    if os.environ.get('CERTBOT_DOMAIN') != API_DOMAIN:
        raise DomainError('Domaine de certificat inattendu.')
    value = os.environ.get('CERTBOT_VALIDATION', '')
    if not re.fullmatch(r'[A-Za-z0-9_-]{20,200}', value):
        raise DomainError('Validation de certificat inattendue.')
    settings = dns.settings()
    if cleanup:
        dns.update(settings, txt=value, clear='true')  # TXT API only: keep A/AAAA.
    else:
        check_delegation(dns)
        dns.update(settings, txt=value)
        print('Attente de la validation DNS du certificat.', flush=True)
        dns.wait_txt(settings, value)


def deploy(dns):
    if Path(os.environ.get('RENEWED_LINEAGE', str(LINEAGE))) != LINEAGE:
        raise DomainError('Lignée de certificat inattendue.')
    proof = subprocess.run(['openssl', 'x509', '-in', str(LINEAGE/'fullchain.pem'),
                            '-noout', '-checkhost', API_DOMAIN], capture_output=True, timeout=10)
    if proof.returncode:
        raise DomainError('Le certificat ne couvre pas '+API_DOMAIN+'.')
    # Reuse atomic deployment/rollback, with a separate directory. Renewing the
    # old DuckDNS certificate cannot overwrite the new domain's certificate.
    dns.LINEAGE, dns.TLS = LINEAGE, TLS
    dns.deploy_certificate()


def site(host, directory):
    extra = '    import /etc/caddy/bdlminer-routes/*.caddy\n' if host == API_DOMAIN else ''
    return host+' {\n    tls '+str(directory)+'/current/fullchain.pem '+str(directory)+'/current/privkey.pem\n'+extra+'''    handle /api/v1/* {
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


def routes():
    result = {}
    for name in ('v1', 'v2', 'api'):
        profile = HOME/'.local/state/bdl-pool-openport'/name
        public = json.loads((profile/'public.json').read_text())
        identity = json.loads((profile/'identity.json').read_text())
        if (public.get('host') != 'openport.pw' or type(public.get('port')) is not int
                or not 1024 <= public['port'] <= 65535 or public['port'] != identity.get('remote_port')):
            raise DomainError('Profil OpenPort '+name.upper()+' incohérent.')
        if subprocess.run(['systemctl', 'is-active', '--quiet', 'bdl-pool-openport@'+name+'.service']).returncode:
            raise DomainError('Relais '+name.upper()+' inactif.')
        result[name] = public['port']
    if len(set(result.values())) != 3:
        raise DomainError('Les trois ports OpenPort doivent être distincts.')
    return result


def activate(dns, ports, previous):
    originals = {path: path.read_bytes() if path.exists() else None
                 for path in (FRAGMENT, LEGACY, API_CONFIG, ROOT/'public.json')}
    owner = API_CONFIG.stat()
    if originals[LEGACY] is not None and originals[LEGACY] != site(OLD_DOMAIN, Path('/etc/caddy/bdl-pool-tls')).encode():
        raise DomainError('Le fragment Caddy de compatibilité existe déjà avec une autre configuration.')
    current = site(API_DOMAIN, TLS)
    before_miner = current.replace('    import /etc/caddy/bdlminer-routes/*.caddy\n', '')
    if originals[FRAGMENT] not in (site(OLD_DOMAIN, Path('/etc/caddy/bdl-pool-tls')).encode(), current.encode(), before_miner.encode()):
        raise DomainError('Le fragment Caddy a été personnalisé. Configuration conservée.')
    backup = ROOT/('domain-before-'+str(time.time_ns()))
    backup.mkdir(mode=0o700)
    for index, (path, content) in enumerate(originals.items()):
        if content is not None:
            dns.private_write(backup/(str(index)+'-'+path.name), content)
    previous_domain = previous.get('domain') if previous.get('domain') != DOMAIN else previous.get('previousDomain', SITE_DOMAIN)
    public = dict(previous, domain=DOMAIN, siteDomain=SITE_DOMAIN, apiDomain=API_DOMAIN, previousDomain=previous_domain,
                  v1Port=ports['v1'], v2Port=ports['v2'], apiPublicPort=ports['api'],
                  managedTLS=True, tlsDirectory=str(TLS))
    health = dict(domain=API_DOMAIN, relayHost='openport.pw', apiPort=ports['api'])
    try:
        dns.private_write(LEGACY, site(OLD_DOMAIN, Path('/etc/caddy/bdl-pool-tls')).encode(), 0o644)
        dns.private_write(FRAGMENT, site(API_DOMAIN, TLS).encode(), 0o644)
        run(['runuser', '-u', 'caddy', '--', 'caddy', 'validate', '--config', '/etc/caddy/Caddyfile'], 'Validation de Caddy')
        run(['systemctl', 'reload', 'caddy.service'], 'Chargement du domaine de l’API')
        if not dns.wait_health(health):
            raise DomainError('HTTPS du nouveau domaine non confirmé.')
        settings = json.loads(originals[API_CONFIG])
        settings.update(host=DOMAIN, apiHost=API_DOMAIN, v1Port=ports['v1'], v2Port=ports['v2'], apiPublicPort=ports['api'])
        dns.private_write(API_CONFIG, (json.dumps(settings, indent=2)+'\n').encode())
        os.chown(API_CONFIG, owner.st_uid, owner.st_gid)
        run(['systemctl', 'restart', 'bdl-pool-api.service'], 'Actualisation des adresses publiques dans l’API')
        if not dns.wait_health(health) or not dns.wait_health(dict(health, domain=OLD_DOMAIN)):
            raise DomainError('API non confirmée après redémarrage.')
        dns.private_write(ROOT/'public.json', (json.dumps(public, indent=2)+'\n').encode(), 0o644)
    except (DomainError, dns.DNSError, OSError, ValueError, subprocess.SubprocessError):
        print('Restauration de la configuration précédente.', flush=True)
        for path, content in originals.items():
            if content is None:
                path.unlink(missing_ok=True)
            else:
                dns.private_write(path, content, 0o644 if path != API_CONFIG else 0o600)
        os.chown(API_CONFIG, owner.st_uid, owner.st_gid)
        failed = False
        for command in (['systemctl', 'reload', 'caddy.service'], ['systemctl', 'restart', 'bdl-pool-api.service']):
            failed |= subprocess.run(command, capture_output=True).returncode != 0
        if failed:
            raise DomainError('Fichiers restaurés, mais un service n’a pas redémarré. Vérifier Caddy et bdl-pool-api.') from None
        raise


def install(dns):
    for command in ('certbot', 'dig', 'caddy', 'openssl', 'runuser', 'systemctl'):
        if not shutil.which(command):
            raise DomainError('Outil absent : '+command+'. Installer d’abord la migration OpenPort existante.')
    ports = routes()
    previous = json.loads((ROOT/'public.json').read_text())
    if previous.get('domain') not in (OLD_DOMAIN, SITE_DOMAIN, DOMAIN) or previous.get('managedTLS') is not True:
        raise DomainError('La migration OpenPort HTTPS précédente doit être terminée.')
    settings = dns.settings()
    if settings['apiPort'] != ports['api']:
        raise DomainError('Le port de l’API ne correspond pas au relais installé.')
    check_addresses(dns, settings)
    print('DNS du pool, alias de l’API et délégation du certificat : validés.', flush=True)
    dns.private_write(HOOK, Path(__file__).read_bytes(), 0o755)
    os.chown(HOOK, 0, 0)
    command = ['certbot', 'certonly', '--non-interactive', '--agree-tos', '--register-unsafely-without-email',
               '--cert-name', 'bdl-pool-domain', '--keep-until-expiring', '-d', API_DOMAIN,
               '--manual', '--preferred-challenges', 'dns',
               '--manual-auth-hook', '/usr/bin/python3 '+str(HOOK)+' auth',
               '--manual-cleanup-hook', '/usr/bin/python3 '+str(HOOK)+' cleanup',
               '--deploy-hook', '/usr/bin/python3 '+str(HOOK)+' deploy']
    run(command, 'Obtention du certificat HTTPS par validation DNS')
    run(['/usr/bin/python3', str(HOOK), 'deploy'], 'Préparation du certificat dans Caddy')
    run(['certbot', 'renew', '--cert-name', 'bdl-pool-domain', '--dry-run', '--no-random-sleep-on-renew'],
        'Test du renouvellement automatique du certificat')
    run(['systemctl', 'enable', '--now', 'certbot.timer'], 'Activation du renouvellement automatique')
    activate(dns, ports, previous)
    print('Domaine et HTTPS configurés.', flush=True)
    print('V1 : '+DOMAIN+':'+str(ports['v1']), flush=True)
    print('V2 : '+DOMAIN+':'+str(ports['v2']), flush=True)
    print('API : https://'+API_DOMAIN+':'+str(ports['api'])+'/api/v1/health', flush=True)
    print('L’ancienne API reste accessible. Bitcoin Core et CKPool n’ont pas été redémarrés.', flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', nargs='?', default='install', choices=('install', 'auth', 'cleanup', 'deploy'))
    args = parser.parse_args()
    if os.geteuid() != 0:
        parser.error('Lancer avec sudo python3 depuis le compte bdl.')
    os.umask(0o077)
    try:
        dns = load_dns()
    except (OSError, ImportError, AttributeError):
        print('Installer d’abord les hooks DNS de la migration OpenPort existante.', flush=True)
        return 1
    try:
        if args.action == 'install':
            if Path(__file__).resolve() != HOME/'bdl-pool-site/monitoring/custom-domain.py':
                raise DomainError('Lancer le script depuis /home/bdl/bdl-pool-site/monitoring/custom-domain.py.')
            LOG.touch(mode=0o600, exist_ok=True)
            LOG.chmod(0o600)
            with (ROOT/'migration.lock').open('a') as lock:
                try:
                    fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
                except BlockingIOError:
                    raise DomainError('Une migration est déjà en cours.') from None
                install(dns)
        elif args.action == 'deploy':
            deploy(dns)
        else:
            challenge(dns, args.action == 'cleanup')
        return 0
    except (DomainError, dns.DNSError) as error:
        print(str(error), flush=True)
    except (OSError, ValueError, KeyError, subprocess.SubprocessError):
        print('Configuration interrompue. Vérifier le journal privé : '+str(LOG), flush=True)
    return 1


if __name__ == '__main__':
    raise SystemExit(main())
