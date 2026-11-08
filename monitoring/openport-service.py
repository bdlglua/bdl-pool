#!/usr/bin/env python3
"""Persistent OpenPort forwarding; private credentials never enter the journal."""
import argparse
from collections import deque
import fcntl
import importlib.util
import os
from pathlib import Path
import signal
import subprocess
import threading
import time

spec = importlib.util.spec_from_file_location('openport_client', Path(__file__).with_name('try-openport.py'))
client = importlib.util.module_from_spec(spec)
spec.loader.exec_module(client)
PORTS = {'v1': 3333, 'v2': 3336, 'api': 443}


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
