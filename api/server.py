#!/usr/bin/env python3
"""Read-only BDL Pool API. Run on loopback behind the supplied HTTPS proxy."""
import argparse
from datetime import datetime, timezone
from functools import lru_cache
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import math
import os
from pathlib import Path
import re
import socket
import stat
import threading
import time
from urllib.parse import parse_qs, unquote, urlsplit

MAX_FILE = 1048576
ADDRESS = re.compile(r'(?:[13][1-9A-HJ-NP-Za-km-z]{25,34}|bc1[ac-hj-np-z02-9]{11,87})')
WORKER = re.compile(r'[A-Za-z0-9._-]{1,128}')
RATES = {'1m': 'hashrate1m', '5m': 'hashrate5m', '1h': 'hashrate1hr',
         '1d': 'hashrate1d', '7d': 'hashrate7d'}
SOFTWARE = ('BDLMiner V2', 'NerdMiner V2', 'ESP-Miner', 'AxeOS', 'NerdQAxe',
            'NerdAxe', 'cgminer', 'BFGMiner', 'Braiins OS', 'cpuminer', 'Antminer', 'Whatsminer')
MODELS = ('M5StickC Plus2', 'M5StickC Plus', 'M5Stack CoreS3', 'M5Stack Core2',
          'Bitaxe Gamma', 'Bitaxe Supra', 'Bitaxe Ultra', 'Bitaxe Hex', 'Bitaxe')


def worker_identity(name):
    if not isinstance(name, str) or not WORKER.fullmatch(name):
        return None
    match = re.fullmatch('('+ADDRESS.pattern+r')(?:[._]([A-Za-z0-9._-]{1,64}))?', name)
    return (match[1], match[2]) if match else None


def device_from_agent(agent):
    """Only report a hardware model when it is explicitly advertised in Stratum."""
    result = dict(model=None, software=None, version=None)
    if not isinstance(agent, str) or len(agent) > 256:
        return result
    patterns = ((r'bdlminer\s*v?2', 'BDLMiner V2'), (r'nerdminer\s*v?2?', 'NerdMiner V2'),
                (r'esp[- ]?miner', 'ESP-Miner'), (r'axeos', 'AxeOS'),
                (r'nerdqaxe', 'NerdQAxe'), (r'nerdaxe', 'NerdAxe'),
                (r'cgminer', 'cgminer'), (r'bfgminer', 'BFGMiner'),
                (r'braiins(?:\s*os)?', 'Braiins OS'), (r'cpuminer', 'cpuminer'),
                (r'antminer', 'Antminer'), (r'whatsminer', 'Whatsminer'))
    for pattern, label in patterns:
        match = re.search(pattern, agent, re.I)
        if match:
            result['software'] = label
            version = re.search(r'^[ /v:_-]*(\d{1,3}(?:\.\d{1,3}){1,3}(?:[-+][A-Za-z0-9._-]{1,16})?)', agent[match.end():], re.I)
            result['version'] = version[1] if version else None
            break
    for pattern, label in ((r'm5stickc\s*(?:plus\s*2|\+2)', 'M5StickC Plus2'),
                           (r'm5stickc\s*(?:plus|\+)', 'M5StickC Plus'),
                           (r'm5stack\s*core\s*s3', 'M5Stack CoreS3'),
                           (r'm5stack\s*core\s*2', 'M5Stack Core2')):
        if re.search(pattern, agent, re.I):
            result['model'] = label
            break
    match = re.search(r'\bbitaxe(?:[ /_-]+(gamma|supra|ultra|hex))?\b', agent, re.I)
    if match:
        result['model'] = 'Bitaxe'+(' '+match[1].title() if match[1] else '')
    for pattern, prefix in ((r'\bantminer[ /_-]+(s\d{1,2}[a-z]?(?:\s+(?:pro|xp|hydro|hyd))?)\b', 'Antminer '),
                            (r'\bwhatsminer[ /_-]+(m\d{1,3}[a-z]?(?:\+{1,2})?)\b', 'Whatsminer ')):
        match = re.search(pattern, agent, re.I)
        if match:
            result['model'] = prefix+match[1].upper()
    return result


