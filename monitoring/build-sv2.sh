#!/usr/bin/env bash
# Build the VM's existing upstream revision with native SV2 support.
set -euo pipefail
umask 077
pool_home=/home/bdl
installed=/usr/local/bin/ckpool
revision=bef6dd2efa2ed803d77fb49fbc064666685cb742
source_url=https://github.com/ckolivas/ckpool.git
script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
config="$pool_home/mining/ckpool/local.conf"
[[ "$(id -un)" == bdl ]] || { echo 'Lancer ce script avec le compte bdl, sans sudo.' >&2; exit 1; }
[[ -r "$config" && -x "$installed" ]] || { echo 'Configuration locale ou binaire CKPool absent.' >&2; exit 1; }
for tool in sudo git python3 mktemp flock; do
  command -v "$tool" >/dev/null || { echo "Outil absent : $tool" >&2; exit 1; }
done
state_dir="$pool_home/.local/state/bdl-pool-monitor"
mkdir -p "$state_dir"
exec 9>"$state_dir/sv2-build.lock"
flock -n 9 || { echo 'Une compilation SV2 est déjà en cours.' >&2; exit 1; }
if python3 "$script_dir/inspect-sv2.py" --binary "$installed" --config "$config" --require-sv2 >/dev/null 2>&1; then
  echo 'Le binaire installé possède déjà le support SV2. Aucune recompilation nécessaire.'
  exit 0
fi
[[ "$(git -C "$pool_home/mining/ckpool" rev-parse HEAD)" == "$revision" ]] || { echo 'Les sources locales ont changé de version. Aucune installation effectuée.' >&2; exit 1; }
git -C "$pool_home/mining/ckpool" diff --quiet -- src configure.ac Makefile.am || { echo 'Sources modifiées localement : vérification nécessaire avant recompilation.' >&2; exit 1; }
stage=''
cleanup() {
  if [[ -n "$stage" && -f "$stage" ]]; then sudo rm -f -- "$stage"; fi
}
trap cleanup EXIT
sudo -v
sudo apt-get update
sudo apt-get install -y --no-install-recommends build-essential autoconf automake libtool pkg-config yasm libsodium-dev libzmq3-dev git
mkdir -p "$pool_home/mining"
build_dir="$(mktemp -d "$pool_home/mining/ckpool-sv2-build.XXXXXXXX")"
echo "Compilation isolée : $build_dir"
git -C "$build_dir" init -q
git -C "$build_dir" remote add origin "$source_url"
git -C "$build_dir" fetch --depth=1 origin "$revision"
git -C "$build_dir" checkout --detach -q FETCH_HEAD
[[ "$(git -C "$build_dir" rev-parse HEAD)" == "$revision" ]] || { echo 'Révision source inattendue.' >&2; exit 1; }
git -C "$build_dir" submodule update --init --recursive
(
  cd -- "$build_dir"
  ./autogen.sh
  ./configure --enable-sv2
  # Limit memory/CPU use while Bitcoin Core is syncing on the same VM.
  make -j1
)
candidate="$build_dir/src/ckpool"
[[ -x "$candidate" ]] || { echo 'La compilation n’a pas produit de binaire CKPool.' >&2; exit 1; }
# Install only an ELF executable, never a build-directory wrapper script.
python3 - "$candidate" <<'PY'
from pathlib import Path
import sys
with Path(sys.argv[1]).open('rb') as f:
    if f.read(4) != b'\x7fELF':
        raise SystemExit('Le fichier généré n’est pas un exécutable ELF. Installation annulée.')
PY
# This diagnostic captures configuration output and emits only safe metadata.
python3 "$script_dir/inspect-sv2.py" --binary "$candidate" --config "$config" --source "$build_dir" --require-sv2
stamp="$(date -u +%Y%m%dT%H%M%SZ)"
backup="${installed}.before-sv2-${stamp}"
[[ ! -e "$backup" ]] || { echo 'Une sauvegarde du même nom existe déjà. Aucune installation effectuée.' >&2; exit 1; }
sudo cp -pL -- "$installed" "$backup"
stage="$(sudo mktemp "$(dirname -- "$installed")/.ckpool-sv2.XXXXXXXX")"
sudo install -m 0755 -- "$candidate" "$stage"
sudo mv -T -- "$stage" "$installed"
stage=''
if ! python3 "$script_dir/inspect-sv2.py" --binary "$installed" --config "$config" --source "$build_dir" --require-sv2; then
  stage="$(sudo mktemp "$(dirname -- "$installed")/.ckpool-restore.XXXXXXXX")"
  sudo cp -p -- "$backup" "$stage"
  sudo mv -T -- "$stage" "$installed"
  stage=''
  echo 'Vérification après installation échouée : ancien binaire restauré.' >&2
  exit 1
fi
python3 - "$state_dir/sv2-build.json" "$build_dir" "$revision" "$backup" "$installed" <<'PY'
from pathlib import Path
import hashlib, json, os, sys
path, build, revision, backup, binary = sys.argv[1:]
manifest = dict(buildDirectory=build, revision=revision, backupBinary=backup,
                installedSha256=hashlib.sha256(Path(binary).read_bytes()).hexdigest())
target = Path(path)
temporary = target.with_suffix('.tmp')
temporary.write_text(json.dumps(manifest, indent=2) + '\n')
os.replace(temporary, target)
PY
printf '\nSV2 compilé et nouveau binaire installé.\nAncien binaire : %s\nSources compilées : %s\n' "$backup" "$build_dir"
echo 'Les ports V2 et les clés Noise restent à configurer. Aucun service n’a été redémarré.'
