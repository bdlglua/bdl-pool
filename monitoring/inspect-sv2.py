#!/usr/bin/env python3
"""Read-only CKPool SV2 capability check. Never prints the pool configuration."""
import argparse
from pathlib import Path
import re
import subprocess
import sys


def capability(binary, config):
    try:
        help_result = subprocess.run([binary, '--help'], capture_output=True, text=True,
                                     timeout=5, check=False)
        help_text = help_result.stdout + help_result.stderr
        # An unsupported CKPool option can fall through to normal startup.
        # Only run this diagnostic when the installed binary advertises it.
        if not re.search(r'--testconfig(?:\s|$)', help_text):
            return None, 'Le binaire ne propose pas le test de configuration ; version à vérifier.'
        result = subprocess.run([binary, '--testconfig', '-B', '-c', config],
                                capture_output=True, text=True, timeout=10, check=False)
        match = re.search(r'^sv2_compiled\s*=\s*([01])\s*$', result.stdout, re.M)
        if result.returncode != 0:
            return None, f'Le test de configuration a échoué (code {result.returncode}).'
        if match:
            return match[1] == '1', 'Test de configuration du binaire installé terminé.'
        return None, 'Le test ne fournit pas d’indicateur SV2 ; version à vérifier.'
    except FileNotFoundError:
        return None, 'Binaire CKPool introuvable.'
    except (OSError, subprocess.SubprocessError):
        return None, 'Le diagnostic du binaire n’a pas pu aboutir.'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', default='/usr/local/bin/ckpool')
    parser.add_argument('--config', default='/home/bdl/mining/ckpool/local.conf')
    parser.add_argument('--source', default='/home/bdl/mining/ckpool')
    parser.add_argument('--require-sv2', action='store_true', help='Return an error unless installed SV2 support is confirmed')
    args = parser.parse_args()
    enabled, message = capability(args.binary, args.config)
    print('SV2 compilé dans le binaire installé :', 'oui' if enabled is True else 'non' if enabled is False else 'indéterminé')
    print(message)
    source = Path(args.source)
    for name in ('config.h', 'src/config.h'):
        path = source / name
        if not path.is_file():
            continue
        text = path.read_text(errors='replace')
        print('Compilation locale HAVE_SV2 :', 'activé' if re.search(r'^\s*#\s*define\s+HAVE_SV2\s+1\b', text, re.M) else 'non activé')
        break
    try:
        revision = subprocess.run(['git', '-C', str(source), 'rev-parse', '--short=12', 'HEAD'],
                                  capture_output=True, text=True, timeout=5, check=True).stdout.strip()
        if re.fullmatch(r'[0-9a-f]{7,64}', revision):
            print('Révision des sources locales :', revision)
    except (OSError, subprocess.SubprocessError):
        pass
    return 1 if args.require_sv2 and enabled is not True else 0


if __name__ == '__main__':
    sys.exit(main())
