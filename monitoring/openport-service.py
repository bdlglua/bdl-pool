#!/usr/bin/env python3
"""Persistent OpenPort forwarding; private credentials never enter the journal."""
import argparse
from collections import deque
import fcntl
import http.client
import importlib.util
import json
import os
from pathlib import Path
import re
import signal
import socket
import ssl
import subprocess
import threading
import time
from urllib.error import HTTPError, URLError
from urllib.request import urlopen

spec = importlib.util.spec_from_file_location('openport_client', Path(__file__).with_name('try-openport.py'))
client = importlib.util.module_from_spec(spec)
spec.loader.exec_module(client)
PORTS = {'v1': 3333, 'v2': 3336, 'api': 443, 'bch': 3334}


def local_api_healthy(host):
    context = ssl.create_default_context()
    with socket.create_connection(('127.0.0.1', 443), timeout=2) as raw:
        with context.wrap_socket(raw, server_hostname=host) as tls:
            tls.sendall(('GET /api/v1/health HTTP/1.1\r\nHost: '+host+'\r\nConnection: close\r\n\r\n').encode())
            response = http.client.HTTPResponse(tls)
            response.begin()
            body = json.loads(response.read(1024))
            return response.status == 200 and isinstance(body, dict) and body.get('status') == 'ok'


def api_transport(public, manifest=Path('/etc/bdl-pool-openport/public.json')):
    """None: local/config/certificate issue; False: only the public path failed."""
    try:
        settings = json.loads(manifest.read_text())
        if not isinstance(settings, dict):
            return None
        host = settings.get('apiDomain', settings.get('domain', ''))
        if (not isinstance(host, str) or not re.fullmatch(r'[a-z0-9](?:[a-z0-9.-]{0,251}[a-z0-9])?', host)
                or '.' not in host or settings.get('managedTLS') is not True
                or settings.get('apiPublicPort') != public['port']):
            return None
        if not local_api_healthy(host):
            return None
    except (OSError, ValueError, TypeError, KeyError, http.client.HTTPException):
        return None
    try:
        with urlopen('https://'+host+':'+str(public['port'])+'/api/v1/health', timeout=4):
            return True
    except HTTPError:
        # Any HTTP response proves the tunnel works. Restarting a relay does
        # not repair a backend error, certificate expiry or Caddy routes.
        return True
    except URLError as error:
        return None if isinstance(error.reason, ssl.SSLCertVerificationError) else False
    except ssl.SSLCertVerificationError:
        return None
    except OSError:
        return False


class APIHealth:
    def __init__(self):
        self.failures = 0

    def reconnect(self, public):
        result = api_transport(public)
        self.failures = self.failures+1 if result is False else 0
        return self.failures >= 3


def serve(profile, binary, local_port, stop):
    """Each restart re-confirms the saved address. No identity is replaced."""
    identity = process = reader = None
    tail = deque(maxlen=64)
    try:
        if not (profile/'identity.json').is_file():
            raise client.TrialError('Identité absente ; relancer l’installation pour conserver les ports.')
        identity = client.credentials(profile)
        public = client.connect(identity, local_port, profile)
        process = subprocess.Popen([str(binary), str(profile/'client.toml')], stdout=subprocess.PIPE,
                                   stderr=subprocess.STDOUT, text=True, errors='replace')
        def capture():
            for line in iter(lambda: process.stdout.readline(1000), ''):
                tail.append(line)
        reader = threading.Thread(target=capture, daemon=True)
        reader.start()
        print('Relais configuré : '+client.endpoint(public['host'], public['port'])+'.', flush=True)
        heartbeat = time.monotonic()+15
        health = APIHealth() if profile.name == 'api' else None
        probe_at = time.monotonic()+90
        failures = 0
        while not stop.wait(.5):
            if process.poll() is not None:
                raise client.TrialError('Client arrêté ; nouvelle tentative par systemd.')
            if time.monotonic() >= heartbeat:
                try:
                    reply = client.api('heartbeat', {key: identity[key] for key in ('client_id', 'token')})
                    if reply.get('status') not in (None, 'ok'):
                        raise client.TrialError('Maintien refusé.')
                    failures = 0
                except client.TrialError:
                    failures += 1
                    if failures >= 3:
                        raise client.TrialError('Maintien indisponible ; nouvelle tentative par systemd.') from None
                heartbeat = time.monotonic()+15
            if health and time.monotonic() >= probe_at:
                if health.reconnect(public):
                    print('Accès public du relais API indisponible ; reconnexion.', flush=True)
                    raise client.TrialError('Relais API sans accès public.')
                probe_at = time.monotonic()+60
    finally:
        if process and process.poll() is None:
            process.terminate()
            try:
                process.wait(timeout=3)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
        if reader:
            reader.join(timeout=1)
            process.stdout.close()
            try:
                descriptor = os.open(profile/'client.log', os.O_WRONLY | os.O_CREAT | os.O_TRUNC | os.O_NOFOLLOW, 0o600)
                with os.fdopen(descriptor, 'w') as log:
                    os.fchmod(log.fileno(), 0o600)
                    log.write(''.join(tail))
            except OSError:
                pass
        if identity:
            try:
                client.api('release', {key: identity[key] for key in ('client_id', 'token')})
            except client.TrialError:
                pass
        (profile/'client.toml').unlink(missing_ok=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('profile', choices=PORTS)
    args = parser.parse_args()
    stop = threading.Event()
    signal.signal(signal.SIGTERM, lambda *_: stop.set())
    signal.signal(signal.SIGINT, lambda *_: stop.set())
    profile = Path.home()/'.local/state/bdl-pool-openport'/args.profile
    try:
        with (profile/'trial.lock').open('a') as lock:
            try:
                fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except BlockingIOError:
                raise client.TrialError('Ce profil est déjà utilisé par un essai ou un service.') from None
            serve(profile, client.verified_binary(), PORTS[args.profile], stop)
        return 0
    except (client.TrialError, OSError):
        # Provider responses and subprocess logs stay private; no tokens in journal.
        print('Relais indisponible. Identité conservée ; nouvelle tentative par systemd.', flush=True)
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
