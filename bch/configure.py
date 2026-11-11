#!/usr/bin/env python3
"""Prepare an isolated BCHN + CashStratum solo instance; never start services."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import re
import secrets

HOME = Path('/home/bdl')
ROOT = HOME/'mining/bch'
NODE_BIN = HOME/'.local/opt/bchn-29.2.0/bin'
POOL_BIN = HOME/'.local/opt/cashstratum-1.7.1/bin/cashstratum'
REPO = HOME/'bdl-pool-site'


def prepare(destination, payout, host, public_port, local_port=3334):
    spec = importlib.util.spec_from_file_location('bch_addresses', Path(__file__).resolve().parents[1]/'api/server.py')
    api = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(api)
    payout = api.canonical_address(payout, 'bch')
    if not payout:
        raise ValueError('Adresse CashAddr BCH mainnet invalide.')
    if not re.fullmatch(r'[a-z0-9](?:[a-z0-9.-]{0,251}[a-z0-9])?', host) or '.' not in host:
        raise ValueError('Nom de domaine Stratum BCH invalide.')
    if any(type(port) is not int or not 1024 <= port <= 65535 for port in (public_port, local_port)) or local_port in (3333, 3336, 8081, 8432, 8433):
        raise ValueError('Port BCH invalide ou réservé à un autre service.')
    destination = Path(destination).resolve()
    def mapped(path):
        target = destination/str(path).lstrip('/')
        if any(parent.is_symlink() for parent in (target, *target.parents)):
            raise ValueError('Les chemins de préparation BCH ne doivent pas être des liens symboliques.')
        return target
    settings = HOME/'.config/bdl-pool-bch'
    # Never regenerate credentials or replace a running instance on a rerun.
    if any(mapped(path).exists() for path in (ROOT/'node/bitcoin.conf', ROOT/'cashstratum.conf', settings/'api.json')):
        raise ValueError('Configuration BCH existante : conservée. Installation interrompue.')
    password = secrets.token_urlsafe(48)
    files = {
        ROOT/'node/bitcoin.conf': '\n'.join((
            'server=1', 'daemon=0', 'chain=main', 'disablewallet=1', 'listen=1',
            'bind=127.0.0.1', 'port=8433', 'rpcbind=127.0.0.1', 'rpcallowip=127.0.0.1',
            'rpcport=8432', 'rpcuser=bdl_bch_pool', 'rpcpassword='+password,
            'dbcache=512', 'prune=10000', 'upnp=0', 'natpmp=0', '')),
        ROOT/'cashstratum.conf': json.dumps(dict(
            btcd=[dict(url='127.0.0.1:8432', auth='bdl_bch_pool', **{'pass': password}, notify=False)],
            bchaddress=payout, btcaddress=payout, poolfee=0, btcsig='BDL Pool BCH',
            serverurl=['127.0.0.1:'+str(local_port)], logdir=str(ROOT/'logs'),
            mindiff=1, startdiff=1, highdiff=1, blockpoll=100, update_interval=30), indent=2)+'\n',
        settings/'api.json': json.dumps(dict(
            snapshot=str(HOME/'.local/state/bdl-pool-monitor/repository/status-bch.json'),
            logs=str(ROOT/'logs'), sv2Public=str(settings/'sv2-disabled.json'),
            onlineSnapshot='/var/lib/bdl-pool-presence/bch-online.json',
            ckpoolSocket='/run/bdl-pool-bch/stratifier', host=host, v1Port=public_port), indent=2)+'\n',
        settings/'monitor.json': json.dumps(dict(
            coin='bch', bitcoinCli=str(NODE_BIN/'bitcoin-cli'), cliArgs=['-datadir='+str(ROOT/'node')],
            config=str(ROOT/'cashstratum.conf'), poolStatus=str(ROOT/'logs/pool/pool.status'),
            stratumHost='127.0.0.1', stratumPort=local_port), indent=2)+'\n',
    }
    common = '\n'.join(('User=bdl', 'Group=bdl', 'UMask=0077', 'NoNewPrivileges=true',
                        'PrivateTmp=true', 'ProtectSystem=strict', 'Restart=on-failure', 'RestartSec=15'))
    files[settings/'units/bdl-bchn.service'] = f'''[Unit]
Description=BDL Pool Bitcoin Cash Node (BCHN)
Wants=network-online.target
After=network-online.target

[Service]
Type=simple
{common}
ExecStart={NODE_BIN}/bitcoind -datadir={ROOT}/node
ReadWritePaths={ROOT}/node
TimeoutStopSec=300

[Install]
WantedBy=multi-user.target
'''
    files[settings/'units/bdl-pool-bch.service'] = f'''[Unit]
Description=BDL Pool Bitcoin Cash solo Stratum V1
Requires=bdl-bchn.service
After=bdl-bchn.service

[Service]
Type=simple
{common}
WorkingDirectory={ROOT}
RuntimeDirectory=bdl-pool-bch
RuntimeDirectoryMode=0700
ExecStartPre=/usr/bin/python3 {REPO}/bch/check-node.py
ExecStart={POOL_BIN} -B -c {ROOT}/cashstratum.conf -n bdl-pool-bch -s /run/bdl-pool-bch
ReadWritePaths={ROOT}/logs /run/bdl-pool-bch
TimeoutStopSec=60

[Install]
WantedBy=multi-user.target
'''
    for path, text in files.items():
        target = mapped(path)
        target.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
        descriptor = os.open(target, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        with os.fdopen(descriptor, 'w') as stream:
            stream.write(text)
    mapped(ROOT/'logs/pool').mkdir(parents=True, exist_ok=True, mode=0o700)
    return files.keys()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--payout', required=True, help='Public BCH mainnet fallback receiving address')
    parser.add_argument('--public-host')
    parser.add_argument('--public-port', type=int)
    parser.add_argument('--from-relay', action='store_true', help='Use the independently tested local BCH OpenPort profile')
    parser.add_argument('--local-port', default=3334, type=int)
    parser.add_argument('--destdir', type=Path, default=Path('/'), help='Staging root; no service changes')
    args = parser.parse_args()
    try:
        if args.from_relay:
            if args.public_host is not None or args.public_port is not None or args.local_port != 3334:
                parser.error('--from-relay utilise le profil BCH et son port local 3334 ; omettre les options de port/domaine.')
            spec = importlib.util.spec_from_file_location('bch_relay_setup', Path(__file__).with_name('relay.py'))
            relay = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(relay)
            profile = Path.home()/'.local/state/bdl-pool-openport/bch'
            try:
                public = relay.route_for_pool(profile)
            except relay.client.TrialError as error:
                raise ValueError(str(error)) from None
            args.public_host, args.public_port = public['host'], public['port']
        elif not args.public_host or args.public_port is None:
            parser.error('Utiliser --from-relay, ou fournir --public-host et --public-port.')
        prepare(args.destdir, args.payout, args.public_host, args.public_port, args.local_port)
    except (ValueError, OSError) as error:
        parser.exit(1, str(error)+'\n')
    print('Configuration BCH préparée. Aucun service démarré. Identifiants RPC conservés uniquement sur le serveur.')


if __name__ == '__main__':
    main()
