#!/usr/bin/env bash
# Push only the collector's filtered JSON to the separate stats branch.
set -euo pipefail
umask 077
script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
state_dir="${XDG_STATE_HOME:-$HOME/.local/state}/bdl-pool-monitor"
mkdir -p "$state_dir"
exec 9>"$state_dir/publish.lock"
flock -n 9 || exit 0
key_path="$HOME/.ssh/bdl-pool-stats"
[[ -f "$key_path" ]] || { echo 'Clé SSH bdl-pool-stats absente.' >&2; exit 1; }
printf -v GIT_SSH_COMMAND 'ssh -i %q -o IdentitiesOnly=yes -o BatchMode=yes -o StrictHostKeyChecking=yes -o ConnectTimeout=15' "$key_path"
export GIT_SSH_COMMAND
export GIT_TERMINAL_PROMPT=0
stats_repo="$state_dir/repository"
remote='git@github.com:bdlglua/bdl-pool.git'
if [[ ! -d "$stats_repo/.git" ]]; then
  git clone --single-branch --branch stats "$remote" "$stats_repo"
fi
[[ "$(git -C "$stats_repo" remote get-url origin)" == "$remote" ]] || { echo 'Dépôt de statistiques inattendu.' >&2; exit 1; }
[[ "$(git -C "$stats_repo" branch --show-current)" == stats ]] || { echo 'Branche de statistiques inattendue.' >&2; exit 1; }
[[ -z "$(git -C "$stats_repo" status --porcelain)" ]] || { echo 'Modifications locales dans le dossier de statistiques. Vérifiez-les avant de réessayer.' >&2; exit 1; }
git -C "$stats_repo" pull --ff-only origin stats
scratch="$(mktemp "$state_dir/status.XXXXXX")"
trap 'rm -f -- "$scratch"' EXIT
python3 "$script_dir/collect.py" --previous "$stats_repo/status.json" > "$scratch"
cp "$scratch" "$stats_repo/status.json"
# Never git add a directory, configuration, log, wallet, or authentication file.
git -C "$stats_repo" add -- status.json
git -C "$stats_repo" -c user.name='BDL Pool monitoring' -c user.email='86241016+bdlglua@users.noreply.github.com' commit -m 'Update public pool snapshot'
git -C "$stats_repo" push origin HEAD:stats
echo 'Relevé publié sur GitHub.'
