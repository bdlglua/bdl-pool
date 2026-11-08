#!/usr/bin/env python3
"""Read the fixed CKPool `clients` IPC command; persist only filtered statistics."""
import argparse
import hashlib
import hmac
import json
import os
from pathlib import Path
import secrets
import socket
import struct
import tempfile
import time

from server import MAX_FILE, RATES, device_from_agent, epoch, number, worker_identity

MAX_IPC = 8 * 1024 * 1024
PERIODS = {'1m': 'dsps1', '5m': 'dsps5', '1h': 'dsps60', '1d': 'dsps1440', '7d': 'dsps10080'}


def clients_from_socket(path):
    deadline = time.monotonic() + 3
    def receive(sock, size):
        chunks = bytearray()
        while len(chunks) < size:
            remaining = deadline-time.monotonic()
            if remaining <= 0:
                raise TimeoutError('CKPool response deadline')
            sock.settimeout(remaining)
            chunk = sock.recv(min(65536, size-len(chunks)))
            if not chunk:
                raise ValueError('Truncated CKPool response')
            chunks.extend(chunk)
        return bytes(chunks)
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as sock:
        sock.settimeout(3)
        sock.connect(str(path))
        sock.sendall(struct.pack('<I', 7)+b'clients')
        size, = struct.unpack('<I', receive(sock, 4))
        if not 0 < size <= MAX_IPC:
            raise ValueError('Oversized CKPool response')
        data = json.loads(receive(sock, size))
    clients = data.get('clients') if isinstance(data, dict) else None
    if not isinstance(clients, list) or len(clients) > 2000:
        raise ValueError('Invalid CKPool clients')
    return clients


def filter_clients(clients, salt):
    result = []
    for client in clients:
        if not isinstance(client, dict) or client.get('authorised') is not True:
            continue
        identity = worker_identity(client.get('workername'))
        identifier = number(client.get('id'), True)
        if not identity or identifier is None:
            continue
        raw_id = f"{identifier}:{client.get('starttime')}:{client['workername']}".encode()
        rates = {}
        for period, key in PERIODS.items():
            value = number(client.get(key))
            rates[period] = number(value * 4294967296) if value is not None else None
        result.append(dict(id=hmac.new(salt, raw_id, hashlib.sha256).hexdigest()[:24],
                           address=identity[0], name=client['workername'],
                           device=device_from_agent(client.get('useragent')),
                           hashrateHps=rates, bestShare=number(client.get('bestdiff')),
                           connectedAt=epoch(client.get('starttime')), lastShareAt=epoch(client.get('lastshare'))))
    return result


def publish(path, data):
    encoded = json.dumps(data, ensure_ascii=False, allow_nan=False, separators=(',', ':')).encode()
    if len(encoded) > MAX_FILE:
        raise ValueError('Presence snapshot too large')
    descriptor, filename = tempfile.mkstemp(prefix='.presence-', dir=path.parent)
    try:
        with os.fdopen(descriptor, 'wb') as stream:
            stream.write(encoded)
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(filename, path)
    finally:
        Path(filename).unlink(missing_ok=True)


def collect(socket_path, output, salt, last_success=None):
    now = epoch(time.time())
    try:
        clients = filter_clients(clients_from_socket(socket_path), salt)
        data = dict(schemaVersion=1, checkedAt=now, updatedAt=now, available=True, clients=clients)
        publish(output, data)
        return now
    except (OSError, ValueError, TypeError, OverflowError):
        publish(output, dict(schemaVersion=1, checkedAt=now, updatedAt=last_success, available=False, clients=[]))
        return last_success


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--config', type=Path, default=Path.home()/'.config/bdl-pool-api/config.json')
    parser.add_argument('--once', action='store_true')
    args = parser.parse_args()
    config = json.loads(args.config.read_text())
    output = Path(config.get('onlineSnapshot', '/var/lib/bdl-pool-presence/online.json'))
    socket_path = Path(config.get('ckpoolSocket', '/run/ckpool/stratifier'))
    salt, last_success = secrets.token_bytes(32), None
    while True:
        last_success = collect(socket_path, output, salt, last_success)
        if args.once:
            break
        time.sleep(15)


if __name__ == '__main__':
    main()
