#!/usr/bin/env python3
"""Read local statistics, emitting only the public status schema on stdout."""
import argparse
from datetime import datetime, timezone
import json
import math
from pathlib import Path
import re
import socket
import subprocess
import sys
import time


def number(value, integer=False):
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        return None
    if not math.isfinite(value) or value < 0 or (integer and int(value) != value):
        return None
    return int(value) if integer else value


def hashrate(value):
    if isinstance(value, (int, float)):
        return number(value)
    if not isinstance(value, str):
        return None
    match = re.fullmatch(r'\s*(\d+(?:\.\d+)?)\s*([kKMGTPE]?)\s*(?:[hH](?:/s)?)?\s*', value)
    if not match:
        return None
    power = {'': 0, 'K': 1, 'M': 2, 'G': 3, 'T': 4, 'P': 5, 'E': 6}[match[2].upper()]
    return number(float(match[1]) * 1000 ** power)


def utc_timestamp(now=None):
    return datetime.fromtimestamp(time.time() if now is None else now, timezone.utc).isoformat(timespec='seconds').replace('+00:00', 'Z')


def filtered_node(data):
    """Allowlist public measurements, including records read from disk."""
    if not isinstance(data, dict):
        return None
    chain = data.get('chain')
    blocks = number(data.get('blocks'), integer=True)
    headers = number(data.get('headers'), integer=True)
    progress = number(data.get('verificationProgress'))
    syncing = data.get('initialBlockDownload')
    if (chain not in ('main', 'test', 'testnet4', 'signet', 'regtest')
            or blocks is None or headers is None or progress is None
            or progress > 1 or type(syncing) is not bool):
        return None
    return dict(chain=chain, blocks=blocks, headers=headers,
                verificationProgress=progress, initialBlockDownload=syncing)


def node_info(cli, cli_args=()):
    empty = dict(chain=None, blocks=None, headers=None,
                 verificationProgress=None, initialBlockDownload=None, updatedAt=None)
    for attempt in range(2):
        try:
            result = subprocess.run([cli, *cli_args, '-rpcclienttimeout=30', 'getblockchaininfo'],
                                    capture_output=True, text=True, timeout=35, check=True)
            data = json.loads(result.stdout)
            node = filtered_node(dict(chain=data.get('chain'), blocks=data.get('blocks'),
                                      headers=data.get('headers'),
                                      verificationProgress=data.get('verificationprogress'),
                                      initialBlockDownload=data.get('initialblockdownload')))
            if node is None:
                raise ValueError('Invalid node measurements')
            node['updatedAt'] = utc_timestamp()
            return node, 'ok'
        except FileNotFoundError:
            break
        except (OSError, subprocess.SubprocessError, ValueError, AttributeError):
            # Neither RPC error text nor credentials are copied to the public JSON.
            if attempt == 0:
                time.sleep(1)
    print('Bitcoin Core : lecture indisponible après nouvelle tentative.', file=sys.stderr)
    return empty, 'unavailable'


def node_from_snapshot(previous, now):
    """Extract an originally successful reading without refreshing its age."""
    try:
        if previous.get('source') not in ('live', 'snapshot'):
            return None
        check = previous.get('checks', {}).get('bitcoin')
        if check == 'unavailable':
            return None
        data = previous.get('node', {})
        node = filtered_node(data)
        if node is None:
            return None
        stamp = data.get('updatedAt')
        if not stamp and check != 'stale':
            # Older successful snapshots used the collection timestamp.
            stamp = previous.get('updatedAt')
        observed = datetime.fromisoformat(stamp.replace('Z', '+00:00'))
        if observed.tzinfo is None or observed.timestamp() > now + 60:
            return None
        node['updatedAt'] = observed.astimezone(timezone.utc).isoformat(timespec='seconds').replace('+00:00', 'Z')
        return node
    except (ValueError, TypeError, AttributeError):
        return None


