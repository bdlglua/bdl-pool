import contextlib
import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch


def module(name, filename):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(filename))
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


domain = module('custom_domain_test', 'custom-domain.py')
dns = module('custom_domain_dns_test', 'duckdns-openport.py')
PORTS = dict(v1=39991, v2=36809, api=36018)
PREVIOUS = dict(domain=domain.OLD_DOMAIN, managedTLS=True, v1Port=39991, v2Port=36809, apiPublicPort=36018)


class DelegationTests(unittest.TestCase):
    def test_challenge_delegation_must_match_on_every_authoritative_server(self):
        fake = SimpleNamespace(dig=Mock(side_effect=[[domain.TARGET+'.'], ['other.example.']]))
        with patch.object(domain, 'nameservers', return_value=['1.1.1.1', '8.8.8.8']):
            with self.assertRaises(domain.DomainError):
                domain.check_delegation(fake)
        self.assertEqual(fake.dig.call_count, 2)

    def test_alias_authoritative_answer_without_recursion_is_supported(self):
        def dig(host, kind, server=None):
            if kind == 'CNAME':
                return [domain.TARGET+'.'] if host.startswith('_acme-challenge') else ['openport.pw.']
            if kind == 'AAAA':
                return ['openport.pw.'] if host == domain.API_DOMAIN else []
            if host == domain.API_DOMAIN and server:
                return ['openport.pw.']
            return ['1.1.1.1']
        fake = SimpleNamespace(dig=dig, relay_ipv4=Mock(return_value='1.1.1.1'))
        with patch.object(domain, 'nameservers', return_value=['8.8.8.8']):
            domain.check_addresses(fake, {})

    def test_foreign_address_or_ipv6_does_not_pass_preflight(self):
        for bad_a, bad_aaaa in ((['9.9.9.9'], []), (['1.1.1.1'], ['2606:4700:4700::1111'])):
            fake = SimpleNamespace(relay_ipv4=Mock(return_value='1.1.1.1'),
                                   dig=Mock(side_effect=lambda _, kind, server=None:
                                            bad_a if kind == 'A' else ['openport.pw.'] if kind == 'CNAME' else bad_aaaa))
            with patch.object(domain, 'nameservers', return_value=['8.8.8.8']):
                with self.assertRaises(domain.DomainError):
                    domain.check_addresses(fake, {})

    def test_website_apex_is_not_required_to_point_to_the_mining_relay(self):
        def dig(host, kind, server=None):
            if host == domain.SITE_DOMAIN:
                raise AssertionError('Website DNS must not be treated as a mining endpoint')
            if kind == 'CNAME':
                return [domain.TARGET+'.'] if host.startswith('_acme-challenge') else ['openport.pw.']
            return ['1.1.1.1'] if kind == 'A' else []
        fake = SimpleNamespace(dig=dig, relay_ipv4=Mock(return_value='1.1.1.1'))
        with patch.object(domain, 'nameservers', return_value=['8.8.8.8']):
            domain.check_addresses(fake, {})

    def test_auth_and_cleanup_only_modify_delegate_txt_and_never_print_credentials(self):
        settings = dict(domain=domain.OLD_DOMAIN, token='PRIVATE_TEST_TOKEN')
        fake = SimpleNamespace(settings=Mock(return_value=settings), update=Mock(), wait_txt=Mock())
        env = dict(CERTBOT_DOMAIN=domain.API_DOMAIN, CERTBOT_VALIDATION='A'*43)
        with patch.dict(os.environ, env), patch.object(domain, 'check_delegation'), contextlib.redirect_stdout(io.StringIO()) as output:
            domain.challenge(fake)
            domain.challenge(fake, cleanup=True)
        self.assertEqual([call.kwargs for call in fake.update.call_args_list],
                         [dict(txt='A'*43), dict(txt='A'*43, clear='true')])
        self.assertNotIn(settings['token'], output.getvalue())
        fake.update.reset_mock()
        with patch.dict(os.environ, dict(env, CERTBOT_DOMAIN='unexpected.example')):
            with self.assertRaises(domain.DomainError):
                domain.challenge(fake)
        fake.update.assert_not_called()

    def test_bad_certificate_cannot_replace_the_current_certificate(self):
        fake = SimpleNamespace(deploy_certificate=Mock())
        with patch.dict(os.environ, {}, clear=True), patch.object(domain.subprocess, 'run', return_value=subprocess.CompletedProcess([], 1)):
            with self.assertRaises(domain.DomainError):
                domain.deploy(fake)
        fake.deploy_certificate.assert_not_called()


