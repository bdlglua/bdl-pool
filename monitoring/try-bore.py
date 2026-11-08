#!/usr/bin/env python3
"""Temporary Bore trial. No DNS, router, CKPool or system service changes."""
import argparse
from collections import deque
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import io
import os
from pathlib import Path
import platform
import queue
import re
import secrets
import signal
import socket
import subprocess
import tarfile
import tempfile
import threading
import time
from urllib.request import urlopen

VERSION = '0.6.0'
# Official release archive digests, plus the digest of the contained binary.
BUILDS = {
    'x86_64': ('x86_64-unknown-linux-musl',
               'e484d1e3acba77169b773f31a5bfb34192d4b660f44a094a658a2522cd2270f7',
               '60548b7a145ba334981b19fda7cd0210d24a108a3d0dc113919b33fd2eaa90ab'),
    'aarch64': ('aarch64-unknown-linux-musl',
                'ffc4515f3617420b243758cf36ed6a63208d7dba76b2ec3e90d1f476a9742951',
                '348068ca74769635b91e9243d6a2befda516b74a7c692f14263513e9c675706a'),
}


def verified_binary():
    machine = platform.machine().lower()
    machine = {'amd64': 'x86_64', 'arm64': 'aarch64'}.get(machine, machine)
    if platform.system() != 'Linux' or machine not in BUILDS:
        raise ValueError('Ce script prend en charge Linux ARM64 et x86-64 uniquement.')
    target, archive_hash, binary_hash = BUILDS[machine]
    directory = Path.home()/'.local/share/bdl-pool-bore'/VERSION
    directory.mkdir(parents=True, exist_ok=True, mode=0o700)
    binary = directory/'bore'
    if binary.is_file() and binary.stat().st_size <= 8*1024*1024 and hashlib.sha256(binary.read_bytes()).hexdigest() == binary_hash:
        return binary
    name = f'bore-v{VERSION}-{target}.tar.gz'
    print(f'Téléchargement de Bore {VERSION} ({machine})…', flush=True)
    with urlopen(f'https://github.com/ekzhang/bore/releases/download/v{VERSION}/{name}', timeout=30) as response:
        data = response.read(4*1024*1024+1)
    if len(data) > 4*1024*1024 or hashlib.sha256(data).hexdigest() != archive_hash:
        raise ValueError('Archive Bore refusée : empreinte SHA-256 incorrecte.')
    with tarfile.open(fileobj=io.BytesIO(data), mode='r:gz') as archive:
        member = archive.getmember('bore')
        if not member.isfile() or not 0 < member.size <= 8*1024*1024:
            raise ValueError('Binaire Bore inattendu.')
        contents = archive.extractfile(member).read()
    if hashlib.sha256(contents).hexdigest() != binary_hash:
        raise ValueError('Binaire Bore refusé : empreinte SHA-256 incorrecte.')
    descriptor, filename = tempfile.mkstemp(prefix='.bore-', dir=directory)
    try:
        with os.fdopen(descriptor, 'wb') as stream:
            stream.write(contents)
            os.fchmod(stream.fileno(), 0o700)
        os.replace(filename, binary)
    finally:
        Path(filename).unlink(missing_ok=True)
    return binary


def test_server():
    completed = threading.Event()
    path = '/bdl-bore-test/'+secrets.token_hex(12)
    class Handler(BaseHTTPRequestHandler):
        def setup(self):
            super().setup()
            self.connection.settimeout(2)

        def do_GET(self):
            if self.path != path:
                self.send_error(404)
                return
            body = b'BDL-BORE-OK\n'
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
    thread = threading.Thread(target=lambda: server.serve_forever(poll_interval=.2), daemon=True)
    thread.start()
    return server, completed, path


def run_trial(binary, mode, duration, remote_port=0, relay='bore.pub'):
    server = process = reader = None
    completed, path = threading.Event(), ''
    try:
        if mode == 'test':
            server, completed, path = test_server()
            local_port = server.server_port
            print('Test du tunnel uniquement : aucune connexion de minage.', flush=True)
        else:
            local_port = 3333 if mode == 'v1' else 3336
            try:
                with socket.create_connection(('127.0.0.1', local_port), timeout=2):
                    pass
            except OSError:
                raise ValueError(f'CKPool ne répond pas sur le port {local_port}. Attendez son démarrage ; le mode test reste utilisable.')
        environment = dict(os.environ, RUST_LOG='bore_cli=info', NO_COLOR='1')
        environment.pop('BORE_SECRET', None)
        process = subprocess.Popen([str(binary), 'local', str(local_port), '--local-host', '127.0.0.1',
                                    '--to', relay, '--port', str(remote_port)],
                                   stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, env=environment)
        lines = queue.Queue(maxsize=64)
        def read_output():
            for line in process.stdout:
                try:
                    lines.put_nowait(re.sub(r'\x1b\[[0-9;]*m', '', line).strip()[:1000])
                except queue.Full:
                    pass
        reader = threading.Thread(target=read_output, daemon=True)
        reader.start()
        started, connected = time.monotonic(), False
        errors = deque(maxlen=3)
        while time.monotonic()-started < duration:
            if completed.is_set():
                time.sleep(.5)  # Allow the fixed reply to traverse both TCP streams.
                print('Requête de test reçue. Tunnel de test arrêté.', flush=True)
                return 0
            try:
                line = lines.get(timeout=.2)
                errors.append(line)
                match = re.search(r'listening at '+re.escape(relay)+r':(\d+)\b', line)
                if match and not connected:
                    connected = True
                    print(f'Relais {mode.upper()} : {relay}:{match[1]}', flush=True)
                    if mode == 'test':
                        print(f'URL de test : http://{relay}:{match[1]}{path}', flush=True)
                    print(f'Laissez ce terminal ouvert. Arrêt automatique après {duration} secondes ou avec Ctrl+C.', flush=True)
            except queue.Empty:
                if process.poll() is not None:
                    raise ValueError('Connexion Bore interrompue. '+ ' '.join(errors))
            if not connected and time.monotonic()-started > 20:
                raise ValueError('Bore ne répond pas. Vérifiez la sortie TCP vers bore.pub:7835. '+ ' '.join(errors))
        print('Durée de l’essai écoulée. Tunnel arrêté.', flush=True)
        return 1 if mode == 'test' else 0
    finally:
        if process is not None:
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
            if reader is not None:
                reader.join(timeout=1)
            process.stdout.close()
        if server is not None:
            server.shutdown()
            server.server_close()


def interrupt(*_):
    raise KeyboardInterrupt


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('mode', nargs='?', choices=('test', 'v1', 'v2'), default='test')
    parser.add_argument('--timeout', type=int, default=600)
    parser.add_argument('--remote-port', type=int, default=0)
    args = parser.parse_args()
    if not 1 <= args.timeout <= 600 or not (args.remote_port == 0 or 1024 <= args.remote_port <= 65535):
        parser.error('Durée : 1 à 600 secondes ; port distant : 0 ou 1024 à 65535.')
    if os.geteuid() == 0:
        parser.error('Lancez avec le compte bdl, sans sudo.')
    signal.signal(signal.SIGTERM, interrupt)
    try:
        return run_trial(verified_binary(), args.mode, args.timeout, args.remote_port)
    except KeyboardInterrupt:
        print('\nEssai arrêté.', flush=True)
        return 130
    except (OSError, ValueError, tarfile.TarError, KeyError) as error:
        print(f'Essai impossible : {error}', flush=True)
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
