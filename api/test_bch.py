"""Network isolation, CashAddr validation and actual HTTP BCH routing."""
import importlib.util
import json
from pathlib import Path
from urllib.parse import quote
import unittest

import test_server as fixtures

api = fixtures.api
ADDRESS = fixtures.ADDRESS

CASHADDR = 'bitcoincash:qpm2qsznhks23z7629mms6s4cwef74vcwvy22gdx6a'


class BCHTests(unittest.TestCase):
    def setUp(self):
        self.fixture = fixtures.APITests()
        self.fixture.setUp()
        self.addCleanup(self.fixture.doCleanups)
        self.root = self.fixture.root
        self.logs = self.root/'bch-logs/users'
        self.logs.mkdir(parents=True)
        self.snapshot = self.root/'status-bch.json'
        self.config = dict(snapshot=str(self.snapshot), logs=str(self.logs.parent),
                           sv2Public=str(self.fixture.public), onlineSnapshot=str(self.root/'bch-online.json'),
                           host='bch.example.com', v1Port=3334)
        self.service = api.PoolAPI(self.config, 'bch')
        self.fixture.service.coins['bch'] = self.service
        self.fixture.write(self.snapshot, dict(self.fixture.sample, coin='bch'))
        self.fixture.write(self.logs/CASHADDR, dict(self.fixture.user, shares=987, worker=[
            dict(workername=CASHADDR.split(':')[1].upper()+'.bitaxe', shares=12),
            dict(workername=ADDRESS+'.wrong-network', shares=555)]))

    def request(self, path):
        return self.fixture.request('/api/v1/coins/bch'+path)

    def test_bch_routing_never_reads_btc_data(self):
        self.assertEqual(self.fixture.request('/api/v1/miners/'+ADDRESS)[2]['shares'], 1234)
        self.assertEqual(self.request('/miners/'+quote(CASHADDR.upper(), safe=''))[2]['shares'], 987)
        workers = self.request('/miners/'+CASHADDR+'/workers?limit=1&offset=0')[2]
        self.assertEqual(workers['total'], 1)
        self.assertEqual(workers['address'], CASHADDR)
        self.assertEqual(workers['coin'], 'bch')
        self.assertEqual(workers['workers'][0]['name'], CASHADDR+'.bitaxe')
        self.assertEqual(self.request('/miners/'+CASHADDR+'/workers/bitaxe')[0], 200)
        self.assertEqual(self.request('/miners/'+ADDRESS)[0], 400)
        self.assertEqual(self.fixture.request('/api/v1/miners/'+CASHADDR)[0], 400)

    def test_checksum_network_case_and_safe_paths(self):
        for address in (CASHADDR, CASHADDR.upper(), CASHADDR.split(':')[1]):
            self.assertEqual(api.canonical_address(address, 'bch'), CASHADDR)
        for address in (CASHADDR[:-1]+'q', CASHADDR.replace('bitcoincash:', 'bchtest:'),
                        CASHADDR.replace('qpm', 'Qpm'), ADDRESS, 'bc1qqqqqqqqqqqqqq', '../config.json'):
            self.assertIsNone(api.canonical_address(address, 'bch'), address)
        self.assertIsNotNone(api.canonical_address('bitcoincash:ppm2qsznhks23z7629mms6s4cwef74vcwvn0h829pq', 'bch'))

    def test_bch_never_offers_btc_sv2_or_snapshot(self):
        body = self.request('/stratum')[2]
        self.assertEqual(body['v1']['url'], 'stratum+tcp://bch.example.com:3334')
        self.assertFalse(body['v2']['configured'])
        self.assertIsNone(body['v2']['authorityPublicKey'])
        self.assertIsNone(body['v2']['port'])
        self.fixture.write(self.snapshot, self.fixture.sample)
        self.assertEqual(self.request('/pool')[0], 503)
        self.assertEqual(self.request('/history')[0], 503)
        self.assertEqual(self.fixture.request('/api/v1/pool')[0], 200)

    def test_missing_bch_config_never_falls_back_to_btc(self):
        self.fixture.service.coins.clear()
        self.assertEqual(self.request('/pool')[0], 503)
        self.assertEqual(self.request('/stratum')[0], 503)
        self.assertEqual(self.fixture.request('/api/v1/coins/ltc/pool')[0], 404)
        self.assertEqual(self.fixture.request('/api/v1/health')[0], 200)

    def test_bch_presence_uses_its_own_snapshot_and_masks_cashaddr(self):
        client = dict(id='a'*24, address=CASHADDR, name=CASHADDR+'.bitaxe',
                      device={}, hashrateHps={'5m': 123}, bestShare=42)
        source = dict(schemaVersion=1, coin='bch', available=True, updatedAt=self.fixture.stamp, clients=[client])
        self.fixture.write(self.root/'bch-online.json', source)
        body = self.request('/online')[2]
        self.assertEqual(body['total'], 1)
        self.assertNotIn(CASHADDR, json.dumps(body))
        self.assertIsNone(self.fixture.request('/api/v1/online')[2]['total'])
        self.fixture.write(self.root/'bch-online.json', dict(source, coin='btc'))
        self.assertFalse(self.request('/online')[2]['available'])

    def test_installer_keeps_secrets_local_and_protects_existing_files(self):
        spec = importlib.util.spec_from_file_location('bch_setup', Path(__file__).parents[1]/'bch/configure.py')
        setup = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(setup)
        stage = self.root/'stage'
        setup.prepare(stage, CASHADDR, 'bch.example.com', 39992)
        node = stage/'home/bdl/mining/bch/node/bitcoin.conf'
        pool = stage/'home/bdl/mining/bch/cashstratum.conf'
        config = stage/'home/bdl/.config/bdl-pool-bch/api.json'
        original = pool.read_bytes()
        self.assertEqual(node.stat().st_mode & 0o777, 0o600)
        self.assertEqual(json.loads(original)['poolfee'], 0)
        password = json.loads(original)['btcd'][0]['pass']
        self.assertIn(password, node.read_text())
        self.assertNotIn(password, config.read_text())
        unit = (config.parent/'units/bdl-pool-bch.service').read_text()
        self.assertIn(' -B ', unit)
        self.assertIn('/run/bdl-pool-bch', unit)
        self.assertNotIn('ckpool.service', unit)
        with self.assertRaises(ValueError):
            setup.prepare(stage, CASHADDR, 'bch.example.com', 39992)
        self.assertEqual(pool.read_bytes(), original)


if __name__ == '__main__':
    unittest.main()