def clean_device(value):
    value = value if isinstance(value, dict) else {}
    model, software, version = (value.get(key) for key in ('model', 'software', 'version'))
    model = model if isinstance(model, str) and (model in MODELS or re.fullmatch(r'(?:Antminer S\d{1,2}[A-Z]?(?: (?:PRO|XP|HYDRO|HYD))?|Whatsminer M\d{1,3}[A-Z]?\+{0,2})', model)) else None
    software = software if software in SOFTWARE else None
    version = version if isinstance(version, str) and re.fullmatch(r'\d{1,3}(?:\.\d{1,3}){1,3}(?:[-+][A-Za-z0-9._-]{1,16})?', version) else None
    return dict(model=model, software=software, version=version)


class APIError(Exception):
    def __init__(self, status, code, message):
        self.status, self.code, self.message = status, code, message


def number(value, integer=False):
    if type(value) not in (int, float):
        return None
    try:
        if not math.isfinite(value) or value < 0:
            return None
    except OverflowError:
        return None
    if integer and int(value) != value:
        return None
    return int(value) if integer else value


def hashrate(value):
    if type(value) in (int, float):
        return number(value)
    if not isinstance(value, str) or len(value) > 40:
        return None
    match = re.fullmatch(r'\s*(\d+(?:\.\d+)?)\s*([kKMGTPE]?)\s*(?:[hH](?:/s)?)?\s*', value)
    return number(float(match[1]) * 1000 ** ' KMGTPE'.index(match[2].upper())) if match else None


def timestamp(value):
    try:
        if not isinstance(value, str) or len(value) > 40:
            return None
        date = datetime.fromisoformat(value.replace('Z', '+00:00'))
        if date.tzinfo is None or date.timestamp() > time.time() + 60:
            return None
        return date.astimezone(timezone.utc).isoformat(timespec='seconds').replace('+00:00', 'Z')
    except (ValueError, OverflowError):
        return None


def epoch(value):
    value = number(value)
    try:
        return timestamp(datetime.fromtimestamp(value, timezone.utc).isoformat()) if value else None
    except (ValueError, OverflowError, OSError):
        return None


def freshness(stamp, maximum_age):
    age = None if stamp is None else max(0, int(time.time() - datetime.fromisoformat(stamp.replace('Z', '+00:00')).timestamp()))
    return dict(updatedAt=stamp, ageSeconds=age, stale=age is None or age > maximum_age)


def reject_constant(_):
    raise ValueError('Invalid JSON number')


