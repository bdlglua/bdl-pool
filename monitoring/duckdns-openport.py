#!/usr/bin/env python3
"""Root-only DuckDNS hooks and relay DNS updates. Never log credential URLs."""
import argparse
import grp
import http.client
import ipaddress
import json
import os
from pathlib import Path
import re
import secrets
import shlex
import socket
import ssl
import subprocess
import tempfile
import time
from urllib.parse import urlencode
from urllib.request import HTTPRedirectHandler, Request, build_opener

CONFIG = Path('/etc/bdl-pool-openport/duckdns.json')
LINEAGE = Path('/etc/letsencrypt/live/bdl-pool-openport')
TLS = Path('/etc/caddy/bdl-pool-tls')


class DNSError(Exception):
    pass


class NoRedirect(HTTPRedirectHandler):
    def redirect_request(self, *_):
        return None


def settings():
    data = json.loads(CONFIG.read_text())
    if (not isinstance(data, dict) or data.get('domain') != 'bdl-pool.duckdns.org'
            or not isinstance(data.get('token'), str)
            or not re.fullmatch(r'[A-Za-z0-9-]{20,80}', data['token'])
            or data.get('relayHost') != 'openport.pw'
            or type(data.get('apiPort')) is not int or not 1 <= data['apiPort'] <= 65535):
        raise DNSError('Configuration DNS invalide.')
    return data


def update(config, **values):
    query = dict(domains=config['domain'].removesuffix('.duckdns.org'), token=config['token'], **values)
    request = Request('https://www.duckdns.org/update?'+urlencode(query), headers={'User-Agent': 'BDL-Pool-DNS'})
    try:
        with build_opener(NoRedirect()).open(request, timeout=12) as response:
            result = response.read(4096).decode().splitlines()
    except (OSError, ValueError):
        raise DNSError('DuckDNS est temporairement inaccessible.') from None
    if not result or result[0].strip() != 'OK':
        raise DNSError('DuckDNS refuse la mise à jour. Vérifier le jeton et le domaine sur le serveur.')
    return result


def dig(name, kind, server=None):
    command = ['dig', '+time=2', '+tries=1', '+short']
    if server:
        command.append('@'+server)
    command.extend([name, kind])
    try:
        result = subprocess.run(command, check=True, capture_output=True, text=True, timeout=5)
    except (OSError, subprocess.SubprocessError):
        raise DNSError('Lecture DNS indisponible.') from None
    return result.stdout.splitlines()


def authoritative_servers():
    servers = []
    for name in dig('duckdns.org', 'NS'):
        if not re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9.-]{0,252}', name):  # Never a dig option.
            continue
        for value in dig(name, 'A'):
            try:
                address = ipaddress.ip_address(value)
                if address.version == 4 and address.is_global:
                    servers.append(str(address))
            except ValueError:
                pass
    if not servers:
        raise DNSError('Serveurs DNS DuckDNS introuvables.')
    return list(dict.fromkeys(servers))[:12]


def wait_txt(config, validation, maximum=240):
    servers = authoritative_servers()
    deadline = time.monotonic()+maximum
    while time.monotonic() < deadline:
        confirmed = 0
        for server in servers:
            try:
                records = [''.join(shlex.split(line)) for line in dig('_acme-challenge.'+config['domain'], 'TXT', server)]
                confirmed += validation in records
            except (DNSError, ValueError):
                pass
        if confirmed == len(servers):
            return
        time.sleep(4)
    raise DNSError('Le TXT de validation n’est pas encore visible sur tous les serveurs DuckDNS.')


def challenge(config, cleanup=False):
    if os.environ.get('CERTBOT_DOMAIN') != config['domain']:
        raise DNSError('Domaine de certificat inattendu.')
    value = os.environ.get('CERTBOT_VALIDATION', '')
    if not re.fullmatch(r'[A-Za-z0-9_-]{20,200}', value):
        raise DNSError('Validation de certificat inattendue.')
    if cleanup:
        # Presence of txt keeps this in the TXT API: never clear A/AAAA here.
        update(config, txt=value, clear='true')
        return
    update(config, txt=value)
    print('Validation DNS : attente de propagation du TXT.', flush=True)
    wait_txt(config, value)
    print('Validation DNS : TXT confirmé.', flush=True)


def health(config, address=None):
    try:
        with socket.create_connection((address or config['relayHost'], config['apiPort']), timeout=5) as raw:
            with ssl.create_default_context().wrap_socket(raw, server_hostname=config['domain']) as tls:
                tls.sendall(('GET /api/v1/health HTTP/1.1\r\nHost: '+config['domain']+'\r\nConnection: close\r\n\r\n').encode())
                response = http.client.HTTPResponse(tls)
                response.begin()
                data = json.loads(response.read(4096))
                return response.status == 200 and isinstance(data, dict) and data.get('status') == 'ok'
    except (OSError, ValueError, http.client.HTTPException):
        return False