class ActivationTests(unittest.TestCase):
    def fixture(self, health):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        main = root/'main.caddy'
        main.write_text(domain.site(domain.OLD_DOMAIN, Path('/etc/caddy/bdl-pool-tls')))
        config = root/'api.json'
        config.write_text(json.dumps(dict(host=domain.OLD_DOMAIN, snapshot='unchanged', apiPublicPort=36018)))
        public = root/'public.json'
        public.write_text(json.dumps(PREVIOUS))
        legacy = root/'legacy.caddy'
        originals = {path: path.read_bytes() if path.exists() else None for path in (main, config, public, legacy)}
        fake = SimpleNamespace(private_write=dns.private_write, DNSError=dns.DNSError, wait_health=Mock(side_effect=health))
        return root, main, config, public, legacy, originals, fake

    def activate(self, fixture, previous=PREVIOUS):
        root, main, config, public, legacy, originals, fake = fixture
        with patch.object(domain, 'ROOT', root), patch.object(domain, 'FRAGMENT', main), patch.object(domain, 'LEGACY', legacy), \
             patch.object(domain, 'API_CONFIG', config), patch.object(domain.os, 'chown'), patch.object(domain, 'run') as run, \
             patch.object(domain.subprocess, 'run', return_value=subprocess.CompletedProcess([], 0)), contextlib.redirect_stdout(io.StringIO()):
            domain.activate(fake, PORTS, previous)
        return run

    def test_activation_keeps_old_certificate_and_api_and_persists_split_hosts(self):
        fixture = self.fixture([True, True, True])
        run = self.activate(fixture)
        root, main, config, public, legacy, originals, fake = fixture
        self.assertIn('api.bdl-pool.com {', main.read_text())
        self.assertIn('/etc/caddy/bdl-pool-domain-tls/current/', main.read_text())
        self.assertEqual(legacy.read_bytes(), originals[main])
        settings = json.loads(config.read_text())
        self.assertEqual(settings['host'], domain.DOMAIN)
        self.assertEqual(settings['apiHost'], domain.API_DOMAIN)
        self.assertEqual(settings['snapshot'], 'unchanged')
        self.assertEqual(json.loads(public.read_text())['apiDomain'], domain.API_DOMAIN)
        self.assertEqual(public.stat().st_mode & 0o777, 0o644)
        self.assertEqual(config.stat().st_mode & 0o777, 0o600)
        commands = [call.args[0] for call in run.call_args_list]
        self.assertEqual([c for c in commands if 'restart' in c], [['systemctl', 'restart', 'bdl-pool-api.service']])

    def test_tls_or_delayed_api_failure_restores_every_config_and_removes_new_alias(self):
        for replies in ([False], [True, False]):
            with self.subTest(replies=replies):
                fixture = self.fixture(replies)
                with self.assertRaises(domain.DomainError):
                    self.activate(fixture)
                for path, content in fixture[5].items():
                    self.assertEqual(path.read_bytes() if path.exists() else None, content)

    def test_unrelated_caddy_configuration_is_preserved(self):
        fixture = self.fixture([True]*3)
        fixture[4].write_text('other.example { respond "unrelated" }\n')
        with self.assertRaises(domain.DomainError):
            self.activate(fixture)
        self.assertIn('unrelated', fixture[4].read_text())
        self.assertEqual(fixture[2].read_bytes(), fixture[5][fixture[2]])

    def test_move_from_apex_to_stratum_keeps_api_certificate_and_supports_reinstall(self):
        fixture = self.fixture([True, True, True])
        previous = dict(PREVIOUS, domain=domain.SITE_DOMAIN, apiDomain=domain.API_DOMAIN)
        self.activate(fixture, previous)
        public = json.loads(fixture[3].read_text())
        self.assertEqual(public['domain'], 'stratum.bdl-pool.com')
        self.assertEqual(public['previousDomain'], 'bdl-pool.com')
        self.assertEqual(public['siteDomain'], 'bdl-pool.com')
        self.assertEqual(public['apiDomain'], 'api.bdl-pool.com')
        settings = json.loads(fixture[2].read_text())
        self.assertEqual(settings['host'], 'stratum.bdl-pool.com')
        self.assertEqual(settings['apiPublicPort'], 36018)

    def test_failed_renewal_test_never_activates_the_new_configuration(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        (root/'public.json').write_text(json.dumps(PREVIOUS))
        fake = SimpleNamespace(settings=Mock(return_value=dict(apiPort=36018)), private_write=Mock())
        def run(command, label):
            if '--dry-run' in command:
                raise domain.DomainError('renewal failed')
        with patch.object(domain, 'ROOT', root), patch.object(domain, 'routes', return_value=PORTS), \
             patch.object(domain, 'check_addresses'), patch.object(domain.shutil, 'which', return_value='/usr/bin/available'), \
             patch.object(domain.os, 'chown'), patch.object(domain, 'run', side_effect=run), patch.object(domain, 'activate') as activate, \
             contextlib.redirect_stdout(io.StringIO()):
            with self.assertRaises(domain.DomainError):
                domain.install(fake)
        activate.assert_not_called()


if __name__ == '__main__':
    unittest.main()
