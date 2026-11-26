#!/usr/bin/env python3
"""Create the private BTC Stratum ingest config from the coordinator secret."""
import argparse
import json
import os
from pathlib import Path
import tempfile


def build_config(coordinator: dict, share_log_directory: Path, cursor_path: Path) -> dict:
    token = coordinator.get("backendToken")
    if (
        not isinstance(token, str)
        or len(token) != 64
        or any(c not in "0123456789abcdef" for c in token)
    ):
        raise ValueError("Le backendToken du coordinateur est absent ou invalide.")
    if not share_log_directory.is_absolute() or not cursor_path.is_absolute():
        raise ValueError("Les chemins du journal de parts et du curseur doivent être absolus.")
    return {
        "stratumListen": "127.0.0.1:3337",
        "stratumUpstream": "127.0.0.1:3338",
        "coordinatorUrl": "http://127.0.0.1:8082/api/v1/collective",
        "backendToken": token,
        "shareLogDirectory": str(share_log_directory),
        "cursorPath": str(cursor_path),
    }


def write_private(path: Path, value: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    path.parent.chmod(0o700)
    fd, temporary = tempfile.mkstemp(prefix=".btc-ingest-", dir=path.parent)
    try:
        os.fchmod(fd, 0o600)
        with os.fdopen(fd, "w", encoding="utf-8") as output:
            json.dump(value, output, indent=2)
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
    parser.add_argument("--coordinator-config", type=Path, required=True)
    parser.add_argument("--share-log-directory", type=Path, required=True)
    parser.add_argument("--cursor-path", type=Path, required=True)
    parser.add_argument("--destination", type=Path, required=True)
    args = parser.parse_args()
    coordinator = json.loads(args.coordinator_config.read_text(encoding="utf-8"))
    value = build_config(coordinator, args.share_log_directory, args.cursor_path)
    write_private(args.destination, value)
    print("Configuration privée du collecteur BTC V1 créée (mode 0600).")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
