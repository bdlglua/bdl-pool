import contextlib
import importlib.util
import io
import json
from pathlib import Path
import socket
import tempfile
import threading
import time
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('collect', Path(__file__).with_name('collect.py'))
collect = importlib.util.module_from_spec(spec)
spec.loader.exec_module(collect)


class CollectorTests(unittest.TestCase):
    def test_ckpool_multiline_json_filtering_units_and_stale_data(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'pool.status'
            now = time.time()
            path.write_text('\n'.join(json.dumps(v) for v in [
                {'lastupdate': now, 'Workers': 2, 'Users': 1, 'secret': 'DO_NOT_PUBLISH'},
                {'hashrate5m': '1.25T'}, {'bestshare': 42, 'accepted': 100}]))
            data, check = collect.pool_info(path, now)
            self.assertEqual(check, 'ok')
            self.assertEqual(data, dict(ready=False, hashrateHps=1.25e12,
                                       workers=2, bestShare=42, blocksFound=None))
            self.assertNotIn('DO_NOT_PUBLISH', json.dumps(data))
            data, check = collect.pool_info(path, now + 181)
            self.assertEqual(check, 'stale')
            self.assertIsNone(data['hashrateHps'])
            path.write_text('{"Workers":2}\n{"bestshare":')
            self.assertEqual(collect.pool_info(path, now)[1], 'invalid')
            self.assertEqual(collect.pool_info(path.with_name('absent'), now)[1], 'missing')

    def test_unknown_is_not_zero_and_units_are_not_guessed(self):
        for value in (None, True, -1, float('nan'), float('inf'), 'bad'):
            self.assertIsNone(collect.hashrate(value))
        self.assertEqual(collect.hashrate('0'), 0)
        self.assertEqual(collect.hashrate('7.5GH/s'), 7.5e9)
        self.assertEqual(collect.hashrate('10k'), 10000)
        self.assertIsNone(collect.hashrate('10watts'))
        self.assertIsNone(collect.number(1.5, integer=True))

    def test_node_failure_does_not_publish_errors_or_retained_state(self):
        with patch.object(collect.subprocess, 'run', side_effect=OSError('SECRET')), contextlib.redirect_stderr(io.StringIO()) as errors:
            data, check = collect.node_info('/missing')
        self.assertEqual(check, 'unavailable')
        self.assertTrue(all(value is None for value in data.values()))
        self.assertNotIn('SECRET', errors.getvalue())

    def test_node_allowlist(self):
        source = dict(chain='main', blocks=12, headers=20,
                      verificationprogress=0.2, initialblockdownload=True,
                      rpcpassword='SECRET', warnings=['SECRET'])
        with patch.object(collect.subprocess, 'run', return_value=type('Result', (), {'stdout': json.dumps(source)})()):
            data, check = collect.node_info('/usr/local/bin/bitcoin-cli')
        self.assertEqual(check, 'ok')
        self.assertEqual(data['blocks'], 12)
        self.assertNotIn('SECRET', json.dumps(data))

    def test_relative_logdir_and_non_json_trailing_content(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            config = root / 'ckpool.conf'
            config.write_text('{"logdir":"logs","rpcpassword":"SECRET"}\ntrailing text')
            path = root / 'logs/pool/pool.status'
            path.parent.mkdir(parents=True)
            path.touch()
            with patch.object(collect.subprocess, 'run', side_effect=OSError):
                self.assertEqual(collect.pool_status_path(config, None), path)

    def test_stratum_requires_subscription_and_job(self):
        for with_job in (False, True):
            with socket.socket() as server:
                server.bind(('127.0.0.1', 0))
                server.listen(1)
                def reply():
                    with server.accept()[0] as client:
                        request = client.recv(8192)
                        self.assertNotIn(b'mining.authorize', request)
                        messages = [dict(id=1, error=None, result=[[['mining.notify', 'session']], 'abcd', 8])]
                        if with_job:
                            messages.append(dict(method='mining.notify', params=['job', '0'*64, '', '', [], '20000000', '1d00ffff', '12345678', True]))
                        client.sendall(('\n'.join(json.dumps(v) for v in messages)+'\n').encode())
                thread = threading.Thread(target=reply)
                thread.start()
                result = collect.stratum_check('127.0.0.1', server.getsockname()[1], timeout=1)
                thread.join()
                self.assertEqual(result, 'job_received' if with_job else 'subscribed_no_job')


if __name__ == '__main__':
    unittest.main()
