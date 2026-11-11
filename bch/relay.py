#!/usr/bin/env python3
"""Install only the BCH OpenPort relay in the pool VM, preserving BTC profiles."""
import argparse
from datetime import datetime, timezone
import fcntl
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import time
from urllib.request import Request, build_opener

SITE = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('bch_openport_client', SITE/'monitoring/try-openport.py')
client = importlib.util.module_from_spec(spec)
spec.loader.exec_module(client)
UNIT = 'bdl-pool-openport@bch.service'
LOCAL_PORT = 3334


def profile_directory(root):
    profile = root/'bch'
    if any(path.is_symlink() for path in (root, *root.parents, profile)):
        raise client.TrialError('Le profil OpenPort BCH ne doit pas être un lien symbolique.')
    profile.mkdir(mode=0o700, parents=True, exist_ok=True)
    root.chmod(0o700)
    profile.chmod(0o700)
    return profile


def other_profiles(root):
    identities = []
    for name in ('v1', 'v2', 'api'):
        profile = root/name
        identity_file, route_file = profile/'identity.json', profile/'public.json'
        if not identity_file.exists():
            if route_file.exists():
                raise client.TrialError('Profil '+name.upper()+' incomplet ; identité existante à vérifier.')
            continue
        if profile.is_symlink() or identity_file.is_symlink():
            raise client.TrialError('Profil '+name.upper()+' inattendu ; aucun relais existant modifié.')
        identity = json.loads(identity_file.read_text())
        if not isinstance(identity, dict):
            raise client.TrialError('Profil '+name.upper()+' illisible ; aucun relais existant modifié.')
        client.validate_identity(identity)
        identities.append(identity)
    return identities


def distinct_identity(identity, others):
    if any(identity['client_id'] == other['client_id'] or identity['remote_port'] == other['remote_port'] for other in others):
        raise client.TrialError('Le profil BCH partage une identité ou un port avec un relais existant. Aucune connexion BCH créée.')


def saved_route(profile):
    if any(path.is_symlink() for path in (profile, *profile.parents)):
        raise client.TrialError('Le profil BCH ne doit pas être un lien symbolique.')
    for name in ('identity.json', 'public.json'):
        if (profile/name).is_symlink():
            raise client.TrialError('Fichier du profil BCH inattendu.')
    identity = json.loads((profile/'identity.json').read_text())
    if not isinstance(identity, dict):
        raise client.TrialError('Identité BCH illisible ; aucun nouveau port demandé.')
    client.validate_identity(identity)
    distinct_identity(identity, other_profiles(profile.parent))
    route = json.loads((profile/'public.json').read_text())
    if not isinstance(route, dict) or route.get('port') != identity['remote_port']:
        raise client.TrialError('Adresse BCH incohérente ; identité conservée.')
    return dict(host=client.relay_host(route.get('host')), port=identity['remote_port'])


def route_for_pool(profile):
    public = saved_route(profile)
    check = json.loads((profile/'transport-check.json').read_text())
    if (not isinstance(check, dict) or check.get('host') != public['host'] or check.get('port') != public['port']
            or check.get('targetPort') != LOCAL_PORT or check.get('probe') != 'temporary_loopback_http'):
        raise client.TrialError('Relais BCH non vérifié ; lancer python3 bch/relay.py install dans la VM.')
    return public


def probe_transport(binary, profile, identity):
    """Prove the public route reaches a temporary loopback HTTP responder."""
    server, completed, path = client.test_server()
    process = None
    log = None
    try:
        public = client.connect(identity, server.server_port, profile)
        descriptor = os.open(profile/'relay-test.log', os.O_WRONLY | os.O_CREAT | os.O_TRUNC | os.O_NOFOLLOW, 0o600)
        log = os.fdopen(descriptor, 'w')
        os.fchmod(log.fileno(), 0o600)
        process = subprocess.Popen([str(binary), str(profile/'client.toml')], stdout=log, stderr=subprocess.STDOUT)
        request = Request('http://'+client.endpoint(public['host'], public['port'])+path,
                          headers={'User-Agent': 'BDL-Pool-BCH-Relay-Check', 'Cache-Control': 'no-cache'})
        deadline = time.monotonic()+25
        while time.monotonic() < deadline:
            if process.poll() is not None:
                raise client.TrialError('Le client du relais BCH s’est arrêté. Journal privé : relay-test.log.')
            try:
                with build_opener(client.NoRedirect()).open(request, timeout=2) as response:
                    correct = response.status == 200 and response.read(64) == b'BDL-OPENPORT-OK\n'
                if correct and completed.wait(.5):
                    return public
            except OSError:
                pass
            time.sleep(.5)
        raise client.TrialError('Le trajet public du relais BCH n’est pas confirmé. Identité conservée ; installation interrompue.')
    finally:
        if process and process.poll() is None:
            process.terminate()
            try:
                process.wait(timeout=3)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
        if log:
            log.close()
        try:
            client.api('release', {key: identity[key] for key in ('client_id', 'token')})
        except client.TrialError:
            pass
        (profile/'client.toml').unlink(missing_ok=True)
        server.shutdown()
        server.server_close()