def previous_node(path, now):
    if not path:
        return None
    path = Path(path)
    try:
        node = node_from_snapshot(json.loads(path.read_text()), now)
        if node is not None:
            return node
    except (OSError, ValueError):
        pass
    # Upgrade recovery: the older collector may already have overwritten its
    # latest reading with nulls. Read bounded local history without modifying it.
    if not (path.parent / '.git').is_dir():
        return None
    deadline = time.monotonic() + 6
    try:
        commits = subprocess.run(['git', '-C', str(path.parent), 'log', '-n', '32',
                                  '--format=%H', '--', path.name], capture_output=True,
                                 text=True, timeout=3, check=True).stdout.splitlines()
        for commit in commits:
            if time.monotonic() >= deadline:
                break
            if not re.fullmatch(r'[0-9a-f]{40,64}', commit):
                continue
            result = subprocess.run(['git', '-C', str(path.parent), 'show', f'{commit}:{path.name}'],
                                    capture_output=True, text=True,
                                    timeout=max(.01, deadline - time.monotonic()), check=True)
            try:
                node = node_from_snapshot(json.loads(result.stdout), now)
            except ValueError:
                continue
            if node is not None:
                print('Dernier relevé réussi du nœud récupéré depuis l’historique local.', file=sys.stderr)
                return node
    except (OSError, subprocess.SubprocessError):
        pass
    return None


def pool_status_path(config_path, explicit):
    if explicit:
        return Path(explicit)
    config_path = Path(config_path)
    try:
        # CKPool configurations may contain trailing content. Read only logdir,
        # never copying this configuration or any other fields into public output.
        content = config_path.read_text()
        paths = re.findall(r'"logdir"\s*:\s*("(?:[^"\\]|\\.)*")', content)
        logdir = Path(json.loads(paths[0])) if len(paths) == 1 else Path('logs')
    except (OSError, ValueError):
        logdir = Path('logs')
    if logdir.is_absolute():
        return logdir / 'pool/pool.status'
    roots = []
    try:
        unit = subprocess.run(['systemctl', 'show', 'ckpool.service', '--property=WorkingDirectory', '--value'],
                              capture_output=True, text=True, timeout=5, check=True).stdout.strip()
        if unit and Path(unit).is_absolute():
            roots.append(Path(unit))
    except (OSError, subprocess.SubprocessError):
        pass
    roots.append(config_path.parent)
    for root in roots:
        candidate = root / logdir / 'pool/pool.status'
        if candidate.is_file():
            return candidate
    return config_path.parent / logdir / 'pool/pool.status'


def pool_info(path, now):
    empty = dict(ready=False, hashrateHps=None, workers=None, bestShare=None, blocksFound=None)
    try:
        # CKPool writes several consecutive JSON objects, rather than one JSON document.
        text = path.read_text()
        if len(text) > 262144:
            return empty, 'invalid'
        decoder = json.JSONDecoder()
        data = {}
        while text.strip():
            value, end = decoder.raw_decode(text.lstrip())
            if not isinstance(value, dict):
                return empty, 'invalid'
            data.update(value)
            text = text.lstrip()[end:]
        updated = number(data.get('lastupdate'))
        if updated is None:
            updated = path.stat().st_mtime
        if not -60 <= now - updated <= 180:
            return empty, 'stale'
        empty.update(hashrateHps=hashrate(data.get('hashrate5m')),
                     workers=number(data.get('Workers', data.get('workers')), integer=True),
                     bestShare=number(data.get('bestshare')))
        # pool.status does not reliably expose accepted block counts; do not invent one.
        return empty, 'ok'
    except FileNotFoundError:
        return empty, 'missing'
    except (OSError, ValueError, TypeError):
        return empty, 'invalid'


def stratum_check(host, port, timeout=5):
    """Subscribe without authenticating a fake worker or sending shares."""
    subscribed = False
    job_received = False
    try:
        deadline = time.monotonic() + timeout
        with socket.create_connection((host, port), timeout=timeout) as sock:
            sock.sendall(b'{"id":1,"method":"mining.subscribe","params":["BDL-status/1"]}\n')
            buffer = b''
            while time.monotonic() < deadline:
                sock.settimeout(max(0.01, deadline - time.monotonic()))
                chunk = sock.recv(16384)
                if not chunk:
                    break
                buffer += chunk
                if len(buffer) > 131072:
                    break
                while b'\n' in buffer:
                    line, buffer = buffer.split(b'\n', 1)
                    if not line.strip():
                        continue
                    message = json.loads(line)
                    if not isinstance(message, dict):
                        continue
                    result = message.get('result')
                    if message.get('id') == 1 and not message.get('error'):
                        subscribed = isinstance(result, list) and len(result) == 3 and isinstance(result[0], list) and isinstance(result[1], str) and type(result[2]) is int and result[2] > 0
                    params = message.get('params')
                    if message.get('method') == 'mining.notify' and isinstance(params, list) and len(params) >= 9:
                        job_received = bool(params[0]) and isinstance(params[1], str) and len(params[1]) == 64
                if subscribed and job_received:
                    return 'job_received'
    except (OSError, ValueError):
        pass
    return 'subscribed_no_job' if subscribed else 'unavailable'


