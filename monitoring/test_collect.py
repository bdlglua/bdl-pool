import contextlib
import importlib.util
import io
import json
from pathlib import Path
import socket
import sys
import tempfile
import threading
import time
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('collect', Path(__file__).with_name('collect.py'))
collect = importlib.util.module_from_spec(spec)
spec.loader.exec_module(collect)


class CollectorTests(unittest.TestCase):
    def test_bch_profile_uses_separate_cli_datadir_logs_and_network_marker(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            profile = root/'monitor.json'
            profile.write_text(json.dumps(dict(coin='bch', bitcoinCli='/opt/bchn/bitcoin-cli',
                cliArgs=['-datadir=/srv/bch/node'], config='/srv/bch/cashstratum.conf',
                poolStatus='/srv/bch/logs/pool/pool.status', stratumPort=3334)))
            node = dict(chain='main', blocks=123, headers=123, verificationProgress=1, initialBlockDownload=False)
            pool = dict(ready=False, hashrateHps=12, workers=1, bestShare=1, blocksFound=None)
            with patch.object(sys, 'argv', ['collect.py', '--profile', str(profile)]), \
                    patch.object(collect, 'node_info', return_value=(node, 'ok')) as rpc, \
                    patch.object(collect, 'pool_info', return_value=(pool, 'ok')) as stats, \
                    patch.object(collect, 'stratum_check', return_value='job_received') as probe, \
                    contextlib.redirect_stdout(io.StringIO()) as output, contextlib.redirect_stderr(io.StringIO()):
                collect.main()
            rpc.assert_called_once_with('/opt/bchn/bitcoin-cli', ['-datadir=/srv/bch/node'])
            stats.assert_called_once()
            self.assertEqual(stats.call_args.args[0], Path('/srv/bch/logs/pool/pool.status'))
            probe.assert_called_once_with('127.0.0.1', 3334)
            snapshot = json.loads(output.getvalue())
            self.assertEqual(snapshot['coin'], 'bch')
            self.assertTrue(snapshot['pool']['ready'])
            previous = root/'previous.json'
            previous.write_text(json.dumps(dict(snapshot, coin='btc')))
            with patch.object(sys, 'argv', ['collect.py', '--profile', str(profile), '--previous', str(previous)]), \
                    patch.object(collect, 'node_info') as rpc, contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
                collect.main()
            rpc.assert_not_called()

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

    def test_slow_rpc_retries_and_recovers(self):
        source = dict(chain='main', blocks=25, headers=100,
                      verificationprogress=0.12, initialblockdownload=True)
        result = type('Result', (), {'stdout': json.dumps(source)})()
        with patch.object(collect.subprocess, 'run', side_effect=[collect.subprocess.TimeoutExpired('bitcoin-cli', 35), result]) as run, patch.object(collect.time, 'sleep'):
            node, check = collect.node_info('/usr/local/bin/bitcoin-cli')
        self.assertEqual(check, 'ok')
        self.assertEqual(node['blocks'], 25)
        self.assertEqual(run.call_count, 2)
        self.assertIsNotNone(node['updatedAt'])

    def test_partial_rpc_result_is_not_a_successful_measurement(self):
        result = type('Result', (), {'stdout': '{"chain":"main","initialblockdownload":false}'})()
        with patch.object(collect.subprocess, 'run', return_value=result), patch.object(collect.time, 'sleep'), contextlib.redirect_stderr(io.StringIO()):
            node, check = collect.node_info('/usr/local/bin/bitcoin-cli')
        self.assertEqual(check, 'unavailable')
        self.assertTrue(all(value is None for value in node.values()))

    def test_retained_node_keeps_its_original_date_and_filters_secrets(self):
        now = time.time()
        observed = collect.utc_timestamp(now - 1800)
        node = dict(chain='main', blocks=25, headers=100, verificationProgress=.12,
                    initialBlockDownload=True, updatedAt=observed, rpcpassword='SECRET')
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'status.json'
            for age in (60, 0):
                path.write_text(json.dumps(dict(source='snapshot', updatedAt=collect.utc_timestamp(now-age),
                                                node=node, checks=dict(bitcoin='stale'))))
                retained = collect.previous_node(path, now)
                self.assertEqual(retained['updatedAt'], observed)
                self.assertNotIn('SECRET', json.dumps(retained))
            node['updatedAt'] = collect.utc_timestamp(now + 3600)
            path.write_text(json.dumps(dict(source='snapshot', node=node)))
            self.assertIsNone(collect.previous_node(path, now))

    def run_snapshot(self, previous, rpc_node, rpc_check):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'status.json'
            path.write_text(json.dumps(previous))
            pool = dict(ready=False, hashrateHps=None, workers=None, bestShare=None, blocksFound=None)
            with patch.object(sys, 'argv', ['collect.py', '--previous', str(path)]), patch.object(collect, 'node_info', return_value=(rpc_node, rpc_check)), patch.object(collect, 'pool_status_path', return_value=path), patch.object(collect, 'pool_info', return_value=(pool, 'missing')), patch.object(collect, 'stratum_check', return_value='job_received') as stratum, contextlib.redirect_stdout(io.StringIO()) as stdout, contextlib.redirect_stderr(io.StringIO()):
                collect.main()
            return json.loads(stdout.getvalue()), stratum.call_count

    def test_failed_read_preserves_legacy_success_and_never_marks_pool_ready(self):
        observed = collect.utc_timestamp(time.time() - 300)
        previous = dict(source='snapshot', updatedAt=observed,
                        node=dict(chain='main', blocks=100, headers=100, verificationProgress=1,
                                  initialBlockDownload=False, secret='SECRET'), checks=dict(bitcoin='ok'))
        empty = dict(chain=None, blocks=None, headers=None, verificationProgress=None,
                     initialBlockDownload=None, updatedAt=None)
        data, probes = self.run_snapshot(previous, empty, 'unavailable')
        self.assertEqual(data['checks']['bitcoin'], 'stale')
        self.assertEqual(data['node']['blocks'], 100)
        self.assertEqual(data['node']['updatedAt'], observed)
        self.assertNotIn('SECRET', json.dumps(data))
        self.assertFalse(data['pool']['ready'])
        self.assertEqual(probes, 0)
        # A second failure must not turn the previous failed collection's time
        # into a new successful node measurement time.
        again, _ = self.run_snapshot(data, empty, 'unavailable')
        self.assertEqual(again['node']['updatedAt'], observed)

    def test_upgrade_recovers_good_node_when_latest_legacy_commit_is_empty(self):
        now = time.time()
        observed = collect.utc_timestamp(now - 600)
        good = dict(source='snapshot', updatedAt=observed, checks=dict(bitcoin='ok'),
                    node=dict(chain='main', blocks=20, headers=100, verificationProgress=.15,
                              initialBlockDownload=True, rpcpassword='SECRET'))
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            path = root / 'status.json'
            def git(*args):
                return collect.subprocess.run(['git', '-C', str(root), *args],
                                              capture_output=True, text=True, check=True)
            git('init', '-b', 'stats')
            for snapshot in (good, dict(source='snapshot', updatedAt=collect.utc_timestamp(now),
                                        node={}, checks=dict(bitcoin='unavailable'))):
                path.write_text(json.dumps(snapshot))
                git('add', '--', 'status.json')
                git('-c', 'user.name=Test', '-c', 'user.email=test@example.invalid',
                    '-c', 'commit.gpgsign=false', 'commit', '-m', 'snapshot')
            with contextlib.redirect_stderr(io.StringIO()):
                recovered = collect.previous_node(path, now)
            self.assertEqual(recovered['blocks'], 20)
            self.assertEqual(recovered['updatedAt'], observed)
            self.assertNotIn('SECRET', json.dumps(recovered))
            self.assertEqual(git('status', '--porcelain').stdout, '')

    def test_no_successful_previous_read_does_not_invent_progress(self):
        empty = dict(chain=None, blocks=None, headers=None, verificationProgress=None,
                     initialBlockDownload=None, updatedAt=None)
        previous = dict(source='snapshot', updatedAt=collect.utc_timestamp(), node=empty,
                        checks=dict(bitcoin='unavailable'))
        data, probes = self.run_snapshot(previous, empty, 'unavailable')
        self.assertEqual(data['checks']['bitcoin'], 'unavailable')
        self.assertIsNone(data['node']['verificationProgress'])
        self.assertFalse(data['pool']['ready'])
        self.assertEqual(probes, 0)

    def test_real_recovery_replaces_a_higher_old_progress(self):
        old = dict(chain='main', blocks=90, headers=100, verificationProgress=.9,
                   initialBlockDownload=True, updatedAt=collect.utc_timestamp(time.time()-600))
        fresh = dict(chain='main', blocks=25, headers=100, verificationProgress=.12,
                     initialBlockDownload=True, updatedAt=collect.utc_timestamp())
        previous = dict(source='snapshot', updatedAt=collect.utc_timestamp(), node=old,
                        checks=dict(bitcoin='stale'))
        data, _ = self.run_snapshot(previous, fresh, 'ok')
        self.assertEqual(data['checks']['bitcoin'], 'ok')
        self.assertEqual(data['node'], fresh)

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
