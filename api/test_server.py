import hashlib
import http.client
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import threading
import time
import unittest

spec = importlib.util.spec_from_file_location('pool_api', Path(__file__).with_name('server.py'))
api = importlib.util.module_from_spec(spec)
spec.loader.exec_module(api)
ADDRESS = '1BoatSLRHtKNngkdXEeobR76b53LETtpyT'
OTHER = '1BitcoinEaterAddressDontSendf59kuE'
PUBLIC_KEY = '9c9DhUCzDsSZaCgKJitKKcngq24FLrqfJ9kUQzhxLiiSvAGuhna'


class APITests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix='bdl-api-test-')
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.users = self.root/'logs/users'
        self.users.mkdir(parents=True)
        self.snapshot = self.root/'status.json'
        self.public = self.root/'sv2-public.json'
        self.stamp = api.epoch(time.time())
        self.sample = dict(schemaVersion=1, source='snapshot', updatedAt=self.stamp,
                           node=dict(chain='main', blocks=975013, headers=975013, verificationProgress=1,
                                     initialBlockDownload=False, updatedAt=self.stamp, rpcpassword='PRIVATE_TEST'),
                           pool=dict(ready=True, hashrateHps=1e12, workers=2, bestShare=1000, blocksFound=None, private='PRIVATE_TEST'),
                           checks=dict(bitcoin='ok', poolStats='ok', stratum='job_received'),
                           history=[dict(time=self.stamp, hashrateHps=1e12, ip='PRIVATE_TEST')], secret='PRIVATE_TEST')
        self.write(self.snapshot, self.sample)
        self.write(self.public, dict(authorityPublicKey=PUBLIC_KEY, privateKey='PRIVATE_TEST'))
        self.user = dict(hashrate1m='1.5T', hashrate5m='1.25T', hashrate1hr='950G', hashrate1d='12M', hashrate7d='7K',
                         lastshare=int(time.time())-20, shares=1234, bestshare=123.5, bestever=1000, workers=2,
                         ip='PRIVATE_TEST', worker=[
                             dict(workername=ADDRESS+'.bitaxe-01', hashrate5m='750G', shares=100, lastshare=int(time.time())-10, password='PRIVATE_TEST'),
                             dict(workername=ADDRESS+'.bitaxe-02', hashrate5m='500G', shares=200),
                             dict(workername=OTHER+'.foreign', hashrate5m='9T', private='PRIVATE_TEST')])
        self.write(self.users/ADDRESS, self.user)
        self.service = api.PoolAPI(dict(snapshot=str(self.snapshot), logs=str(self.root/'logs'),
                                       sv2Public=str(self.public), host='bdl-pool.duckdns.org'))
        self.server = api.Server(('127.0.0.1', 0), self.service)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()
        self.addCleanup(self.stop)

    def stop(self):
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(timeout=2)
        api.read_cached.cache_clear()

    def write(self, path, data):
        path.write_text(json.dumps(data))
        api.read_cached.cache_clear()

    def request(self, path, method='GET', headers=None):
        connection = http.client.HTTPConnection('127.0.0.1', self.server.server_port, timeout=3)
        connection.request(method, path, headers=headers or {})
        response = connection.getresponse()
        status, headers, body = response.status, dict(response.getheaders()), response.read()
        connection.close()
        self.assertNotIn(b'PRIVATE_TEST', body)
        return status, headers, json.loads(body) if body else None

    def test_global_allowlist_freshness_and_units(self):
        status, headers, body = self.request('/api/v1/pool')
        self.assertEqual(status, 200)
        self.assertEqual(headers['Access-Control-Allow-Origin'], '*')
        self.assertTrue(body['pool']['readyAtLastCheck'])
        self.assertEqual(body['pool']['hashrateHps'], 1e12)
        self.assertIsNone(body['pool']['blocksFound'])
        self.assertEqual(body['node']['updatedAt'], self.stamp)
        self.assertFalse(body['node']['stale'])
        for location in (body, body['pool'], body['node']):
            self.assertNotIn('secret', location)
            self.assertNotIn('rpcpassword', location)

    def test_stale_or_failed_node_never_confirms_availability(self):
        for scenario in ('old_snapshot', 'old_node', 'retained', 'no_job'):
            data = json.loads(json.dumps(self.sample))
            if scenario == 'old_snapshot': data['updatedAt'] = api.epoch(time.time()-1000)
            if scenario == 'old_node': data['node']['updatedAt'] = api.epoch(time.time()-1000)
            if scenario == 'retained': data['checks']['bitcoin'] = 'stale'
            if scenario == 'no_job': data['checks']['stratum'] = 'subscribed_no_job'
            self.write(self.snapshot, data)
            self.assertFalse(self.request('/api/v1/pool')[2]['pool']['readyAtLastCheck'], scenario)

    def test_unavailable_snapshot_does_not_break_health(self):
        self.snapshot.unlink();api.read_cached.cache_clear()
        self.assertEqual(self.request('/api/v1/pool')[0], 503)
        self.assertEqual(self.request('/api/v1/health')[2], dict(apiVersion='1', status='ok'))

    def test_miner_rates_workers_and_ownership(self):
        status, _, body = self.request('/api/v1/miners/'+ADDRESS)
        self.assertEqual(status, 200)
        self.assertEqual(body['hashrateHps']['5m'], 1.25e12)
        self.assertEqual(body['hashrateHps']['1m'], 1.5e12)
        self.assertEqual(body['hashrateHps']['1h'], 950e9)
        self.assertEqual(body['workersRecorded'], 2)
        self.assertEqual(body['shares'], 1234)
        self.assertNotIn('worker', body)
        rows = self.request('/api/v1/miners/'+ADDRESS+'/workers?limit=1&offset=1')[2]
        self.assertEqual(rows['total'], 2)
        self.assertEqual([row['name'] for row in rows['workers']], [ADDRESS+'.bitaxe-02'])
        self.assertEqual(self.request('/api/v1/miners/'+ADDRESS+'/workers/bitaxe-01')[2]['worker']['shares'], 100)
        self.assertEqual(self.request('/api/v1/miners/'+ADDRESS+'/workers/'+ADDRESS+'.bitaxe-01')[0], 200)
        self.assertEqual(self.request('/api/v1/miners/'+ADDRESS+'/workers/'+OTHER+'.foreign')[0], 404)

    def test_stale_miner_is_explicit_and_dates_do_not_refresh(self):
        old = time.time()-1000
        os.utime(self.users/ADDRESS, (old, old));api.read_cached.cache_clear()
        body = self.request('/api/v1/miners/'+ADDRESS)[2]
        self.assertTrue(body['stale'])
        self.assertEqual(body['updatedAt'], api.epoch(old))
        self.assertGreaterEqual(body['ageSeconds'], 999)

    def test_no_address_directory_and_unknown_miner(self):
        self.assertEqual(self.request('/api/v1/miners')[0], 404)
        self.assertEqual(self.request('/api/v1/miners/'+OTHER)[0], 404)
        self.assertEqual(self.request('/api/v1/miners/not-a-bitcoin-address')[0], 400)

    def test_traversal_symlinks_special_and_large_files(self):
        for route in ('/api/v1/miners/%2e%2e', '/api/v1/miners/%2e%2e%2fprivate', '/api/v1/miners/'+ADDRESS+'%2f..'):
            self.assertIn(self.request(route)[0], (400, 404))
        path = self.users/ADDRESS;path.unlink()
        private = self.root/'private.json';private.write_text('{"secret":"PRIVATE_TEST"}')
        path.symlink_to(private);api.read_cached.cache_clear()
        self.assertEqual(self.request('/api/v1/miners/'+ADDRESS)[0], 503)
        path.unlink();os.mkfifo(path);api.read_cached.cache_clear()
        self.assertEqual(self.request('/api/v1/miners/'+ADDRESS)[0], 503)
        path.unlink();path.write_bytes(b' '* (api.MAX_FILE+1));api.read_cached.cache_clear()
        self.assertEqual(self.request('/api/v1/miners/'+ADDRESS)[0], 503)

    def test_query_validation_and_ambiguous_workers(self):
        for query in ('limit=0', 'limit=201', 'offset=-1', 'limit=1&limit=2', 'unknown=1', 'limit='):
            self.assertEqual(self.request('/api/v1/miners/'+ADDRESS+'/workers?'+query)[0], 400, query)
        self.assertEqual(self.request('/api/v1/health?callback=alert')[0], 400)
        self.user['worker'].append(dict(workername=ADDRESS+'_bitaxe-01'))
        self.write(self.users/ADDRESS, self.user)
        self.assertEqual(self.request('/api/v1/miners/'+ADDRESS+'/workers/bitaxe-01')[0], 400)
        self.assertEqual(self.request('/api/v1/miners/'+ADDRESS+'/workers/'+ADDRESS+'_bitaxe-01')[0], 200)

    def test_public_stratum_key_and_missing_key(self):
        body = self.request('/api/v1/stratum')[2]
        self.assertEqual(body['v2']['authorityPublicKey'], PUBLIC_KEY)
        self.assertEqual(body['v2']['url'], 'stratum2+tcp://bdl-pool.duckdns.org:3336/'+PUBLIC_KEY)
        self.assertEqual(body['v2']['availability'], 'not_verified')
        for key in ('1'*50, PUBLIC_KEY[:-1]+'1', None):
            self.write(self.public, dict(authorityPublicKey=key))
            self.assertFalse(self.request('/api/v1/stratum')[2]['v2']['configured'])

    def test_head_options_etag_and_read_only_methods(self):
        original = hashlib.sha256(self.snapshot.read_bytes()).hexdigest()
        status, headers, body = self.request('/api/v1/health')
        self.assertEqual(self.request('/api/v1/health', headers={'If-None-Match': headers['ETag']})[0], 304)
        status, headers, body = self.request('/api/v1/pool', 'HEAD')
        self.assertEqual(status, 200);self.assertIsNone(body)
        status, headers, body = self.request('/api/v1/pool', 'OPTIONS')
        self.assertEqual(status, 204);self.assertIsNone(body)
        for method in ('POST', 'PUT', 'PATCH', 'DELETE'):
            self.assertEqual(self.request('/api/v1/pool', method)[0], 405)
        self.assertEqual(hashlib.sha256(self.snapshot.read_bytes()).hexdigest(), original)

    def test_corrupt_or_nonfinite_statistics(self):
        for text in ('{"shares":', '{"shares":NaN}', '[]'):
            (self.users/ADDRESS).write_text(text);api.read_cached.cache_clear()
            self.assertEqual(self.request('/api/v1/miners/'+ADDRESS)[0], 503)
        self.user.update(shares=-1, bestshare=float('inf'), hashrate5m='invalid')
        self.user['bestshare'] = None
        self.write(self.users/ADDRESS, self.user)
        result = self.request('/api/v1/miners/'+ADDRESS)[2]
        self.assertIsNone(result['shares']);self.assertIsNone(result['hashrateHps']['5m'])
        self.assertIsNone(api.number(10**1000));self.assertIsNone(api.number(True))

    def test_history_and_openapi(self):
        result = self.request('/api/v1/history')[2]
        self.assertEqual(result['points'], [dict(time=self.stamp, hashrateHps=1e12)])
        status, _, document = self.request('/api/v1/openapi.json')
        self.assertEqual(status, 200)
        self.assertEqual(document['openapi'], '3.1.0')
        self.assertIn('/api/v1/miners/{address}/workers', document['paths'])


if __name__ == '__main__':
    unittest.main()
