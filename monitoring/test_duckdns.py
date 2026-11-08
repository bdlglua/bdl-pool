import contextlib
import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch
from urllib.error import HTTPError


def module(name, filename):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(filename))
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


dns = module('dns_test', 'duckdns-openport.py')
migration = module('migration_test', 'migrate-openport.py')
SETTINGS = dict(domain='bdl-pool.duckdns.org', token='00000000-1111-2222-3333-444444444444', relayHost='openport.pw', apiPort=36018)


class DNSTests(unittest.TestCase):
    def test_provider_errors_do_not_expose_token_urls(self):
        class Broken:
            def open(self, request, timeout):
                raise HTTPError(request.full_url, 403, 'Forbidden', {}, None)
        with patch.object(dns, 'build_opener', return_value=Broken()):
            with self.assertRaises(dns.DNSError) as caught:
                dns.update(SETTINGS, ip='198.51.100.1')
        self.assertNotIn(SETTINGS['token'], str(caught.exception))
        self.assertNotIn('token=', str(caught.exception))

    def test_txt_hooks_never_clear_address_records(self):
        env = dict(CERTBOT_DOMAIN=SETTINGS['domain'], CERTBOT_VALIDATION='A'*43)
        with patch.dict(os.environ, env), patch.object(dns, 'update') as update, patch.object(dns, 'wait_txt'):
            dns.challenge(SETTINGS)
            dns.challenge(SETTINGS, cleanup=True)
        self.assertEqual(update.call_args_list[0].kwargs, dict(txt='A'*43))
        self.assertEqual(update.call_args_list[1].kwargs, dict(txt='A'*43, clear='true'))
        with patch.dict(os.environ, dict(env, CERTBOT_DOMAIN='other.example')), patch.object(dns, 'update') as update:
            with self.assertRaises(dns.DNSError):
                dns.challenge(SETTINGS)
            update.assert_not_called()

    def test_authoritative_txt_propagation_checks_every_nameserver(self):
        replies = [['"wrong"'], ['"'+'A'*43+'"'], ['"'+'A'*20+'" "'+'A'*23+'"'], ['"'+'A'*43+'"']]
        with patch.object(dns, 'authoritative_servers', return_value=['192.0.2.1', '192.0.2.2']), patch.object(dns, 'dig', side_effect=replies) as query, patch.object(dns.time, 'sleep') as sleep:
            dns.wait_txt(SETTINGS, 'A'*43)
        self.assertEqual(query.call_count, 4)
        self.assertEqual(sleep.call_count, 1)

    def test_first_sync_clears_old_aaaa_then_sets_explicit_relay_ip(self):
        with patch.object(dns, 'relay_ipv4', return_value='198.51.100.1'), patch.object(dns, 'update', side_effect=[['OK'], ['OK', '198.51.100.1', '', 'UPDATED']]) as update:
            dns.sync(SETTINGS, first=True)
        self.assertEqual(update.call_args_list[0].kwargs, dict(clear='true'))
        self.assertEqual(update.call_args_list[1].kwargs, dict(ip='198.51.100.1', verbose='true'))

    def test_later_sync_never_uses_source_ip_detection_and_removes_competing_aaaa(self):
        replies = [['OK', '198.51.100.1', '2001:db8::1', 'UPDATED'], ['OK'], ['OK', '198.51.100.1', '', 'UPDATED']]
        with patch.object(dns, 'relay_ipv4', return_value='198.51.100.1'), patch.object(dns, 'update', side_effect=replies) as update:
            dns.sync(SETTINGS)
        self.assertEqual([call.kwargs for call in update.call_args_list], [dict(ip='198.51.100.1', verbose='true'), dict(clear='true'), dict(ip='198.51.100.1', verbose='true')])

    def test_failed_tls_proof_never_changes_dns(self):
        with patch.object(dns.socket, 'getaddrinfo', return_value=[(None, None, None, None, ('1.1.1.1', 36018))]), patch.object(dns, 'health', return_value=False), patch.object(dns, 'update') as update:
            with self.assertRaises(dns.DNSError):
                dns.sync(SETTINGS, first=True)
        update.assert_not_called()

    def test_health_wait_accepts_delayed_api_startup(self):
        with patch.object(dns, 'health', side_effect=[False, False, True]) as health, patch.object(dns.time, 'sleep') as sleep:
            self.assertTrue(dns.wait_health(SETTINGS))
        self.assertEqual(health.call_count, 3)
        self.assertEqual(sleep.call_count, 2)


