#!/usr/bin/env python3
"""Temporary OpenPort.pw trial with a saved identity and no pool/DNS changes."""
import argparse
import fcntl
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import ipaddress
import json
import os
from pathlib import Path
import platform
import re
import secrets
import signal
import socket
import subprocess
import tempfile
import threading
import time
from urllib.error import HTTPError
from urllib.request import HTTPRedirectHandler, Request, build_opener, urlopen

PROVIDER = 'https://openport.pw'
# Digests published at https://openport.pw/checksums.sha256, verified 2026-11-08.
BUILDS = {
    'x86_64': 'ce70860e6929265f7a63c618679aa5b7aff036f12ae50f49171252bcf44b72b1',
    'aarch64': '63f63648f952c2cc2928946e33d1e9463d8a63b28e15f57274516cbf1d0a71d4',
}
MAX_BINARY = 16*1024*1024


class TrialError(Exception):
    pass


class NoRedirect(HTTPRedirectHandler):
    def redirect_request(self, *_):
        return None


def api(action, data):
    # Credentials are sent only to the provider, never followed to another host.
    request = Request(PROVIDER+'/api/'+action, data=json.dumps(data).encode(),
                      headers={'Content-Type': 'application/json', 'User-Agent': 'BDL-Pool-OpenPort-Trial'})
    try:
        with build_opener(NoRedirect()).open(request, timeout=8) as response:
            encoded = response.read(65537)
        if len(encoded) > 65536:
            raise ValueError('Oversized response')
        result = json.loads(encoded)
    except HTTPError as error:
        raise TrialError(f'OpenPort refuse la requête {action} (HTTP {error.code}).') from None
    except (OSError, ValueError):
        raise TrialError(f'Réponse OpenPort indisponible pour {action}.') from None
    if not isinstance(result, dict):
        raise TrialError('Réponse OpenPort inattendue.')
    return result


def private_json(path, data):
    descriptor, temporary = tempfile.mkstemp(prefix='.'+path.name+'-', dir=path.parent)
    try:
        with os.fdopen(descriptor, 'w') as stream:
            json.dump(data, stream)
            stream.write('\n')
        os.replace(temporary, path)
    finally:
        Path(temporary).unlink(missing_ok=True)


def valid_port(value):
    return type(value) is int and 1 <= value <= 65535


def credentials(profile):
    path = profile/'identity.json'
    saved = None
    if path.exists():
        try:
            if path.is_symlink():
                raise ValueError('Symlink')
            saved = json.loads(path.read_text())
            if not isinstance(saved, dict):
                raise ValueError('Invalid identity')
            validate_identity(saved)
            path.chmod(0o600)
        except (ValueError, OSError, TrialError):
            raise TrialError('Identité locale illisible. Aucun nouveau port n’a été demandé ; conserver identity.json pour garder le port.') from None
    payload = {key: saved[key] for key in ('client_id', 'token')} if saved else {}
    result = api('register', payload)
    if saved:
        if result.get('status') != 'ok' or result.get('remote_port') != saved['remote_port']:
            raise TrialError('Le port précédent n’est pas confirmé par OpenPort. Identité conservée, essai interrompu.')
        print(f'Port précédent confirmé : {saved["remote_port"]}.', flush=True)
        return saved
    identity = {key: result.get(key) for key in ('client_id', 'token', 'remote_port')}
    validate_identity(identity)
    private_json(path, identity)
    print(f'Port attribué : {identity["remote_port"]}. Identité conservée sur ce serveur.', flush=True)
    return identity


def validate_identity(identity):
    if (not isinstance(identity.get('client_id'), str)
            or not re.fullmatch(r'[A-Za-z0-9_-]{1,128}', identity['client_id'])
            or not isinstance(identity.get('token'), str)
            or not re.fullmatch(r'[A-Za-z0-9._~+/=-]{8,512}', identity['token'])
            or not valid_port(identity.get('remote_port'))):
        raise TrialError('Identité OpenPort inattendue.')


def relay_host(value):
    if not isinstance(value, str):
        raise TrialError('Adresse du relais inattendue.')
    host = value.lower().rstrip('.')
    if host == 'openport.pw' or (host.endswith('.openport.pw') and re.fullmatch(r'[a-z0-9.-]{1,253}', host)):
        return host
    try:
        address = ipaddress.ip_address(host)
        if address.is_global:
            return host
    except ValueError:
        pass
    raise TrialError('Adresse du relais inattendue.')


