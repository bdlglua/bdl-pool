#!/usr/bin/env python3
"""Build a separate pool-mode CKPool config for BDL collective BTC V1."""
from __future__ import annotations

import argparse
import copy
import hashlib
import json
import os
from pathlib import Path
import re
import tempfile


_B58 = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
_BECH32 = "qpzry9x8gf2tvdw0s3jn54khce6mua7l"
_BECH32_GENERATORS = (0x3B6A57B2, 0x26508E6D, 0x1EA119FA, 0x3D4233DD, 0x2A1462B3)


def _base58check(address: str) -> bool:
    number = 0
    for char in address:
        try:
            digit = _B58.index(char)
        except ValueError:
            return False
        number = number * 58 + digit
    body = number.to_bytes((number.bit_length() + 7) // 8, "big") if number else b""
    body = b"\0" * (len(address) - len(address.lstrip("1"))) + body
    if len(body) != 25 or body[0] not in (0x00, 0x05):
        return False
    checksum = hashlib.sha256(hashlib.sha256(body[:-4]).digest()).digest()[:4]
    return checksum == body[-4:]


def _polymod(values: list[int]) -> int:
    checksum = 1
    for value in values:
        top = checksum >> 25
        checksum = ((checksum & 0x1FFFFFF) << 5) ^ value
        for bit, generator in enumerate(_BECH32_GENERATORS):
            if (top >> bit) & 1:
                checksum ^= generator
    return checksum


def _convertbits(data: list[int], from_bits: int, to_bits: int) -> bytes | None:
    accumulator = 0
    bits = 0
    output = bytearray()
    max_value = (1 << to_bits) - 1
    max_accumulator = (1 << (from_bits + to_bits - 1)) - 1
    for value in data:
        if value < 0 or value >> from_bits:
            return None
        accumulator = ((accumulator << from_bits) | value) & max_accumulator
        bits += from_bits
        while bits >= to_bits:
            bits -= to_bits
            output.append((accumulator >> bits) & max_value)
    if bits >= from_bits or ((accumulator << (to_bits - bits)) & max_value):
        return None
    return bytes(output)


def _segwit_address(address: str) -> bool:
    if len(address) > 90 or address.lower() != address and address.upper() != address:
        return False
    value = address.lower()
    separator = value.rfind("1")
    if separator < 1 or separator + 7 > len(value):
        return False
    hrp = value[:separator]
    if hrp != "bc":
        return False
    try:
        data = [_BECH32.index(char) for char in value[separator + 1:]]
    except ValueError:
        return False
    expanded = [ord(char) >> 5 for char in hrp] + [0] + [ord(char) & 31 for char in hrp]
    checksum = _polymod(expanded + data)
    payload = data[:-6]
    if not payload:
        return False
    witness_version = payload[0]
    if witness_version > 16:
        return False
    program = _convertbits(payload[1:], 5, 8)
    if program is None or not 2 <= len(program) <= 40:
        return False
    if witness_version == 0:
        return checksum == 1 and len(program) in (20, 32)
    return checksum == 0x2BC830A3


def valid_mainnet_address(address: str) -> bool:
    if not isinstance(address, str) or not address or len(address) > 90:
        return False
    return _base58check(address) or _segwit_address(address)


def read_first_json(path: Path) -> dict:
    text = path.read_text(encoding="utf-8")
    decoder = json.JSONDecoder()
    value, _ = decoder.raw_decode(text.lstrip())
    if not isinstance(value, dict):
        raise ValueError("Le premier document de local.conf doit être un objet JSON.")
    return value


def build_config(base: dict, payout_address: str, log_directory: str) -> dict:
    if not valid_mainnet_address(payout_address):
        raise ValueError("Adresse BTC mainnet invalide (Base58Check ou bech32/bech32m attendu).")
    rpc_nodes = base.get("btcd")
    if not isinstance(rpc_nodes, list) or not rpc_nodes:
        raise ValueError("La configuration CKPool existante ne contient pas de liste btcd.")
    for node in rpc_nodes:
        if not isinstance(node, dict) or not node.get("url") or not node.get("auth") or not node.get("pass"):
            raise ValueError("Un nœud btcd existant ne contient pas url/auth/pass ; arrêt sans modification.")

    config = copy.deepcopy(base)
    # This new instance must build its own pool templates from the local node,
    # never forward work to an upstream pool or create extra proxy listeners.
    for key in (
        "upstream", "proxy", "proxies", "nodeserver", "trusted",
        "passthroughserver", "redirecturl", "sv2url", "sv2jdurl",
    ):
        config.pop(key, None)
    config["btcaddress"] = payout_address
    config["serverurl"] = ["127.0.0.1:3338"]
    config["logdir"] = str(Path(log_directory)) + "/"
    config["donation"] = 0.0
    # CKPool's initial vardiff is integral. Starting at diff 1 prevents slow
    # ESP32-class workers from receiving the very high default start difficulty.
    config["mindiff"] = 1
    config["startdiff"] = 1
    config["maxdiff"] = 0
    return config


def write_private_json(path: Path, value: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    path.parent.chmod(0o700)
    fd, temporary = tempfile.mkstemp(prefix=".collective-", dir=path.parent)
    try:
        os.fchmod(fd, 0o600)
        with os.fdopen(fd, "w", encoding="utf-8") as output:
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
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--destination", type=Path, required=True)
    parser.add_argument("--btc-address", required=True)
    parser.add_argument("--log-directory", required=True)
    args = parser.parse_args()
    base = read_first_json(args.source)
    config = build_config(base, args.btc_address, args.log_directory)
    write_private_json(args.destination, config)
    print("Configuration CKPool collective créée (fichier privé, mode 0600).")
    print("RPC Bitcoin existant conservé ; listener V1 lié à 127.0.0.1:3338.")
    print("Récompenses vers adresse commune ; donation désactivée ; difficulté initiale 1.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
