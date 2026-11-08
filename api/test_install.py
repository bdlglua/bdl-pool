"""Run the installer's read-only port preflight under simulated Linux policies."""
from pathlib import Path
import re
import subprocess
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


if __name__ == '__main__':
    unittest.main()