def endpoint(host, port):
    return ('['+host+']' if ':' in host else host)+':'+str(port)


def verified_binary():
    machine = platform.machine().lower()
    machine = {'amd64': 'x86_64', 'arm64': 'aarch64'}.get(machine, machine)
    if platform.system() != 'Linux' or machine not in BUILDS:
        raise TrialError('Ce script prend en charge Linux ARM64 et x86-64 uniquement.')
    digest = BUILDS[machine]
    directory = Path.home()/'.local/share/bdl-pool-openport'
    directory.mkdir(parents=True, exist_ok=True, mode=0o700)
    binary = directory/('openport-core-'+machine+'-'+digest[:12])
    if binary.is_file() and binary.stat().st_size <= MAX_BINARY and hashlib.sha256(binary.read_bytes()).hexdigest() == digest:
        return binary
    print(f'Téléchargement du client OpenPort ({machine})…', flush=True)
    with urlopen(PROVIDER+'/client/openport-core-'+machine, timeout=30) as response:
        data = response.read(MAX_BINARY+1)
    if len(data) > MAX_BINARY or hashlib.sha256(data).hexdigest() != digest:
        raise TrialError('Client OpenPort refusé : empreinte SHA-256 différente de la version vérifiée.')
    descriptor, temporary = tempfile.mkstemp(prefix='.core-', dir=directory)
    try:
        with os.fdopen(descriptor, 'wb') as stream:
            stream.write(data)
            os.fchmod(stream.fileno(), 0o700)
        os.replace(temporary, binary)
    finally:
        Path(temporary).unlink(missing_ok=True)
    return binary


def test_server():
    completed = threading.Event()
    path = '/bdl-openport-test/'+secrets.token_hex(12)
    class Handler(BaseHTTPRequestHandler):
        def setup(self):
            super().setup()
            self.connection.settimeout(2)

        def do_GET(self):
            if self.path != path:
                self.send_error(404)
                return
            body = b'BDL-OPENPORT-OK\n'
            self.send_response(200)
            self.send_header('Content-Type', 'text/plain')
            self.send_header('Content-Length', str(len(body)))
            self.send_header('Cache-Control', 'no-store')
            self.end_headers()
            self.wfile.write(body)
            self.wfile.flush()
            completed.set()

        def log_message(self, *_):
            pass
    server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
    threading.Thread(target=lambda: server.serve_forever(poll_interval=.2), daemon=True).start()
    return server, completed, path


def connect(identity, local_port, profile):
    auth = {key: identity[key] for key in ('client_id', 'token')}
    response = api('connect', dict(auth, protocol='tcp', local_port=local_port))
    if response.get('status') != 'ok' or not valid_port(response.get('control_port')):
        raise TrialError('OpenPort n’a pas confirmé la préparation du relais.')
    host = relay_host(response.get('server_host') or 'openport.pw')
    if 'remote_port' in response and response['remote_port'] != identity['remote_port']:
        raise TrialError('OpenPort a changé le port annoncé. Essai interrompu.')
    route_file = profile/'public.json'
    public = dict(host=host, port=identity['remote_port'])
    if route_file.exists():
        try:
            previous = json.loads(route_file.read_text())
        except (ValueError, OSError):
            raise TrialError('Adresse précédente illisible ; essai interrompu pour conserver la configuration.') from None
        if previous != public:
            raise TrialError('L’adresse du relais a changé. Configuration précédente conservée, essai interrompu.')
    private_json(route_file, public)
    config = ('[client]\nremote_addr = '+json.dumps(endpoint(host, response['control_port']))+'\nheartbeat_timeout = 40\n'
              '[client.transport]\ntype = "tcp"\n[client.transport.tcp]\nnodelay = true\nkeepalive_secs = 20\nkeepalive_interval = 5\n'
              '[client.services.'+identity['client_id']+']\ntype = "tcp"\ntoken = '+json.dumps(identity['token'])+'\n'
              'local_addr = "127.0.0.1:'+str(local_port)+'"\nnodelay = true\n')
    config_file = profile/'client.toml'
    descriptor = os.open(config_file, os.O_WRONLY | os.O_CREAT | os.O_TRUNC | os.O_NOFOLLOW, 0o600)
    with os.fdopen(descriptor, 'w') as stream:
        stream.write(config)
        os.fchmod(stream.fileno(), 0o600)
    return public


