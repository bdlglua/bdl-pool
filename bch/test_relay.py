"""BCH provisioning is isolated; tests never register a real provider identity."""
import contextlib
import importlib.util
import io
import json
from pathlib import Path
import shutil
import sys
import tempfile
import threading
import unittest
from unittest.mock import Mock, patch
from urllib.request import urlopen

spec = importlib.util.spec_from_file_location('bch_relay', Path(__file__).with_name('relay.py'))
relay = importlib.util.module_from_spec(spec)
spec.loader.exec_module(relay)


class BCHRelayTests(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)/'profiles'
        self.root.mkdir()
        self.old = {}
        for index, name in enumerate(('v1', 'v2', 'api')):
            profile = self.root/name
            profile.mkdir()
            identity = dict(client_id='old-'+name, token='SECRET_OLD_'+name, remote_port=48001+index)
            for filename, data in (('identity.json', identity), ('public.json', dict(host='openport.pw', port=48001+index))):
                path = profile/filename
                path.write_text(json.dumps(data))
                self.old[path] = path.read_bytes()
        self.identity = dict(client_id='new-bch', token='SECRET_NEW_BCH', remote_port=48004)
        self.calls = []

    def provider(self, action, data):
        self.calls.append((action, data))
        if action == 'register':
            return dict(status='ok', **self.identity)
        if action == 'connect':
            return dict(status='ok', server_host='openport.pw', control_port=2333)
        return dict(status='ok')

    def fake_probe(self, binary, profile, identity):
        public = relay.client.connect(identity, relay.LOCAL_PORT, profile)
        (profile/'client.toml').unlink()
        return public

    def test_reserve_verifies_reuse_and_keeps_all_btc_files_and_tokens_private(self):
        with patch.object(relay.client, 'api', side_effect=self.provider), \
                patch.object(relay.client, 'verified_binary', return_value=Path('/verified/client')), \
                patch.object(relay, 'probe_transport', side_effect=self.fake_probe) as probe, \
                contextlib.redirect_stdout(io.StringIO()) as output:
            public = relay.reserve(self.root)
        self.assertEqual(public, dict(host='openport.pw', port=48004))
        self.assertEqual(probe.call_count, 2)
        registrations = [payload for action, payload in self.calls if action == 'register']
        self.assertEqual(registrations, [{}, dict(client_id='new-bch', token='SECRET_NEW_BCH')])
        self.assertNotIn('SECRET', output.getvalue())
        self.assertNotIn('client_id', output.getvalue())
        for path, content in self.old.items():
            self.assertEqual(path.read_bytes(), content)
        for name in ('identity.json', 'public.json', 'transport-check.json'):
            self.assertEqual((self.root/'bch'/name).stat().st_mode & 0o777, 0o600)
        self.assertFalse(json.loads((self.root/'bch/transport-check.json').read_text())['miningVerified'])
        self.assertEqual(relay.route_for_pool(self.root/'bch'), public)

    def test_colliding_identity_or_port_never_connects_or_releases_a_btc_profile(self):
        self.identity['remote_port'] = 48001
        with patch.object(relay.client, 'api', side_effect=self.provider), \
                patch.object(relay.client, 'verified_binary', return_value='/verified/client'), \
                patch.object(relay, 'probe_transport') as probe, contextlib.redirect_stdout(io.StringIO()):
            with self.assertRaises(relay.client.TrialError):
                relay.reserve(self.root)
        probe.assert_not_called()
        self.assertEqual([action for action, _ in self.calls], ['register'])
        for path, content in self.old.items():
            self.assertEqual(path.read_bytes(), content)

    def test_missing_or_broken_identity_never_allocates_replacement(self):
        profile = relay.profile_directory(self.root)
        (profile/'public.json').write_text('{"host":"openport.pw","port":48004}')
        with patch.object(relay.client, 'api', side_effect=AssertionError('No provider mutation')):
            with self.assertRaises(relay.client.TrialError):
                relay.reserve(self.root)
        (profile/'identity.json').write_text('{broken')
        with patch.object(relay.client, 'api', side_effect=AssertionError('No provider mutation')), \
                patch.object(relay.client, 'verified_binary', return_value='/verified/client'):
            with self.assertRaises(relay.client.TrialError):
                relay.reserve(self.root)

    def test_failed_probe_leaves_no_verification_record(self):
        with patch.object(relay.client, 'api', side_effect=self.provider), \
                patch.object(relay.client, 'verified_binary', return_value='/verified/client'), \
                patch.object(relay, 'probe_transport', side_effect=relay.client.TrialError('unreachable')), \
                contextlib.redirect_stdout(io.StringIO()):
            with self.assertRaises(relay.client.TrialError):
                relay.reserve(self.root)
        self.assertTrue((self.root/'bch/identity.json').exists())
        self.assertFalse((self.root/'bch/transport-check.json').exists())

    def test_instance_unit_and_service_mapping_only_target_bch(self):
        unit = relay.unit_text()
        self.assertIn('openport-service.py bch', unit)
        self.assertIn('BindPaths=/home/bdl/.local/state/bdl-pool-openport/bch', unit)
        self.assertNotIn('%i', unit)
        self.assertNotIn('3333', unit)
        spec = importlib.util.spec_from_file_location('relay_service', Path(__file__).parents[1]/'monitoring/openport-service.py')
        service = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(service)
        self.assertEqual(service.PORTS, dict(v1=3333, v2=3336, api=443, bch=3334))

    def test_public_probe_requires_the_request_to_reach_the_local_responder(self):
        profile = relay.profile_directory(self.root)
        server, completed, path = relay.client.test_server()
        address = f'http://127.0.0.1:{server.server_port}{path}'
        class Forwarder:
            def __init__(self):
                self.running = True
            def poll(self):
                return None if self.running else 0
            def terminate(self):
                self.running = False
            def wait(self, **_):
                return 0
        forwarder = Forwarder()
        class LocalOpener:
            def open(self, request, **kwargs):
                # Only the incoming public URL is mapped here; use a real local
                # HTTP request so completion cannot be inferred from the body alone.
                return urlopen(address, **kwargs)
        with patch.object(relay.client, 'test_server', return_value=(server, completed, path)), \
                patch.object(relay.client, 'api', side_effect=self.provider), \
                patch.object(relay.subprocess, 'Popen', return_value=forwarder), \
                patch.object(relay, 'build_opener', return_value=LocalOpener()):
            self.assertEqual(relay.probe_transport('/verified/client', profile, self.identity)['port'], 48004)
        self.assertTrue(completed.is_set())
        self.assertFalse(forwarder.running)
        self.assertFalse((profile/'client.toml').exists())
        self.assertEqual(self.calls[-1][0], 'release')

    def test_process_start_failure_releases_only_bch_and_removes_secret_config(self):
        profile = relay.profile_directory(self.root)
        with patch.object(relay.client, 'api', side_effect=self.provider), \
                patch.object(relay.subprocess, 'Popen', side_effect=OSError('Missing executable')):
            with self.assertRaises(OSError):
                relay.probe_transport('/missing', profile, self.identity)
        self.assertFalse((profile/'client.toml').exists())
        self.assertEqual(self.calls[-1], ('release', dict(client_id='new-bch', token='SECRET_NEW_BCH')))
        for target, content in self.old.items():
            self.assertEqual(target.read_bytes(), content)

    def test_expected_body_without_a_local_request_does_not_validate_the_relay(self):
        profile = relay.profile_directory(self.root)
        completed = threading.Event()
        server = Mock(server_port=12345)
        process = Mock()
        process.poll.return_value = None
        response = Mock(status=200)
        response.read.return_value = b'BDL-OPENPORT-OK\n'
        response.__enter__ = Mock(return_value=response)
        response.__exit__ = Mock(return_value=False)
        opener = Mock()
        opener.open.return_value = response
        with patch.object(relay.client, 'test_server', return_value=(server, completed, '/random-probe')), \
                patch.object(relay.client, 'api', side_effect=self.provider), \
                patch.object(relay.subprocess, 'Popen', return_value=process), \
                patch.object(relay, 'build_opener', return_value=opener), \
                patch.object(relay.time, 'monotonic', side_effect=[0, 0, 26]), \
                patch.object(relay.time, 'sleep'), patch.object(completed, 'wait', return_value=False):
            with self.assertRaises(relay.client.TrialError):
                relay.probe_transport('/verified/client', profile, self.identity)
        self.assertFalse(completed.is_set())
        process.terminate.assert_called_once()
        server.shutdown.assert_called_once()

    def test_changed_verification_record_is_not_used_for_pool_setup(self):
        profile = relay.profile_directory(self.root)
        with patch.object(relay.client, 'api', side_effect=self.provider), contextlib.redirect_stdout(io.StringIO()):
            identity = relay.client.credentials(profile)
            relay.client.connect(identity, 3334, profile)
        (profile/'transport-check.json').write_text(json.dumps(dict(host='openport.pw', port=48005, targetPort=3334, probe='temporary_loopback_http')))
        with self.assertRaises(relay.client.TrialError):
            relay.route_for_pool(profile)

    def test_pool_configuration_reads_only_the_verified_bch_endpoint(self):
        with patch.object(relay.client, 'api', side_effect=self.provider), \
                patch.object(relay.client, 'verified_binary', return_value='/verified/client'), \
                patch.object(relay, 'probe_transport', side_effect=self.fake_probe), contextlib.redirect_stdout(io.StringIO()):
            relay.reserve(self.root)
        home = self.root.parent/'home'
        shutil.copytree(self.root, home/'.local/state/bdl-pool-openport')
        spec = importlib.util.spec_from_file_location('bch_configure', Path(__file__).with_name('configure.py'))
        setup = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(setup)
        stage = self.root.parent/'stage'
        with patch.object(Path, 'home', return_value=home), patch.object(sys, 'argv', [
                'configure.py', '--payout', 'bitcoincash:qpm2qsznhks23z7629mms6s4cwef74vcwvy22gdx6a',
                '--from-relay', '--destdir', str(stage)]), contextlib.redirect_stdout(io.StringIO()) as output:
            setup.main()
        profile = json.loads((stage/'home/bdl/.config/bdl-pool-bch/api.json').read_text())
        self.assertEqual((profile['host'], profile['v1Port']), ('openport.pw', 48004))
        self.assertNotIn('SECRET', output.getvalue())

    def test_a_dangling_identity_symlink_never_requests_a_new_identity(self):
        profile = relay.profile_directory(self.root)
        (profile/'identity.json').symlink_to(profile/'missing-private-file')
        with patch.object(relay.client, 'api', side_effect=AssertionError('No provider mutation')):
            with self.assertRaises(relay.client.TrialError):
                relay.reserve(self.root)


if __name__ == '__main__':
    unittest.main()
