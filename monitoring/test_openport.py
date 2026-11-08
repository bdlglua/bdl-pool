import contextlib
import importlib.util
import io
import json
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import tempfile
import threading
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('openport_trial', Path(__file__).with_name('try-openport.py'))
trial = importlib.util.module_from_spec(spec)
spec.loader.exec_module(trial)


class OpenPortTests(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.profile = Path(directory.name)
        self.identity = dict(client_id='trial-profile', token='PRIVATE_TEST_TOKEN_1234', remote_port=48001)
        self.calls = []

    def provider(self, action, data):
        self.calls.append((action, data))
        if action == 'register':
            return dict(status='ok', **self.identity)
        if action == 'connect':
            return dict(status='ok', server_host='openport.pw', control_port=2333)
        return dict(status='ok')

    def test_restart_reuses_identity_and_never_prints_credentials(self):
        output = io.StringIO()
        with patch.object(trial, 'api', side_effect=self.provider), contextlib.redirect_stdout(output):
            first = trial.credentials(self.profile)
            public = trial.connect(first, 12345, self.profile)
            second = trial.credentials(self.profile)
            self.assertEqual(trial.connect(second, 12346, self.profile), public)
        registrations = [data for action, data in self.calls if action == 'register']
        self.assertEqual(registrations, [{}, {'client_id': first['client_id'], 'token': first['token']}])
        self.assertEqual(first, second)
        self.assertEqual(public, dict(host='openport.pw', port=48001))
        self.assertNotIn(first['token'], output.getvalue())
        self.assertNotIn(first['client_id'], output.getvalue())
        for filename in ('identity.json', 'client.toml'):
            self.assertEqual((self.profile/filename).stat().st_mode & 0o777, 0o600)

    def test_changed_port_or_broken_identity_never_allocates_replacement(self):
        with patch.object(trial, 'api', side_effect=self.provider):
            trial.credentials(self.profile)
        before = (self.profile/'identity.json').read_bytes()
        self.identity['remote_port'] = 48002
        with patch.object(trial, 'api', side_effect=self.provider):
            with self.assertRaises(trial.TrialError):
                trial.credentials(self.profile)
        self.assertEqual((self.profile/'identity.json').read_bytes(), before)
        self.assertTrue(self.calls[-1][1])
        (self.profile/'identity.json').write_text('{broken')
        with patch.object(trial, 'api', side_effect=AssertionError('Must not re-register corrupt identity')):
            with self.assertRaises(trial.TrialError):
                trial.credentials(self.profile)

    def test_changed_route_is_not_silently_adopted(self):
        with patch.object(trial, 'api', side_effect=self.provider):
            trial.connect(self.identity, 12345, self.profile)
        previous = (self.profile/'public.json').read_bytes()
        with patch.object(trial, 'api', return_value=dict(status='ok', server_host='other.openport.pw', control_port=2333)):
            with self.assertRaises(trial.TrialError):
                trial.connect(self.identity, 12345, self.profile)
        self.assertEqual((self.profile/'public.json').read_bytes(), previous)
        for host in ('127.0.0.1', '192.168.1.2', '169.254.169.254', 'openport.pw.evil.example'):
            with self.assertRaises(trial.TrialError):
                trial.relay_host(host)

    def test_api_does_not_forward_token_or_echo_error_body_on_redirect(self):
        received = []
        class Handler(BaseHTTPRequestHandler):
            def do_POST(self):
                received.append(self.path)
                self.send_response(307)
                self.send_header('Location', '/unexpected')
                self.end_headers()
                self.wfile.write(b'PRIVATE_TEST_TOKEN_1234')
            def log_message(self, *_):
                pass
        server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        threading.Thread(target=server.serve_forever, daemon=True).start()
        try:
            with patch.object(trial, 'PROVIDER', f'http://127.0.0.1:{server.server_port}'):
                with self.assertRaises(trial.TrialError) as caught:
                    trial.api('connect', self.identity)
            self.assertNotIn(self.identity['token'], str(caught.exception))
            self.assertEqual(received, ['/api/connect'])
        finally:
            server.shutdown()
            server.server_close()

    def test_failed_process_start_releases_tunnel_and_removes_secret_config(self):
        with patch.object(trial, 'api', side_effect=self.provider), patch.object(trial.subprocess, 'Popen', side_effect=OSError('Unavailable executable')):
            with self.assertRaises(OSError):
                trial.run_trial('/missing', 'test', self.profile, 1)
        self.assertFalse((self.profile/'client.toml').exists())
        self.assertTrue((self.profile/'identity.json').exists())
        self.assertEqual(self.calls[-1][0], 'release')


if __name__ == '__main__':
    unittest.main()