def run_trial(binary, mode, profile, duration):
    server = process = heartbeat = identity = None
    stop = threading.Event()
    completed, path = threading.Event(), ''
    try:
        if mode == 'test':
            server, completed, path = test_server()
            port = server.server_port
            print('Test du tunnel uniquement : aucune connexion de minage.', flush=True)
        else:
            port = 3333 if mode == 'v1' else 3336
            try:
                with socket.create_connection(('127.0.0.1', port), timeout=2):
                    pass
            except OSError:
                raise TrialError(f'CKPool ne répond pas sur le port {port}. Le mode test reste utilisable pendant la synchronisation.') from None
        identity = credentials(profile)
        public = connect(identity, port, profile)
        def keep_alive():
            while not stop.wait(15):
                try:
                    api('heartbeat', {key: identity[key] for key in ('client_id', 'token')})
                except TrialError:
                    pass
        heartbeat = threading.Thread(target=keep_alive, daemon=True)
        heartbeat.start()
        descriptor = os.open(profile/'client.log', os.O_WRONLY | os.O_CREAT | os.O_TRUNC | os.O_NOFOLLOW, 0o600)
        with os.fdopen(descriptor, 'w') as log:
            os.fchmod(log.fileno(), 0o600)
            process = subprocess.Popen([str(binary), str(profile/'client.toml')], stdout=log, stderr=subprocess.STDOUT)
        address = endpoint(public['host'], public['port'])
        print(f'Relais attribué : {address} — en attente de validation externe.', flush=True)
        if mode == 'test':
            print(f'URL de test : http://{address}{path}', flush=True)
        print(f'Laissez ce terminal ouvert. Arrêt automatique après {duration} secondes ou avec Ctrl+C.', flush=True)
        started = time.monotonic()
        while time.monotonic()-started < duration:
            if completed.is_set():
                time.sleep(.5)
                print('Requête de test reçue. Tunnel de test arrêté ; identité et port conservés.', flush=True)
                return 0
            if process.poll() is not None:
                raise TrialError('Le client OpenPort s’est arrêté. Son journal reste local dans '+str(profile/'client.log'))
            stop.wait(.2)
        print('Durée de l’essai écoulée. Tunnel arrêté ; identité conservée.', flush=True)
        return 1 if mode == 'test' else 0
    finally:
        stop.set()
        if heartbeat:
            heartbeat.join(timeout=9)
        if process and process.poll() is None:
            process.terminate()
            try:
                process.wait(timeout=3)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
        if identity:
            try:
                api('release', {key: identity[key] for key in ('client_id', 'token')})
            except TrialError:
                pass
        (profile/'client.toml').unlink(missing_ok=True)
        if server:
            server.shutdown()
            server.server_close()


def interrupt(*_):
    raise KeyboardInterrupt


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('mode', nargs='?', choices=('test', 'v1', 'v2'), default='test')
    parser.add_argument('--profile', choices=('v1', 'v2'), default=None)
    parser.add_argument('--timeout', type=int, default=600)
    args = parser.parse_args()
    if not 1 <= args.timeout <= 600:
        parser.error('Durée : 1 à 600 secondes.')
    if os.geteuid() == 0:
        parser.error('Lancez avec le compte bdl, sans sudo.')
    if args.mode != 'test' and args.profile not in (None, args.mode):
        parser.error('Le profil doit correspondre au protocole pour conserver son port.')
    signal.signal(signal.SIGTERM, interrupt)
    name = args.profile or ('v1' if args.mode == 'test' else args.mode)
    profile = Path.home()/'.local/state/bdl-pool-openport'/name
    profile.mkdir(parents=True, exist_ok=True, mode=0o700)
    profile.chmod(0o700)
    try:
        with (profile/'trial.lock').open('a') as lock:
            try:
                fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except BlockingIOError:
                raise TrialError('Un essai utilise déjà ce profil ; attendre son arrêt.') from None
            return run_trial(verified_binary(), args.mode, profile, args.timeout)
    except KeyboardInterrupt:
        print('\nEssai arrêté ; identité conservée.', flush=True)
        return 130
    except (TrialError, OSError) as error:
        print(f'Essai impossible : {error}', flush=True)
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