def reserve(root):
    profile = profile_directory(root)
    if any((profile/name).is_symlink() for name in ('identity.json', 'public.json', 'transport-check.json', 'trial.lock')):
        raise client.TrialError('Fichier BCH inattendu ; aucun nouveau port demandé.')
    others = other_profiles(root)
    if (profile/'public.json').exists() and not (profile/'identity.json').exists():
        raise client.TrialError('Identité BCH absente pour un port déjà réservé ; aucun nouveau port demandé.')
    with (profile/'trial.lock').open('a') as lock:
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            raise client.TrialError('Le profil BCH est déjà utilisé par un essai ou un service.') from None
        binary = client.verified_binary()
        public = None
        # Re-register the same saved identity for the second check, proving port reuse.
        for attempt in range(2):
            identity = client.credentials(profile)
            distinct_identity(identity, others)
            confirmed = probe_transport(binary, profile, identity)
            if public is not None and confirmed != public:
                raise client.TrialError('Le port BCH a changé entre les deux contrôles.')
            public = confirmed
            print('Contrôle public BCH '+str(attempt+1)+'/2 réussi.', flush=True)
        client.private_json(profile/'transport-check.json', dict(public,
            checkedAt=datetime.now(timezone.utc).isoformat(timespec='seconds'),
            probe='temporary_loopback_http', targetPort=LOCAL_PORT, miningVerified=False))
        return public


def unit_text():
    # An instance-specific unit leaves the installed V1/V2/API template untouched.
    return '''[Unit]
Description=BDL Pool Bitcoin Cash OpenPort relay
Wants=network-online.target
After=network-online.target
StartLimitIntervalSec=0

[Service]
Type=simple
User=bdl
Group=bdl
Environment=PYTHONDONTWRITEBYTECODE=1
WorkingDirectory=/home/bdl/bdl-pool-site/monitoring
ExecStart=/usr/bin/python3 /home/bdl/bdl-pool-site/monitoring/openport-service.py bch
Restart=on-failure
RestartSec=20
TimeoutStopSec=25
UMask=0077
NoNewPrivileges=true
PrivateTmp=true
PrivateDevices=true
ProtectSystem=strict
ProtectHome=tmpfs
BindReadOnlyPaths=/home/bdl/bdl-pool-site/monitoring
BindReadOnlyPaths=/home/bdl/.local/share/bdl-pool-openport
BindPaths=/home/bdl/.local/state/bdl-pool-openport/bch
RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX
MemoryMax=128M
TasksMax=256
LimitNOFILE=1024

[Install]
WantedBy=multi-user.target
'''


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=('install', 'status'))
    args = parser.parse_args()
    home = Path.home()
    if home != Path('/home/bdl') or os.getuid() == 0 or SITE != home/'bdl-pool-site':
        parser.error('Lancer dans la VM avec le compte bdl, dépôt /home/bdl/bdl-pool-site, sans sudo.')
    os.umask(0o077)
    root = home/'.local/state/bdl-pool-openport'
    try:
        if args.action == 'status':
            public = saved_route(root/'bch')
        else:
            profile_directory(root)
            with (root/'install.lock').open('a') as lock:
                try:
                    fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
                except BlockingIOError:
                    raise client.TrialError('Une installation OpenPort est déjà en cours.') from None
                active = subprocess.run(['systemctl', 'is-active', '--quiet', UNIT]).returncode == 0
                if active:
                    public = saved_route(root/'bch')
                    print('Relais BCH déjà actif ; service et identité conservés.')
                else:
                    public = reserve(root)
                    staged = root/'bch'/UNIT
                    staged.write_text(unit_text())
                    staged.chmod(0o600)
                    subprocess.run(['sudo', 'install', '-m', '644', str(staged), '/etc/systemd/system/'+UNIT], check=True)
                    subprocess.run(['sudo', 'systemctl', 'daemon-reload'], check=True)
                    subprocess.run(['sudo', 'systemctl', 'enable', '--now', UNIT], check=True)
                    subprocess.run(['systemctl', 'is-active', '--quiet', UNIT], check=True)
        print('Relais BCH : '+client.endpoint(public['host'], public['port']))
        print('Destination dans la VM : 127.0.0.1:'+str(LOCAL_PORT))
        print('Le minage attend l’installation et la synchronisation du pool BCH.')
        return 0
    except client.TrialError as error:
        print(str(error), flush=True)
    except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError):
        print('Installation BCH interrompue. Les identités restent dans leur dossier privé ; aucun relais BTC modifié.', flush=True)
    return 1


if __name__ == '__main__':
    raise SystemExit(main())