def history_from(path, now):
    try:
        data = json.loads(Path(path).read_text())
        history = data.get('history', [])
        clean = []
        for point in history[-288:]:
            stamp = datetime.fromisoformat(point['time'].replace('Z', '+00:00')).timestamp()
            rate = number(point['hashrateHps'])
            if now - 86400 <= stamp <= now and rate is not None:
                clean.append(dict(time=point['time'], hashrateHps=rate))
        return sorted(clean, key=lambda point: point['time'])
    except (OSError, ValueError, TypeError, KeyError, AttributeError):
        return []


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--bitcoin-cli', default='/usr/local/bin/bitcoin-cli')
    parser.add_argument('--config', default='/home/bdl/mining/ckpool/local.conf')
    parser.add_argument('--pool-status')
    parser.add_argument('--previous')
    parser.add_argument('--stratum-host', default='192.168.1.2')
    parser.add_argument('--stratum-port', type=int, default=3333)
    parser.add_argument('--profile', type=Path, help='Private local BCH monitoring profile')
    parser.add_argument('--coin', choices=('btc', 'bch'), default='btc')
    args = parser.parse_args()
    cli_args = []
    if args.profile:
        profile = json.loads(args.profile.read_text())
        if profile.get('coin') != 'bch':
            parser.error('The BCH profile must declare coin=bch')
        args.coin = 'bch'
        args.bitcoin_cli = profile['bitcoinCli']
        cli_args = profile.get('cliArgs', [])
        if not isinstance(cli_args, list) or any(not isinstance(value, str) for value in cli_args):
            parser.error('Invalid CLI arguments')
        args.config = profile['config']
        args.pool_status = profile['poolStatus']
        args.stratum_host = profile.get('stratumHost', '127.0.0.1')
        args.stratum_port = profile['stratumPort']
    if args.previous:
        try:
            if json.loads(Path(args.previous).read_text()).get('coin', 'btc') != args.coin:
                parser.error('Previous snapshot belongs to another network')
        except (OSError, ValueError):
            pass
    node, bitcoin = node_info(args.bitcoin_cli, cli_args) if cli_args else node_info(args.bitcoin_cli)
    now = time.time()
    if bitcoin != 'ok':
        retained = previous_node(args.previous, now)
        if retained is not None:
            node, bitcoin = retained, 'stale'
    pool, statistics = pool_info(pool_status_path(args.config, args.pool_status), now)
    stratum = stratum_check(args.stratum_host, args.stratum_port) if bitcoin == 'ok' and node['initialBlockDownload'] is False else 'not_checked'
    pool['ready'] = bitcoin == 'ok' and node['chain'] == 'main' and node['initialBlockDownload'] is False and stratum == 'job_received'
    now = time.time()
    timestamp = utc_timestamp(now)
    history = history_from(args.previous, now) if args.previous else []
    if pool['hashrateHps'] is not None:
        history.append(dict(time=timestamp, hashrateHps=pool['hashrateHps']))
    result = dict(schemaVersion=1, coin=args.coin, source='snapshot', updatedAt=timestamp,
                  node=node, pool=pool, history=history[-288:],
                  checks=dict(bitcoin=bitcoin, poolStats=statistics, stratum=stratum))
    print(json.dumps(result, indent=2, allow_nan=False))
    print(f'Bitcoin : {bitcoin} ; statistiques CKPool : {statistics} ; Stratum : {stratum}.', file=sys.stderr)


if __name__ == '__main__':
    main()
