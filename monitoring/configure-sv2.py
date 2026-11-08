#!/usr/bin/env python3
"""Configure native CKPool SV2, keeping private keys and RPC settings local."""
from datetime import datetime, timezone
import fcntl
import getpass
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import shlex
import stat
import subprocess
import sys
import tempfile

REVISION = 'bef6dd2efa2ed803d77fb49fbc064666685cb742'
SCRIPT_DIR = Path(__file__).resolve().parent


def no_duplicate_keys(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError('Clé JSON présente plusieurs fois : configuration non modifiée.')
        result[key] = value
    return result


def public_key_valid(key):
    if not isinstance(key, str) or not re.fullmatch(r'[1-9A-HJ-NP-Za-km-z]{40,70}', key):
        return False
    alphabet = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz'
    value = 0
    for character in key:
        value = value * 58 + alphabet.index(character)
    raw = b'\x00' * (len(key) - len(key.lstrip('1'))) + value.to_bytes((value.bit_length()+7)//8, 'big')
    return (len(raw) == 38 and raw[:2] == b'\x01\x00'
            and hashlib.sha256(hashlib.sha256(raw[:-4]).digest()).digest()[:4] == raw[-4:])


def protect_directory(path):
    if path.is_symlink():
        raise ValueError('Dossier de clés symbolique refusé.')
    path.mkdir(parents=True, mode=0o700, exist_ok=True)
    if path.stat().st_uid != os.geteuid() or not path.is_dir():
        raise ValueError('Le dossier de clés doit appartenir au compte du pool.')
    path.chmod(0o700)


def inspect_module():
    spec = importlib.util.spec_from_file_location('bdl_inspect_sv2', SCRIPT_DIR / 'inspect-sv2.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def verify_configuration(binary, candidate):
    result = subprocess.run([str(binary), '--testconfig', '-B', '-c', str(candidate)],
                            capture_output=True, text=True, timeout=15, check=False)
    return (result.returncode == 0
            and re.search(r'^sv2_compiled\s*=\s*1\s*$', result.stdout, re.M)
            and re.search(r'^sv2urls\s*=\s*1\s*$', result.stdout, re.M)
            and re.search(r'^serverurl\[\d+\]\s*=\s*0\.0\.0\.0:3336\s+type=sv2\b', result.stdout, re.M))


def compile_keytool(home, binary, state):
    manifest = json.loads((state / 'sv2-build.json').read_text())
    if manifest.get('revision') != REVISION:
        raise ValueError('La révision compilée n’est pas celle vérifiée pour cet outil.')
    if manifest.get('installedSha256') != hashlib.sha256(binary.read_bytes()).hexdigest():
        raise ValueError('Le binaire installé a changé depuis la compilation V2.')
    build = Path(manifest['buildDirectory']).resolve()
    if not build.is_relative_to((home / 'mining').resolve()):
        raise ValueError('Emplacement des sources compilées inattendu.')
    for relative in ('config.h', 'src/sv2_noise.c', 'src/libckpool.a', 'src/secp256k1/.libs/libsecp256k1.a'):
        if not (build / relative).is_file():
            raise ValueError('Sources ou bibliothèque de la compilation V2 absentes.')
    # Exercise the upstream encoding and Noise regression programs on this VM.
    checked = subprocess.run(['make', '-j1', '-C', str(build/'test'), 'sv2_authority_b58', 'sv2_noise_hs'],
                             capture_output=True, text=True, timeout=180, check=False)
    test_log = state/'sv2-native-tests.log'
    test_log.write_text(checked.stdout + checked.stderr)
    test_log.chmod(0o600)
    if checked.returncode:
        raise ValueError('Compilation des tests Noise échouée. Journal local : '+str(test_log))
    for program in ('sv2_authority_b58', 'sv2_noise_hs'):
        checked = subprocess.run([str(build/'test'/program)], capture_output=True, text=True, timeout=90, check=False)
        if checked.returncode:
            raise ValueError('Le test natif '+program+' a échoué. Configuration conservée.')
    flags = shlex.split(subprocess.run(['pkg-config', '--cflags', '--libs', 'libsodium', 'libzmq'],
                                      capture_output=True, text=True, timeout=10, check=True).stdout)
    command = ['cc', '-O2', '-D_GNU_SOURCE', '-DHAVE_SV2', '-I'+str(build),
               '-I'+str(build/'src'), '-I'+str(build/'src/secp256k1/include'),
               str(SCRIPT_DIR/'sv2-keys.c'), str(build/'src/sv2_noise.c'),
               str(build/'src/libckpool.a'), str(build/'src/secp256k1/.libs/libsecp256k1.a'),
               *flags, '-lm', '-pthread', '-lrt']
    fd, temporary = tempfile.mkstemp(prefix='sv2-keytool-', dir=state)
    os.close(fd)
    temporary = Path(temporary)
    try:
        result = subprocess.run(command + ['-o', str(temporary)], capture_output=True, text=True, timeout=180, check=False)
        (state/'sv2-keytool-build.log').write_text(result.stdout + result.stderr)
        (state/'sv2-keytool-build.log').chmod(0o600)
        if result.returncode:
            raise ValueError('Compilation de l’outil de clés échouée. Journal local : '+str(state/'sv2-keytool-build.log'))
        temporary.chmod(0o700)
        target = state / 'sv2-keytool'
        os.replace(temporary, target)
        return target
    finally:
        temporary.unlink(missing_ok=True)


def configure(home, binary, *, keytool=None, verifier=verify_configuration):
    home = Path(home).resolve()
    binary = Path(binary)
    config_path = home/'mining/ckpool/local.conf'
    if config_path.is_symlink() or not config_path.is_file() or config_path.stat().st_uid != os.geteuid():
        raise ValueError('local.conf doit être un fichier ordinaire appartenant au compte du pool.')
    original = config_path.read_bytes()
    text = original.decode('utf-8')
    stripped = text.lstrip()
    config, end = json.JSONDecoder(object_pairs_hook=no_duplicate_keys).raw_decode(stripped)
    if not isinstance(config, dict):
        raise ValueError('Configuration CKPool non reconnue.')
    tail = stripped[end:]
    existing = config.get('sv2url')
    if existing not in (None, [], '', '0.0.0.0:3336', ['0.0.0.0:3336']):
        raise ValueError('Une écoute V2 différente est déjà configurée. Aucune modification effectuée.')
    if keytool is None:
        enabled, _ = inspect_module().capability(str(binary), str(config_path))
        if enabled is not True:
            raise ValueError('Le support SV2 doit être confirmé dans le binaire avant configuration.')
    state = home/'.local/state/bdl-pool-monitor'
    protect_directory(state)
    key_dir = home/'.local/share/ckpool/sv2'
    if not key_dir.resolve().is_relative_to(home):
        raise ValueError('Le dossier des clés doit rester dans le dossier persistant du pool.')
    protect_directory(key_dir)
    authority = Path(config.get('sv2_authority_key') or key_dir/'authority.key')
    server_static = Path(config.get('sv2_static_key') or key_dir/'static.key')
    for key_path in (authority, server_static):
        if not key_path.is_absolute() or not key_path.resolve().is_relative_to(home) or key_path.is_symlink():
            raise ValueError('Chemin de clé déjà configuré inattendu ; identité existante conservée.')
        if not key_path.parent.is_dir():
            raise ValueError('Dossier de clé existant introuvable.')
    if authority.resolve() == server_static.resolve():
        raise ValueError('Les clés d’autorité et de serveur doivent avoir des fichiers distincts.')
    executable = Path(keytool) if keytool is not None else compile_keytool(home, binary, state)
    generated = subprocess.run([str(executable), str(authority), str(server_static)],
                               capture_output=True, text=True, timeout=30, check=False)
    match = re.search(r'^SV2_AUTHORITY_PUBLIC_KEY=([1-9A-HJ-NP-Za-km-z]+)\s*$', generated.stdout, re.M)
    if generated.returncode or not match or not public_key_valid(match[1]):
        code = re.search(r'^SV2_ERROR=([a-z_]+)$', generated.stderr, re.M)
        raise ValueError('Vérification des clés/Noise échouée'+(' ('+code[1]+')' if code else '')+'. Configuration conservée.')
    public_key = match[1]
    for key_path in (authority, server_static):
        metadata = key_path.lstat()
        if not stat.S_ISREG(metadata.st_mode) or metadata.st_uid != os.geteuid() or stat.S_IMODE(metadata.st_mode) != 0o600:
            raise ValueError('Les clés privées doivent appartenir au compte du pool, en mode 600.')
    updated = dict(config, sv2url=['0.0.0.0:3336'], sv2_authority_key=str(authority), sv2_static_key=str(server_static))
    candidate_text = json.dumps(updated, indent=2, ensure_ascii=False, allow_nan=False) + '\n' + tail.lstrip('\r\n')
    fd, candidate_name = tempfile.mkstemp(prefix='.local-sv2-', suffix='.conf', dir=config_path.parent)
    candidate = Path(candidate_name)
    backup = None
    try:
        with os.fdopen(fd, 'w') as stream:
            stream.write(candidate_text)
            stream.flush()
            os.fsync(stream.fileno())
        if not verifier(binary, candidate):
            raise ValueError('Le binaire a refusé la configuration V2. Fichier original conservé.')
        if config_path.read_bytes() != original:
            raise ValueError('local.conf a changé pendant la préparation. Aucune modification effectuée.')
        if updated != config:
            suffix = datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%S%fZ')
            backup = config_path.with_name(config_path.name+'.before-sv2-'+suffix)
            fd_backup = os.open(backup, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
            with os.fdopen(fd_backup, 'wb') as stream:
                stream.write(original)
                stream.flush()
                os.fsync(stream.fileno())
            os.replace(candidate, config_path)
            config_path.chmod(0o600)
        public = dict(authorityPublicKey=public_key, stratumPort=3336, publicPort=3336)
        public_file = state/'sv2-public.json'
        public_file.write_text(json.dumps(public, indent=2)+'\n')
        public_file.chmod(0o600)
        return public_key, backup
    finally:
        candidate.unlink(missing_ok=True)


def main():
    if os.geteuid() == 0 or getpass.getuser() != 'bdl':
        raise SystemExit('Lancer avec le compte bdl, sans sudo.')
    os.umask(0o077)
    home = Path('/home/bdl')
    state = home/'.local/state/bdl-pool-monitor'
    protect_directory(state)
    with (state/'sv2-build.lock').open('w') as lock:
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            raise SystemExit('Une opération de compilation/configuration V2 est déjà en cours.')
        try:
            print('Préparation et vérification locale des clés Noise…', flush=True)
            key, backup = configure(home, Path('/usr/local/bin/ckpool'))
        except (OSError, ValueError, subprocess.SubprocessError, KeyError):
            # Do not expose exception payloads from configuration or native tools.
            error = sys.exc_info()[1]
            if isinstance(error, ValueError):
                raise SystemExit(str(error))
            raise SystemExit('Préparation V2 interrompue par une erreur de fichier ou d’outil. Vérification nécessaire avant de poursuivre.')
        print('Clés persistantes et échange Noise local vérifiés.')
        print('Écoute Stratum V2 configurée : 0.0.0.0:3336')
        print('Configuration Stratum V1 conservée.')
        if backup:
            print('Sauvegarde de local.conf :', backup)
        print('Clé publique SV2 :', key)
        print('Aucun service n’a été redémarré. La redirection TCP 3336 reste à valider.')


if __name__ == '__main__':
    main()
