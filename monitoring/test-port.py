#!/usr/bin/env python3
"""Temporary TCP reachability test. Does not serve files or start mining."""
import argparse
import socket
import sys
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--bind', default='0.0.0.0')
    parser.add_argument('--port', type=int, default=3333)
    parser.add_argument('--timeout', type=float, default=300)
    args = parser.parse_args()
    if not 0 <= args.port <= 65535 or not 0 < args.timeout <= 600:
        parser.error('Port ou durée invalide.')
    try:
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as server:
            server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
            server.bind((args.bind, args.port))
            server.listen(4)
            port = server.getsockname()[1]
            print(f'Test TCP en attente sur le port {port}. Laissez ce terminal ouvert.', flush=True)
            print('Arrêt automatique après un test réussi ou au bout de cinq minutes. Ctrl+C pour arrêter.', flush=True)
            deadline = time.monotonic() + args.timeout
            while time.monotonic() < deadline:
                server.settimeout(min(1, deadline - time.monotonic()))
                try:
                    client, _ = server.accept()
                except socket.timeout:
                    continue
                with client:
                    probe_deadline = min(deadline, time.monotonic() + 3)
                    request = b''
                    try:
                        while len(request) < 128 and b'\n' not in request and time.monotonic() < probe_deadline:
                            client.settimeout(max(.001, probe_deadline - time.monotonic()))
                            chunk = client.recv(128 - len(request))
                            if not chunk:
                                break
                            request += chunk
                        if request == b'BDL-PORT-TEST\n':
                            client.sendall(b'BDL-PORT-OK\n')
                            print('Connexion de test valide reçue. Test terminé, port de test fermé.', flush=True)
                            return 0
                    except OSError:
                        continue
            print('Délai écoulé. Port de test fermé, aucun test validé.', flush=True)
            return 1
    except KeyboardInterrupt:
        print('\nTest arrêté, port de test fermé.', flush=True)
        return 130
    except OSError as error:
        print(f'Impossible de lancer le test TCP : {error}', file=sys.stderr)
        return 1


if __name__ == '__main__':
    sys.exit(main())
