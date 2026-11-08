"""Run the installer's read-only port preflight under simulated Linux policies."""
from pathlib import Path
from pathlib import PosixPath
import json
import re
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch


class PortPreflightTests(unittest.TestCase):
    def preflight(self, occupied=None, active=(), failure=False):
        script = Path(__file__).with_name('install.sh').read_text()
        source = re.search(r"python3 - <<'PY'\n(.*?)\nPY", script, re.S)[1]
        def run(command, **kwargs):
            if command[0] == 'systemctl':
                return subprocess.CompletedProcess(command, 0 if command[-1] in active else 3)
            if command[0] == 'ss':
                port = int(command[-1].split(':')[-1])
                return subprocess.CompletedProcess(command, 1 if failure else 0,
                                                   'LISTEN 0 128 0.0.0.0:'+str(port) if occupied == port else '', '')
            raise AssertionError('Unexpected command: '+str(command))
        with patch('subprocess.run', side_effect=run), patch('socket.socket.bind', side_effect=PermissionError('Privileged port')):
            exec(compile(source, 'installer-port-preflight', 'exec'), {})

    def test_free_ports_do_not_require_bind_privileges(self):
        self.preflight()

    def test_real_listener_still_blocks_installation(self):
        with self.assertRaisesRegex(SystemExit, 'port 80 est occupé'):
            self.preflight(occupied=80)

    def test_own_services_can_keep_their_ports_on_reinstall(self):
        self.preflight(occupied=80, active=('caddy.service', 'bdl-pool-api.service'))

    def test_failed_inspection_is_not_reported_as_a_busy_port(self):
        with self.assertRaisesRegex(SystemExit, 'Impossible de vérifier'):
            self.preflight(failure=True)


class DomainConfigurationTests(unittest.TestCase):
    def render(self, relay):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        home = root/'home/bdl'
        script = home/'bdl-pool-site/api'
        script.mkdir(parents=True)
        (script.parent/'config.js').write_text('publicHost: "bdl-pool.duckdns.org"')
        source = home/'mining/ckpool/local.conf'
        source.parent.mkdir(parents=True)
        source.write_text('{}')
        public = root/'etc/bdl-pool-openport/public.json'
        public.parent.mkdir(parents=True)
        public.write_text(json.dumps(relay))
        destination = home/'api.json'
        destination.write_text('{"original":true}')
        caddy = root/'staged.caddy'
        installer = Path(__file__).with_name('install.sh').read_text()
        code = re.search(r'python3 - "\$script_dir" .*?<<\x27PY\x27\n(.*?)\nPY', installer, re.S)[1]
        def mapped(*parts):
            path = PosixPath(*parts)
            if str(path).startswith(('/home/bdl', '/etc/')):
                path = root/str(path).lstrip('/')
            return path
        with patch('pathlib.Path', side_effect=mapped), patch.object(sys, 'argv', ['installer', str(script), str(destination), str(caddy)]):
            exec(compile(code, 'installer-domain-config', 'exec'), {})
        return json.loads(destination.read_text()), caddy.read_text()

    def test_reinstall_supports_old_and_custom_api_domains_with_their_own_certificate(self):
        relay = dict(domain='bdl-pool.duckdns.org', managedTLS=True,
                     v1Port=39991, v2Port=36809, apiPublicPort=36018)
        settings, fragment = self.render(relay)
        self.assertEqual(settings['apiHost'], 'bdl-pool.duckdns.org')
        self.assertIn('tls /etc/caddy/bdl-pool-tls/current/', fragment)
        settings, fragment = self.render(dict(relay, domain='bdl-pool.com', previousDomain='bdl-pool.duckdns.org',
                                             apiDomain='api.bdl-pool.com', tlsDirectory='/etc/caddy/bdl-pool-domain-tls'))
        self.assertEqual(settings['host'], 'bdl-pool.com')
        self.assertEqual(settings['apiHost'], 'api.bdl-pool.com')
        self.assertEqual(settings['v1Port'], 39991)
        self.assertTrue(fragment.startswith('api.bdl-pool.com {'))
        self.assertIn('tls /etc/caddy/bdl-pool-domain-tls/current/', fragment)

    def test_unrelated_domain_and_unexpected_certificate_path_are_rejected(self):
        relay = dict(domain='bdl-pool.duckdns.org', managedTLS=True,
                     v1Port=39991, v2Port=36809, apiPublicPort=36018)
        for change in (dict(domain='unrelated.example'), dict(tlsDirectory='/tmp/certificates')):
            with self.subTest(change=change), self.assertRaises(SystemExit):
                self.render(dict(relay, **change))


if __name__ == '__main__':
    unittest.main()
