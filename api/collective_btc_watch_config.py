#!/usr/bin/env python3
"""Build a private BTC block-watcher config and capture the starting chain tip."""
from __future__ import annotations

import argparse
import base64
import json
import os
from pathlib import Path
import tempfile
from urllib.parse import urlsplit, urlunsplit
from urllib.request import Request, urlopen

from collective_btc_config import valid_mainnet_address

MAX_U64 = (1 << 64) - 1
COORDINATOR_URL = "http://127.0.0.1:8082/api/v1/collective"


def read_first_json(path: Path) -> dict:
    text = path.read_text(encoding="utf-8")
    decoder = json.JSONDecoder()
    value, _ = decoder.raw_decode(text.lstrip())
    if not isinstance(value, dict):
        raise ValueError("Le premier document de configuration doit être un objet JSON.")
    return value


def local_rpc_url(value: str) -> str:
    if not isinstance(value, str) or not value or len(value) > 512:
        raise ValueError("URL RPC Bitcoin absente ou invalide.")
    candidate = value if "://" in value else "http://" + value
    parsed = urlsplit(candidate)
    if (
        parsed.scheme != "http"
        or parsed.hostname not in ("127.0.0.1", "localhost", "::1")
        or parsed.port is None
        or not 1 <= parsed.port <= 65535
        or parsed.username is not None
        or parsed.password is not None
        or parsed.path not in ("", "/")
        or parsed.query
        or parsed.fragment
    ):
        raise ValueError("Le RPC Bitcoin doit être un endpoint HTTP local sur loopback.")
    return urlunsplit(("http", parsed.netloc, "/", "", ""))


def get_rpc_node(pool_config: dict) -> tuple[str, str, str]:
    nodes = pool_config.get("btcd")
    if not isinstance(nodes, list) or not nodes:
        raise ValueError("La configuration CKPool ne contient aucun nœud btcd.")
    for node in nodes:
        if not isinstance(node, dict):
            continue
        try:
            url = local_rpc_url(node.get("url"))
        except (ValueError, TypeError):
            continue
        user, password = node.get("auth"), node.get("pass")
        if (
            isinstance(user, str) and user and len(user) <= 128 and ":" not in user
            and isinstance(password, str) and password and len(password) <= 256
            and not any(ord(ch) < 32 or ord(ch) == 127 for ch in user + password)
        ):
            return url, user, password
    raise ValueError("Aucun nœud Bitcoin Core local avec identifiants RPC valides n’a été trouvé.")


def rpc_call(url: str, user: str, password: str, method: str) -> int:
    credentials = base64.b64encode((user + ":" + password).encode("utf-8")).decode("ascii")
    payload = json.dumps({
        "jsonrpc": "1.0", "id": "bdl-collective-install", "method": method, "params": []
    }).encode("utf-8")
    request = Request(url, data=payload, headers={
        "Authorization": "Basic " + credentials,
        "Content-Type": "application/json",
    })
    try:
        with urlopen(request, timeout=5) as response:
            value = json.loads(response.read(65537).decode("utf-8"))
    except Exception as error:
        raise ValueError("Impossible de joindre le RPC Bitcoin local ; aucune configuration watcher créée.") from error
    if not isinstance(value, dict) or value.get("error") is not None:
        raise ValueError("Bitcoin Core a refusé la lecture de la hauteur actuelle.")
    height = value.get("result")
    if type(height) is not int or not 0 <= height < MAX_U64:
        raise ValueError("Bitcoin Core a renvoyé une hauteur de chaîne invalide.")
    return height


def build_config(
    pool_config: dict,
    coordinator_config: dict,
    cursor_path: Path,
    window_work_units: int,
    tip_height: int,
) -> dict:
    if type(window_work_units) is not int or not 1 <= window_work_units <= MAX_U64:
        raise ValueError("La fenêtre PPLNS doit être un entier positif sur 64 bits.")
    if not cursor_path.is_absolute():
        raise ValueError("Le chemin du curseur doit être absolu.")
    address = pool_config.get("btcaddress")
    if not isinstance(address, str) or not valid_mainnet_address(address):
        raise ValueError("L’adresse BTC commune de collective.conf est absente ou invalide.")
    token = coordinator_config.get("backendToken")
    if (
        not isinstance(token, str) or len(token) != 64
        or any(char not in "0123456789abcdef" for char in token)
    ):
        raise ValueError("Le backendToken du coordinateur est absent ou invalide.")
    rpc_url, rpc_user, rpc_password = get_rpc_node(pool_config)
    if type(tip_height) is not int or not 0 <= tip_height < MAX_U64:
        raise ValueError("Hauteur Bitcoin invalide.")
    return {
        "rpcUrl": rpc_url,
        "rpcUser": rpc_user,
        "rpcPassword": rpc_password,
        "coordinatorUrl": COORDINATOR_URL,
        "backendToken": token,
        "payoutAddress": address,
        "windowWorkUnits": window_work_units,
        # Start after the tip observed during installation: old solo blocks
        # can never be mistaken for collective-pool rewards.
        "startHeight": tip_height + 1,
        "cursorPath": str(cursor_path),
        "pollIntervalSeconds": 15,
    }


def write_private(path: Path, value: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    path.parent.chmod(0o700)
    descriptor, temporary = tempfile.mkstemp(prefix=".btc-watch-", dir=path.parent)
    try:
        os.fchmod(descriptor, 0o600)
        with os.fdopen(descriptor, "w", encoding="utf-8") as output:
            json.dump(value, output, indent=2, sort_keys=True)
            output.write("\n")
            output.flush()
            os.fsync(output.fileno())
        os.replace(temporary, path)
        os.chmod(path, 0o600)
    except Exception:
        try:
            os.unlink(temporary)
        except FileNotFoundError:
            pass
        raise


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pool-config", type=Path, required=True)
    parser.add_argument("--coordinator-config", type=Path, required=True)
    parser.add_argument("--cursor-path", type=Path, required=True)
    parser.add_argument("--destination", type=Path, required=True)
    parser.add_argument("--window-work-units", type=int, required=True)
    args = parser.parse_args()
    pool = read_first_json(args.pool_config)
    coordinator = read_first_json(args.coordinator_config)
    rpc_url, rpc_user, rpc_password = get_rpc_node(pool)
    tip = rpc_call(rpc_url, rpc_user, rpc_password, "getblockcount")
    value = build_config(pool, coordinator, args.cursor_path, args.window_work_units, tip)
    write_private(args.destination, value)
    print("Configuration privée du watcher BTC créée (mode 0600).")
    print(f"Hauteur de départ : {value['startHeight']} (aucun bloc antérieur ne sera attribué au collectif).")
    print("Maturité requise : 100 confirmations. Aucun paiement ne sera signé ou diffusé.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
