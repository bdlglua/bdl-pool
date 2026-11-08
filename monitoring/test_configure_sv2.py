"""Configuration transaction tests; native cryptography is checked on the pool VM."""
import contextlib
import importlib.util
import io
import json
import os
from pathlib import Path
import stat
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('configure_sv2', Path(__file__).with_name('configure-sv2.py'))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
# Public vector from CKPool's authority encoding test; never a production key.
PUBLIC_KEY = '9bXiEd8boQVhq7WddEcERUL5tyyJVFYdU8th3HfbNXK3Yw6GRXh'
ORIGINAL = ('{"serverurl":["0.0.0.0:3333"],"btcd":[{"url":"127.0.0.1:8332",'
            '"auth":"owner","pass":"PRIVATE_TEST_RPC"}],"donation":0}\n'
            'Comments after JSON are ignored.\n')


class ConfigurationTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix='bdl-sv2-config-test-')
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.home = self.root/'home'
        self.config = self.home/'mining/ckpool/local.conf'
        self.config.parent.mkdir(parents=True)
        self.config.write_text(ORIGINAL)
        self.config.chmod(0o600)
        self.tool = self.root/'fake-keytool'
        self.tool.write_text('''#!/usr/bin/python3
import os, sys
from pathlib import Path
for name in sys.argv[1:]:
    path = Path(name)
    if not path.exists():
        fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        os.write(fd, b'1'*64+b'\\n')
        os.close(fd)
print('NATIVE_LOG=PRIVATE_TEST_RPC')
print('SV2_AUTHORITY_PUBLIC_KEY=' + ''' + repr(PUBLIC_KEY) + ''')
''')
        self.tool.chmod(0o700)

    def run_configuration(self, verifier=None):
        return module.configure(self.home, self.root/'ckpool', keytool=self.tool,
                                verifier=verifier or self.accept)

    def accept(self, binary, candidate):
        data, _ = json.JSONDecoder().raw_decode(candidate.read_text())
        self.assertEqual(data['serverurl'], ['0.0.0.0:3333'])
        self.assertEqual(data['sv2url'], ['0.0.0.0:3336'])
        self.assertEqual(data['btcd'][0]['pass'], 'PRIVATE_TEST_RPC')
        return True

    def test_transaction_backup_permissions_and_reuse(self):
        with contextlib.redirect_stdout(io.StringIO()) as output:
            public, backup = self.run_configuration()
        self.assertEqual(public, PUBLIC_KEY)
        self.assertNotIn('PRIVATE_TEST_RPC', output.getvalue())
        self.assertEqual(backup.read_text(), ORIGINAL)
        self.assertEqual(stat.S_IMODE(backup.stat().st_mode), 0o600)
        data, end = json.JSONDecoder().raw_decode(self.config.read_text())
        self.assertEqual(self.config.read_text()[end:].strip(), 'Comments after JSON are ignored.')
        self.assertEqual(data['donation'], 0)
        keys = [Path(data[name]) for name in ('sv2_authority_key', 'sv2_static_key')]
        contents = [p.read_bytes() for p in keys]
        self.assertTrue(all(stat.S_IMODE(p.stat().st_mode) == 0o600 for p in keys))
        self.assertEqual(self.run_configuration(), (PUBLIC_KEY, None))
        self.assertEqual([p.read_bytes() for p in keys], contents)
        self.assertEqual(stat.S_IMODE(self.config.stat().st_mode), 0o600)
        public_file = self.home/'.local/state/bdl-pool-monitor/sv2-public.json'
        self.assertEqual(json.loads(public_file.read_text()),
                         dict(authorityPublicKey=PUBLIC_KEY, stratumPort=3336, publicPort=3336))

    def test_failed_validation_preserves_original(self):
        with self.assertRaises(ValueError):
            self.run_configuration(lambda *_: False)
        self.assertEqual(self.config.read_text(), ORIGINAL)
        self.assertFalse(list(self.config.parent.glob('local.conf.before-sv2-*')))
        self.assertFalse(list(self.config.parent.glob('.local-sv2-*')))

    def test_concurrent_edit_is_preserved(self):
        def edit(binary, candidate):
            self.config.write_text(ORIGINAL+'# changed\n')
            return True
        with self.assertRaises(ValueError):
            self.run_configuration(edit)
        self.assertEqual(self.config.read_text(), ORIGINAL+'# changed\n')

    def test_key_failure_filters_native_output(self):
        self.tool.write_text('#!/usr/bin/python3\nprint("PRIVATE_TEST_RPC")\nraise SystemExit(1)\n')
        with self.assertRaises(ValueError) as error:
            self.run_configuration()
        self.assertNotIn('PRIVATE_TEST_RPC', str(error.exception))
        self.assertEqual(self.config.read_text(), ORIGINAL)

    def test_duplicate_json_keys_and_other_bind_are_rejected(self):
        for text in ('{"serverurl":[],"serverurl":["0.0.0.0:3333"]}',
                     '{"sv2url":["0.0.0.0:4444"]}'):
            self.config.write_text(text)
            with self.assertRaises(ValueError):
                self.run_configuration()
            self.assertEqual(self.config.read_text(), text)

    def test_public_key_checksum(self):
        self.assertTrue(module.public_key_valid(PUBLIC_KEY))
        self.assertFalse(module.public_key_valid(PUBLIC_KEY[:-1]+'1'))

    def test_native_config_check_rejects_missing_support_or_wrong_port(self):
        good = 'sv2_compiled = 1\nsv2urls = 1\nserverurl[1] = 0.0.0.0:3336 type=sv2 highdiff=0\n'
        for text, valid in ((good, True), (good.replace('3336', '4444'), False),
                            (good.replace('compiled = 1', 'compiled = 0'), False)):
            with patch.object(module.subprocess, 'run') as run:
                run.return_value = module.subprocess.CompletedProcess([], 0, text, '')
                self.assertEqual(bool(module.verify_configuration('ckpool', 'candidate')), valid)


if __name__ == '__main__':
    unittest.main()
