import importlib.util
import json
from pathlib import Path
import ssl
import tempfile
import unittest
from unittest.mock import patch
from urllib.error import HTTPError, URLError

spec = importlib.util.spec_from_file_location('openport_health', Path(__file__).with_name('openport-service.py'))
service = importlib.util.module_from_spec(spec)
spec.loader.exec_module(service)


class APIHealthTests(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.manifest = Path(directory.name)/'public.json'
        self.manifest.write_text(json.dumps(dict(apiDomain='api.bdl-pool.com', managedTLS=True, apiPublicPort=36018)))
        self.public = dict(host='openport.pw', port=36018)
        local = patch.object(service, 'local_api_healthy', return_value=True)
        self.local_probe = local.start()
        self.addCleanup(local.stop)

    def test_public_failure_requires_a_healthy_local_api(self):
        with patch.object(service, 'urlopen', side_effect=URLError('timeout')):
            self.assertIs(service.api_transport(self.public, self.manifest), False)
        self.local_probe.return_value = False
        with patch.object(service, 'urlopen', side_effect=AssertionError('No public check when local API is down')) as call:
            self.assertIsNone(service.api_transport(self.public, self.manifest))
            self.assertEqual(call.call_count, 0)

    def test_http_error_is_a_working_tunnel_and_expired_certificate_is_not_repaired_by_reconnecting(self):
        with patch.object(service, 'urlopen', side_effect=HTTPError('https://api.example', 502, 'backend', {}, None)):
            self.assertIs(service.api_transport(self.public, self.manifest), True)
        with patch.object(service, 'urlopen', side_effect=URLError(ssl.SSLCertVerificationError('expired'))):
            self.assertIsNone(service.api_transport(self.public, self.manifest))

    def test_bad_manifest_does_not_probe_another_host_or_port(self):
        for settings in (dict(apiDomain='bad/host', managedTLS=True, apiPublicPort=36018),
                         dict(apiDomain='api.bdl-pool.com', managedTLS=True, apiPublicPort=36019)):
            self.manifest.write_text(json.dumps(settings))
            with patch.object(service, 'urlopen', side_effect=AssertionError('No network probe')):
                self.assertIsNone(service.api_transport(self.public, self.manifest))

    def test_only_three_consecutive_public_failures_request_a_reconnect(self):
        probe = service.APIHealth()
        sequence = [False, False, True, False, False, None, False, False, False]
        with patch.object(service, 'api_transport', side_effect=sequence):
            self.assertEqual([probe.reconnect(self.public) for _ in sequence], [False]*8+[True])


if __name__ == '__main__':
    unittest.main()
