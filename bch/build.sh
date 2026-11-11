#!/usr/bin/env bash
# Build pinned BCH binaries in private versioned paths, never over Bitcoin Core.
set -euo pipefail
[[ $(id -un) == bdl && "$HOME" == /home/bdl ]] || { echo 'Lancer avec le compte bdl.' >&2; exit 1; }
for tool in git cmake ninja make autoconf automake libtoolize pkg-config; do
  command -v "$tool" >/dev/null || { echo "Outil absent : $tool (voir bch/README.md)." >&2; exit 1; }
done
build_root="$HOME/.cache/bdl-pool-bch-build"
mkdir -p "$build_root" "$HOME/.local/opt"
exec 9>"$build_root/build.lock"
flock -n 9 || { echo 'Une compilation BCH est déjà en cours.' >&2; exit 1; }
checkout() {
  local name="$1" remote="$2" ref="$3" expected="$4"
  if [[ ! -d "$build_root/$name" ]]; then
    git clone --depth 1 --branch "$ref" "$remote" "$build_root/$name"
  fi
  [[ $(git -C "$build_root/$name" rev-parse HEAD) == "$expected" ]] || { echo "Révision inattendue : $name" >&2; exit 1; }
  git -C "$build_root/$name" diff --exit-code
  git -C "$build_root/$name" diff --cached --exit-code
}
checkout bchn https://github.com/bitcoin-cash-node/bitcoin-cash-node.git v29.2.0 07576013c91ff4a3a74acd85f189c69121cdad1b
checkout cashstratum https://github.com/cashstratum/cashstratum.git v1.7.1 8ce7fd78fc8e3481c123a0d3284536ec2299c9d9
cmake -S "$build_root/bchn" -B "$build_root/bchn/build" -GNinja \
  -DBUILD_BITCOIN_WALLET=OFF -DBUILD_BITCOIN_QT=OFF -DBUILD_BITCOIN_ZMQ=OFF \
  -DENABLE_UPNP=OFF -DENABLE_NATPMP=OFF -DENABLE_MAN=OFF -DBUILD_BITCOIN_TESTS=OFF -DBUILD_BITCOIN_BENCH=OFF
cmake --build "$build_root/bchn/build" --target bitcoind bitcoin-cli --parallel "${BCH_BUILD_JOBS:-2}"
install -d "$HOME/.local/opt/bchn-29.2.0/bin" "$HOME/.local/opt/cashstratum-1.7.1/bin"
install -m 755 "$build_root/bchn/build/src/bitcoind" "$build_root/bchn/build/src/bitcoin-cli" "$HOME/.local/opt/bchn-29.2.0/bin/"
cd "$build_root/cashstratum"
./autogen.sh
./configure --disable-sv2
make -j "${BCH_BUILD_JOBS:-2}"
make check
install -m 755 src/ckpool "$HOME/.local/opt/cashstratum-1.7.1/bin/cashstratum"
install -m 755 src/ckpmsg "$HOME/.local/opt/cashstratum-1.7.1/bin/ckpmsg"
echo 'Binaires BCH compilés. Suite : bch/install.sh (voir bch/README.md).'