class MigrationTests(unittest.TestCase):
    def execute(self, failure=None):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        home = root/'home/bdl'
        scripts = home/'bdl-pool-site/monitoring'; scripts.mkdir(parents=True)
        (scripts/'duckdns-openport.py').write_text('# validated separately\n')
        private = root/'private'; private.mkdir()
        fragment = root/'api.caddy'; fragment.write_text('bdl-pool.duckdns.org {\n respond "old"\n}\n')
        api = home/'.config/bdl-pool-api/config.json'; api.parent.mkdir(parents=True)
        api.write_text(json.dumps(dict(host=SETTINGS['domain'], v1Port=3333, v2Port=3336)))
        commands = []
        def run(command, label):
            commands.append(command)
            if failure == 'renewal' and '--dry-run' in command:
                raise migration.MigrationError('renewal failed')
        health = iter([True, False]) if failure == 'caddy' else iter([True, True, True])
        with contextlib.ExitStack() as stack:
            for name, value in dict(HOME_BDL=home, ROOT=private, LOG=root/'migration.log', FRAGMENT=fragment,
                                    API_CONFIG=api, HOOK=root/'libexec/hook.py', __file__=str(scripts/'migrate-openport.py')).items():
                stack.enter_context(patch.object(migration, name, value))
            stack.enter_context(patch.object(migration.os, 'geteuid', return_value=0))
            stack.enter_context(patch.object(migration.os, 'chown'))
            stack.enter_context(patch.object(migration, 'routes', return_value=dict(v1=39991, v2=36809, api=36018)))
            stack.enter_context(patch.object(migration, 'reject_old_updaters'))
            stack.enter_context(patch.object(migration.getpass, 'getpass', return_value=SETTINGS['token']))
            stack.enter_context(patch.object(migration.shutil, 'which', return_value='/bin/available'))
            stack.enter_context(patch.object(migration, 'run', side_effect=run))
            stack.enter_context(patch.object(migration, 'install_dns_timer'))
            stack.enter_context(patch.object(migration.subprocess, 'run', return_value=subprocess.CompletedProcess([], 0)))
            stack.enter_context(patch.object(migration.dns, 'wait_health', side_effect=lambda *_, **kwargs: next(health)))
            sync = stack.enter_context(patch.object(migration.dns, 'sync'))
            stack.enter_context(patch.object(migration.dns, 'dig', return_value=[]))
            stack.enter_context(patch.object(sys, 'argv', ['migration']))
            output = stack.enter_context(contextlib.redirect_stdout(io.StringIO()))
            result = migration.main()
        return root, private, fragment, api, commands, sync, result, output.getvalue()

    def test_success_preserves_private_token_and_persists_tls_before_dns(self):
        root, private, fragment, api, commands, sync, result, output = self.execute()
        self.assertEqual(result, 0)
        self.assertEqual((private/'duckdns.json').stat().st_mode & 0o777, 0o600)
        self.assertNotIn(SETTINGS['token'], output)
        self.assertTrue(any('--dry-run' in command for command in commands))
        self.assertTrue((private/'dns-migrated').exists())
        public = json.loads((private/'public.json').read_text())
        self.assertTrue(public['managedTLS'])
        self.assertNotIn('token', public)
        self.assertEqual(json.loads(api.read_text())['apiPublicPort'], 36018)
        self.assertIn('tls /etc/caddy/bdl-pool-tls/current/', fragment.read_text())
        sync.assert_called_once()

    def test_failed_renewal_does_not_touch_caddy_or_dns(self):
        root, private, fragment, api, commands, sync, result, output = self.execute('renewal')
        self.assertEqual(result, 1)
        self.assertTrue(any('--dry-run' in command for command in commands))
        self.assertIn('respond "old"', fragment.read_text())
        self.assertEqual(json.loads(api.read_text())['v1Port'], 3333)
        self.assertFalse((private/'public.json').exists())
        sync.assert_not_called()

    def test_failed_reloaded_tls_restores_caddy_before_dns(self):
        root, private, fragment, api, commands, sync, result, output = self.execute('caddy')
        self.assertEqual(result, 1)
        self.assertIn('respond "old"', fragment.read_text())
        sync.assert_not_called()


if __name__ == '__main__':
    unittest.main()
