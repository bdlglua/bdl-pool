#!/usr/bin/env python3
"""Only let the BCH pool start with BCHN, BCH mainnet and usable templates."""
import json
from pathlib import Path
import subprocess
import sys

HOME = Path('/home/bdl')
cli = [str(HOME/'.local/opt/bchn-29.2.0/bin/bitcoin-cli'), '-datadir='+str(HOME/'mining/bch/node'), '-rpcclienttimeout=15']
try:
    def rpc(method):
        return json.loads(subprocess.run([*cli, method], check=True, capture_output=True, text=True, timeout=20).stdout)
    network = rpc('getnetworkinfo')
    chain = rpc('getblockchaininfo')
    if 'Bitcoin Cash Node' not in network.get('subversion', '') or chain.get('chain') != 'main' or chain.get('initialblockdownload') is not False:
        raise ValueError('BCH node not ready')
    template = rpc('getblocktemplate')
    if type(template.get('coinbasevalue')) is not int or template['coinbasevalue'] <= 0 or not template.get('previousblockhash'):
        raise ValueError('No usable BCH template')
except (OSError, ValueError, subprocess.SubprocessError):
    sys.exit('BCHN indisponible, en synchronisation ou sans travail valide. Nouvelle tentative automatique.')