def wait_health(config, maximum=30):
    deadline = time.monotonic()+maximum
    while time.monotonic() < deadline:
        if health(config):
            return True
        time.sleep(.5)
    return False


def relay_ipv4(config):
    try:
        values = socket.getaddrinfo(config['relayHost'], config['apiPort'], socket.AF_INET, socket.SOCK_STREAM)
    except OSError:
        raise DNSError('Résolution du relais indisponible.') from None
    addresses = list(dict.fromkeys(row[4][0] for row in values))
    for value in addresses[:4]:
        if ipaddress.ip_address(value).is_global and health(config, value):
            return value
    raise DNSError('API HTTPS non confirmée à travers le relais. DNS conservé.')


def sync(config, first=False):
    address = relay_ipv4(config)
    if first:
        # DuckDNS has no separate AAAA-delete operation. Clear once, then set
        # an explicit relay IPv4; never use its source-IP autodetection.
        update(config, clear='true')
    result = update(config, ip=address, verbose='true')
    if len(result) < 3:
        raise DNSError('DuckDNS n’a pas confirmé les enregistrements.')
    if result[2].strip():
        update(config, clear='true')
        result = update(config, ip=address, verbose='true')
    if len(result) < 3 or result[1].strip() != address or result[2].strip():
        raise DNSError('Les enregistrements DuckDNS ne correspondent pas encore au relais.')
    print('DuckDNS : IPv4 du relais enregistrée, aucun enregistrement IPv6.', flush=True)


def private_write(path, contents, mode=0o600, group=None):
    descriptor, temporary = tempfile.mkstemp(prefix='.'+path.name+'-', dir=path.parent)
    try:
        with os.fdopen(descriptor, 'wb') as stream:
            stream.write(contents)
            os.fchmod(stream.fileno(), mode)
            if group is not None:
                os.fchown(stream.fileno(), 0, group)
        os.replace(temporary, path)
    finally:
        Path(temporary).unlink(missing_ok=True)


def deploy_certificate():
    if Path(os.environ.get('RENEWED_LINEAGE', str(LINEAGE))) != LINEAGE:
        raise DNSError('Lignée de certificat inattendue.')
    certificate, key = LINEAGE/'fullchain.pem', LINEAGE/'privkey.pem'
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    context.load_cert_chain(certificate, key)
    import hashlib
    release = TLS/'releases'/hashlib.sha256(certificate.read_bytes()).hexdigest()[:20]
    group = grp.getgrnam('caddy').gr_gid
    for directory in (TLS, TLS/'releases', release):
        directory.mkdir(exist_ok=True, mode=0o750)
        directory.chmod(0o750)
        os.chown(directory, 0, group)
    private_write(release/'fullchain.pem', certificate.read_bytes(), 0o640, group)
    private_write(release/'privkey.pem', key.read_bytes(), 0o640, group)
    current = TLS/'current'
    previous = os.readlink(current) if current.is_symlink() else None
    temporary = TLS/('.current-'+secrets.token_hex(8))
    try:
        os.symlink('releases/'+release.name, temporary)
        os.replace(temporary, current)
        validate = subprocess.run(['runuser', '-u', 'caddy', '--', 'caddy', 'validate', '--config', '/etc/caddy/Caddyfile'],
                                  capture_output=True, timeout=30)
        if validate.returncode:
            raise DNSError('Caddy refuse le certificat déployé.')
        subprocess.run(['systemctl', 'reload', 'caddy.service'], check=True, capture_output=True, timeout=30)
    except (OSError, subprocess.SubprocessError, DNSError):
        if previous:
            temporary.unlink(missing_ok=True)
            os.symlink(previous, temporary)
            os.replace(temporary, current)
        elif current.is_symlink():
            current.unlink()
        raise DNSError('Déploiement du certificat refusé ; version précédente conservée.') from None
    finally:
        temporary.unlink(missing_ok=True)
    print('Certificat HTTPS déployé dans Caddy.', flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=('auth', 'cleanup', 'sync', 'deploy'))
    args = parser.parse_args()
    if os.geteuid() != 0:
        parser.error('Cette commande est réservée aux services root.')
    try:
        if args.action == 'deploy':
            deploy_certificate()
        else:
            config = settings()
            if args.action == 'sync':
                sync(config)
            else:
                challenge(config, args.action == 'cleanup')
        return 0
    except DNSError as error:
        print(str(error), flush=True)
        return 1
    except (OSError, ValueError, KeyError):
        print('Opération DNS/HTTPS interrompue. Vérifier la configuration privée du serveur.', flush=True)
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