@lru_cache(maxsize=32)
def read_cached(filename, bucket):
    """Bound file size/cache size; never follow a final symlink or read a device."""
    for attempt in range(2):
        try:
            descriptor = os.open(filename, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
            with os.fdopen(descriptor, 'rb') as stream:
                before = os.fstat(stream.fileno())
                if not stat.S_ISREG(before.st_mode) or before.st_size > MAX_FILE:
                    raise ValueError('Invalid statistics file')
                raw = stream.read(MAX_FILE + 1)
                after = os.fstat(stream.fileno())
            if len(raw) > MAX_FILE or (before.st_size, before.st_mtime_ns) != (after.st_size, after.st_mtime_ns):
                raise ValueError('Statistics changed while reading')
            # pool.status consists of consecutive JSON objects; user files are one object.
            decoder = json.JSONDecoder(parse_constant=reject_constant)
            text, merged = raw.decode('utf-8'), {}
            while text.strip():
                value, end = decoder.raw_decode(text.lstrip())
                if not isinstance(value, dict):
                    raise ValueError('Object expected')
                merged.update(value)
                text = text.lstrip()[end:]
            if not merged:
                raise ValueError('Empty statistics')
            return merged, before.st_mtime
        except (ValueError, UnicodeError):
            if attempt:
                raise APIError(503, 'statistics_unavailable', 'Statistics are temporarily unavailable.')
            time.sleep(.02)
        except FileNotFoundError:
            raise
        except OSError:
            raise APIError(503, 'statistics_unavailable', 'Statistics are temporarily unavailable.')


def read_json(path):
    return read_cached(str(path), int(time.monotonic() / 5))


def measurements(data):
    return dict(hashrateHps={period: hashrate(data.get(key)) for period, key in RATES.items()},
                shares=number(data.get('shares'), True), bestShare=number(data.get('bestshare')),
                bestEver=number(data.get('bestever')), lastShareAt=epoch(data.get('lastshare')))


def valid_public_key(key):
    if not isinstance(key, str) or not re.fullmatch(r'[1-9A-HJ-NP-Za-km-z]{40,70}', key):
        return False
    value = 0
    for character in key:
        value = value * 58 + '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz'.index(character)
    raw = b'\x00' * (len(key) - len(key.lstrip('1'))) + value.to_bytes((value.bit_length()+7)//8, 'big')
    return (len(raw) == 38 and raw[:2] == b'\x01\x00'
            and hashlib.sha256(hashlib.sha256(raw[:-4]).digest()).digest()[:4] == raw[-4:])


class PoolAPI:
    def __init__(self, config):
        self.snapshot = Path(config['snapshot'])
        self.logs = Path(config['logs'])
        self.public_key_file = Path(config['sv2Public'])
        self.online_file = Path(config.get('onlineSnapshot', '/var/lib/bdl-pool-presence/online.json'))
        self.host = config.get('host', 'bdl-pool.duckdns.org')
        self.api_host = config.get('apiHost', self.host)
        if any(not isinstance(host, str) or not re.fullmatch(r'[a-z0-9](?:[a-z0-9.-]{0,251}[a-z0-9])?', host)
               for host in (self.host, self.api_host)):
            raise ValueError('Invalid public hostname')
        self.v1_port, self.v2_port = config.get('v1Port', 3333), config.get('v2Port', 3336)
        self.api_port = config.get('apiPublicPort', 443)
        if any(type(p) is not int or not 1 <= p <= 65535 for p in (self.v1_port, self.v2_port, self.api_port)):
            raise ValueError('Invalid public port')

    def pool(self):
        try:
            data, _ = read_json(self.snapshot)
        except FileNotFoundError:
            raise APIError(503, 'snapshot_unavailable', 'No monitoring snapshot is available.')
        stamp = timestamp(data.get('updatedAt'))
        if stamp is None or data.get('source') not in ('snapshot', 'live'):
            raise APIError(503, 'snapshot_unavailable', 'No valid monitoring snapshot is available.')
        metadata = freshness(stamp, 900)
        raw_node = data.get('node') if isinstance(data.get('node'), dict) else {}
        node = {key: number(raw_node.get(key), True) for key in ('blocks', 'headers')}
        progress = number(raw_node.get('verificationProgress'))
        node.update(chain=raw_node.get('chain') if raw_node.get('chain') in ('main', 'test', 'testnet4', 'signet', 'regtest') else None,
                    verificationProgress=progress if progress is not None and progress <= 1 else None,
                    initialBlockDownload=raw_node.get('initialBlockDownload') if type(raw_node.get('initialBlockDownload')) is bool else None)
        checks = data.get('checks') if isinstance(data.get('checks'), dict) else {}
        node.update(freshness(timestamp(raw_node.get('updatedAt')) or (stamp if checks.get('bitcoin') == 'ok' else None), 900))
        allowed = {'bitcoin': ('ok', 'stale', 'unavailable'), 'poolStats': ('ok', 'missing', 'invalid', 'stale'),
                   'stratum': ('job_received', 'subscribed_no_job', 'unavailable', 'not_checked', 'invalid')}
        public_checks = {key: checks.get(key) if checks.get(key) in values else 'unknown' for key, values in allowed.items()}
        raw_pool = data.get('pool') if isinstance(data.get('pool'), dict) else {}
        pool = {key: number(raw_pool.get(key), key in ('workers', 'blocksFound'))
                for key in ('hashrateHps', 'workers', 'bestShare', 'blocksFound')}
        pool['readyAtLastCheck'] = (raw_pool.get('ready') is True and not metadata['stale'] and not node['stale']
                                   and all(node.get(key) is not None for key in ('chain', 'blocks', 'headers', 'verificationProgress'))
                                   and node['initialBlockDownload'] is False and public_checks['bitcoin'] == 'ok'
                                   and public_checks['stratum'] == 'job_received')
        return dict(apiVersion='1', source=data['source'], **metadata, pool=pool, node=node, checks=public_checks)

    def history(self):
        self.pool()  # Same source validity checks as the global endpoint.
        data, _ = read_json(self.snapshot)
        history = data.get('history')
        points = []
        if isinstance(history, list):
            for item in history[-288:]:
                if not isinstance(item, dict):
                    continue
                stamp, rate = timestamp(item.get('time')), number(item.get('hashrateHps'))
                if stamp and rate is not None:
                    points.append(dict(time=stamp, hashrateHps=rate))
        return dict(apiVersion='1', updatedAt=timestamp(data.get('updatedAt')), points=points)

    def stratum(self):
        key = None
        try:
            public, _ = read_json(self.public_key_file)
            candidate = public.get('authorityPublicKey')
            if valid_public_key(candidate):
                key = candidate
        except (FileNotFoundError, APIError):
            pass
        return dict(apiVersion='1', v1=dict(host=self.host, port=self.v1_port,
                    url=f'stratum+tcp://{self.host}:{self.v1_port}'),
                    v2=dict(configured=key is not None, host=self.host, port=self.v2_port,
                    authorityPublicKey=key, url=f'stratum2+tcp://{self.host}:{self.v2_port}/{key}' if key else None,
                    availability='not_verified'))

    def miner(self, address):
        if not ADDRESS.fullmatch(address):
            raise APIError(400, 'invalid_address', 'Use a Bitcoin mainnet address without a worker suffix.')
        directory = self.logs / 'users'
        if directory.is_symlink():
            raise APIError(503, 'statistics_unavailable', 'Statistics are temporarily unavailable.')
        try:
            data, modified = read_json(directory / address)
        except FileNotFoundError:
            raise APIError(404, 'miner_not_found', 'No CKPool statistics are recorded for this address.')
        rows = data.get('worker', [])
        if not isinstance(rows, list) or len(rows) > 5000:
            raise APIError(503, 'statistics_unavailable', 'Statistics are temporarily unavailable.')
        workers = []
        for row in rows:
            if not isinstance(row, dict):
                continue
            name = row.get('workername')
            if (isinstance(name, str) and WORKER.fullmatch(name)
                    and (name == address or name.startswith((address + '.', address + '_')))):
                workers.append(dict(name=name, **measurements(row)))
        workers.sort(key=lambda worker: worker['name'])
        presence, clients = self.presence()
        address_clients = [client for client in clients if client['address'] == address]
        for worker in workers:
            connected = [client for client in address_clients if client['name'] == worker['name']]
            worker.update(online=bool(connected) if presence['available'] else None,
                          connections=len(connected) if presence['available'] else None,
                          devices=[client['device'] for client in connected],
                          presenceUpdatedAt=presence['updatedAt'])
        result = dict(apiVersion='1', address=address, **freshness(epoch(modified), 180),
                      workersActive=number(data.get('workers'), True), workersRecorded=len(workers), **measurements(data),
                      connections=len(address_clients) if presence['available'] else None,
                      presenceAvailable=presence['available'], presenceUpdatedAt=presence['updatedAt'])
        return result, workers

    def presence(self):
        data = {}
        try:
            data, _ = read_json(self.online_file)
        except (FileNotFoundError, APIError):
            pass
        metadata = freshness(timestamp(data.get('updatedAt')), 45)
        available = data.get('schemaVersion') == 1 and data.get('available') is True and not metadata['stale']
        status = 'ok' if available else 'stale' if data.get('available') is True else 'unavailable'
        if not available:
            try:
                node = self.pool()['node']
                if node['initialBlockDownload'] is True and not node['stale']:
                    status = 'syncing'
            except APIError:
                pass
        rows = data.get('clients')
        clients = []
        if available and isinstance(rows, list) and len(rows) <= 2000:
            for row in rows:
                if not isinstance(row, dict):
                    continue
                identity = worker_identity(row.get('name'))
                identifier = row.get('id')
                if not identity or identity[0] != row.get('address') or not isinstance(identifier, str) or not re.fullmatch(r'[a-f0-9]{24}', identifier):
                    continue
                rates = row.get('hashrateHps') if isinstance(row.get('hashrateHps'), dict) else {}
                clients.append(dict(id=identifier, address=identity[0], name=row['name'], worker=identity[1],
                                    device=clean_device(row.get('device')), hashrateHps={period: number(rates.get(period)) for period in RATES},
                                    bestShare=number(row.get('bestShare')), connectedAt=timestamp(row.get('connectedAt')),
                                    lastShareAt=timestamp(row.get('lastShareAt'))))
        elif available:
            available, status = False, 'unavailable'
        return dict(available=available, status=status, updatedAt=metadata['updatedAt'],
                    checkedAt=timestamp(data.get('checkedAt')), stale=not available), clients

    def online(self, limit, offset):
        metadata, clients = self.presence()
        public = []
        for client in clients:
            item = {key: value for key, value in client.items() if key not in ('address', 'name')}
            item['addressMasked'] = client['address'][:6]+'…'+client['address'][-4:]
            if item['worker']:
                item['worker'] = ADDRESS.sub(lambda match: match[0][:6]+'…'+match[0][-4:], item['worker'])
            public.append(item)
        public.sort(key=lambda client: client['id'])
        return dict(apiVersion='1', **metadata, total=len(public) if metadata['available'] else None,
                    limit=limit, offset=offset, miners=public[offset:offset+limit])

    def dispatch(self, target):
        if len(target) > 2048:
            raise APIError(414, 'uri_too_long', 'Request URL is too long.')
        try:
            url = urlsplit(target)
            path = unquote(url.path, errors='strict').rstrip('/')
            query = parse_qs(url.query, keep_blank_values=True, strict_parsing=True, max_num_fields=4)
        except (ValueError, UnicodeError):
            raise APIError(400, 'invalid_request', 'Invalid request URL.')
        prefix = '/api/v1/'
        if not path.startswith(prefix):
            raise APIError(404, 'not_found', 'Endpoint not found.')
        route = path[len(prefix):]
        paginated = route == 'online' or re.fullmatch(r'miners/[^/]+/workers', route)
        if query and not paginated:
            raise APIError(400, 'invalid_query', 'This endpoint does not accept query parameters.')
        if paginated:
            if any(key not in ('limit', 'offset') or len(value) != 1 or not re.fullmatch(r'\d{1,6}', value[0]) for key, value in query.items()):
                raise APIError(400, 'invalid_query', 'Use one integer limit and offset.')
            limit, offset = int(query.get('limit', ['50'])[0]), int(query.get('offset', ['0'])[0])
            if not 1 <= limit <= 200:
                raise APIError(400, 'invalid_query', 'limit must be between 1 and 200.')
        if route == 'online':
            return self.online(limit, offset)
        if route == 'health':
            return dict(apiVersion='1', status='ok')
        if route == 'pool':
            return self.pool()
        if route == 'history':
            return self.history()
        if route == 'stratum':
            return self.stratum()
        if route == 'openapi.json':
            specification = json.loads(Path(__file__).with_name('openapi.json').read_text())
            origin = 'https://'+self.api_host+((':'+str(self.api_port)) if self.api_port != 443 else '')
            specification['servers'] = [dict(url=origin)]
            return specification
        parts = route.split('/')
        if len(parts) in (2, 3, 4) and parts[0] == 'miners' and (len(parts) == 2 or parts[2] == 'workers'):
            miner, workers = self.miner(parts[1])
            if len(parts) == 2:
                return miner
            if len(parts) == 3:
                return dict(apiVersion='1', address=parts[1], updatedAt=miner['updatedAt'], stale=miner['stale'],
                            total=len(workers), limit=limit, offset=offset, workers=workers[offset:offset+limit])
            name = parts[3]
            if not WORKER.fullmatch(name):
                raise APIError(400, 'invalid_worker', 'Invalid worker name.')
            matches = [w for w in workers if w['name'] == name or w['name'] in (parts[1]+'.'+name, parts[1]+'_'+name)]
            if len(matches) != 1:
                raise APIError(404 if not matches else 400, 'worker_not_found' if not matches else 'ambiguous_worker',
                               'Use the full worker name returned by the workers endpoint.')
            return dict(apiVersion='1', address=parts[1], updatedAt=miner['updatedAt'], stale=miner['stale'], worker=matches[0])
        raise APIError(404, 'not_found', 'Endpoint not found.')


class Handler(BaseHTTPRequestHandler):
    server_version = 'BDLPoolAPI/1'
    sys_version = ''

    def setup(self):
        super().setup()
        self.connection.settimeout(5)

    def log_message(self, *_):
        pass  # Do not write address/worker URLs to the journal.

    def respond(self, status, data, head=False):
        body = json.dumps(data, ensure_ascii=False, allow_nan=False, separators=(',', ':')).encode()
        etag = '"'+hashlib.sha256(body).hexdigest()+'"'
        cached = status == 200 and self.headers.get('If-None-Match') == etag
        self.send_response(304 if cached else status)
        self.send_header('Content-Type', 'application/json; charset=utf-8')
        self.send_header('Access-Control-Allow-Origin', '*')
        self.send_header('Access-Control-Allow-Methods', 'GET, HEAD, OPTIONS')
        self.send_header('Access-Control-Allow-Headers', 'If-None-Match')
        self.send_header('Access-Control-Expose-Headers', 'ETag')
        self.send_header('Cache-Control', 'public, max-age=5, must-revalidate' if status == 200 else 'no-store')
        self.send_header('X-Content-Type-Options', 'nosniff')
        self.send_header('Connection', 'close')
        if status == 405:
            self.send_header('Allow', 'GET, HEAD, OPTIONS')
        if status == 503:
            self.send_header('Retry-After', '5')
        if status != 204 and not cached:
            self.send_header('Content-Length', str(len(body)))
            self.send_header('ETag', etag)
        self.end_headers()
        if not head and not cached and status != 204:
            self.wfile.write(body)

    def read(self, head=False):
        try:
            self.respond(200, self.server.api.dispatch(self.path), head)
        except APIError as error:
            self.respond(error.status, dict(error=dict(code=error.code, message=error.message)), head)
        except (BrokenPipeError, ConnectionResetError, socket.timeout):
            pass
        except Exception:
            self.respond(503, dict(error=dict(code='service_unavailable', message='Service temporarily unavailable.')), head)

    def do_GET(self):
        self.read()

    def do_HEAD(self):
        self.read(True)

    def do_OPTIONS(self):
        self.respond(204, {})

    def do_POST(self):
        self.respond(405, dict(error=dict(code='method_not_allowed', message='This API is read-only.')))

    do_PUT = do_PATCH = do_DELETE = do_POST


class Server(ThreadingHTTPServer):
    daemon_threads = True
    request_queue_size = 32

    def __init__(self, address, api):
        self.api = api
        self.slots = threading.BoundedSemaphore(16)
        super().__init__(address, Handler)

    def process_request(self, request, client_address):
        if not self.slots.acquire(blocking=False):
            request.close()
            return
        try:
            super().process_request(request, client_address)
        except Exception:
            self.slots.release()
            raise

    def process_request_thread(self, request, client_address):
        try:
            super().process_request_thread(request, client_address)
        finally:
            self.slots.release()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--config', type=Path, default=Path.home()/'.config/bdl-pool-api/config.json')
    parser.add_argument('--port', type=int, default=8081)
    args = parser.parse_args()
    if not 1 <= args.port <= 65535:
        parser.error('Invalid port')
    api = PoolAPI(json.loads(args.config.read_text()))
    with Server(('127.0.0.1', args.port), api) as server:
        print(f'BDL Pool API listening on 127.0.0.1:{args.port}', flush=True)
        server.serve_forever()


if __name__ == '__main__':
    main()
