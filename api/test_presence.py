import importlib.util
import json
import os
from pathlib import Path
import socket
import struct
import sys
import tempfile
import threading
import time
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).parent))
import presence
import test_server as fixtures
ADDRESS, OTHER, api = fixtures.ADDRESS, fixtures.OTHER, fixtures.api


class PresenceTests(unittest.TestCase):
    def setUp(self):
        self.fixture = fixtures.APITests()
        self.fixture.setUp()
        self.addCleanup(self.fixture.doCleanups)
        self.path = self.fixture.root/'online.json'
        self.request = self.fixture.request
        self.client = dict(id=123, workername=ADDRESS+'.bitaxe-01', useragent='BDLMinerV2/2.5.4 M5StickC Plus2',
                           authorised=True, subscribed=True, dsps1=1, dsps5=2, dsps60=.5,
                           bestdiff=4321, starttime=int(time.time())-10000, lastshare=int(time.time())-5000,
                           address='PRIVATE_TEST', enonce1='PRIVATE_TEST', password='PRIVATE_TEST')

    def write_presence(self, clients=None, available=True, stamp=None):
        self.fixture.write(self.path, dict(schemaVersion=1, available=available,
                           updatedAt=stamp or api.epoch(time.time()), checkedAt=api.epoch(time.time()),
                           clients=presence.filter_clients([self.client] if clients is None else clients, b'test-salt')))

    def test_native_ipc_framing_and_filtered_snapshot(self):
        socket_path = self.fixture.root/'stratifier'
        with socket.socket(socket.AF_UNIX) as listener:
            listener.bind(str(socket_path));listener.listen(1)
            seen = []
            def reply():
                connection, _ = listener.accept()
                with connection:
                    size = struct.unpack('<I', connection.recv(4))[0]
                    seen.append(connection.recv(size))
                    payload = json.dumps(dict(clients=[self.client])).encode()
                    connection.sendall(struct.pack('<I', len(payload)))
                    connection.sendall(payload[:15]);connection.sendall(payload[15:])
            thread = threading.Thread(target=reply, daemon=True);thread.start()
            result = presence.collect(socket_path, self.path, b'test-salt')
            thread.join(timeout=2)
        self.assertEqual(seen, [b'clients'])
        saved = json.loads(self.path.read_text())
        self.assertEqual(saved['updatedAt'], result)
        self.assertEqual(saved['clients'][0]['hashrateHps']['5m'], 2*4294967296)
        self.assertNotIn('PRIVATE_TEST', self.path.read_text())
        self.assertEqual(self.path.stat().st_mode & 0o777, 0o600)

    def test_online_table_never_exposes_full_address_or_raw_agent(self):
        self.write_presence()
        status, _, result = self.request('/api/v1/online')
        self.assertEqual(status, 200);self.assertEqual(result['total'], 1)
        self.assertTrue(result['available'])
        encoded = json.dumps(result)
        self.assertNotIn(ADDRESS, encoded)
        self.assertNotIn('workername', encoded)
        self.assertNotIn('useragent', encoded)
        row = result['miners'][0]
        self.assertEqual(row['addressMasked'], ADDRESS[:6]+'…'+ADDRESS[-4:])
        self.assertEqual(row['worker'], 'bitaxe-01')
        self.assertEqual(row['device']['model'], 'M5StickC Plus2')
        self.assertEqual(row['device']['software'], 'BDLMiner V2')
        # A worker suffix containing the wallet must not bypass masking.
        self.client['workername'] = ADDRESS+'.'+ADDRESS
        self.write_presence()
        self.assertNotIn(ADDRESS, json.dumps(self.request('/api/v1/online')[2]))

    def test_presence_is_connection_based_not_a_last_share_guess(self):
        self.write_presence()
        worker = self.request('/api/v1/miners/'+ADDRESS+'/workers/bitaxe-01')[2]['worker']
        self.assertTrue(worker['online'])  # Its last share was hours ago; it is still connected.
        self.assertEqual(worker['devices'][0]['model'], 'M5StickC Plus2')
        self.write_presence([])
        self.assertFalse(self.request('/api/v1/miners/'+ADDRESS+'/workers/bitaxe-01')[2]['worker']['online'])
        self.assertEqual(self.request('/api/v1/online')[2]['total'], 0)

    def test_unavailable_or_stale_presence_is_not_reported_as_zero_or_online(self):
        for available, stamp in ((False, api.epoch(time.time())), (True, api.epoch(time.time()-60))):
            self.write_presence(available=available, stamp=stamp)
            result = self.request('/api/v1/online')[2]
            self.assertFalse(result['available']);self.assertIsNone(result['total']);self.assertEqual(result['miners'], [])
            miner = self.request('/api/v1/miners/'+ADDRESS)[2]
            self.assertIsNone(miner['connections']);self.assertFalse(miner['presenceAvailable'])
            self.assertIsNone(self.request('/api/v1/miners/'+ADDRESS+'/workers/bitaxe-01')[2]['worker']['online'])

    def test_failed_collector_preserves_real_last_success_date(self):
        old = api.epoch(time.time()-100)
        result = presence.collect(self.fixture.root/'missing-socket', self.path, b'test', old)
        self.assertEqual(result, old)
        saved = json.loads(self.path.read_text())
        self.assertFalse(saved['available']);self.assertEqual(saved['updatedAt'], old);self.assertEqual(saved['clients'], [])

    def test_user_agent_detection_does_not_invent_hardware(self):
        cases = [
            ('BDLMinerV2/2.5.4', None, 'BDLMiner V2'),
            ('NerdMinerV2', None, 'NerdMiner V2'),
            ('ESP-Miner/2.7.0', None, 'ESP-Miner'),
            ('ESP-Miner/2.7.0 Bitaxe Gamma', 'Bitaxe Gamma', 'ESP-Miner'),
            ('cgminer/4.11.1 Antminer S19 Pro', 'Antminer S19 PRO', 'cgminer'),
            ('<img src=x onerror=alert(1)> PRIVATE_TEST', None, None)]
        for agent, model, software in cases:
            with self.subTest(agent=agent):
                result = api.device_from_agent(agent)
                self.assertEqual(result['model'], model);self.assertEqual(result['software'], software)
                self.assertNotIn('PRIVATE_TEST', json.dumps(result))

    def test_unauthorized_clients_excluded_and_ids_are_opaque(self):
        second = dict(self.client, id=124, authorised=False)
        bad = dict(self.client, id=125, workername='../../PRIVATE_TEST')
        rows = presence.filter_clients([self.client, second, bad], b'one-salt')
        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]['id'], presence.filter_clients([self.client], b'one-salt')[0]['id'])
        self.assertNotEqual(rows[0]['id'], presence.filter_clients([self.client], b'two-salt')[0]['id'])

    def test_online_pagination_and_sync_state(self):
        self.write_presence([self.client, dict(self.client, id=124, workername=OTHER+'.second')])
        result = self.request('/api/v1/online?limit=1&offset=1')[2]
        self.assertEqual(result['total'], 2);self.assertEqual(len(result['miners']), 1)
        self.assertEqual(self.request('/api/v1/online?limit=0')[0], 400)
        self.fixture.sample['node']['initialBlockDownload'] = True
        self.fixture.write(self.fixture.snapshot, self.fixture.sample)
        self.write_presence(available=False)
        self.assertEqual(self.request('/api/v1/online')[2]['status'], 'syncing')


if __name__ == '__main__':
    unittest.main()
