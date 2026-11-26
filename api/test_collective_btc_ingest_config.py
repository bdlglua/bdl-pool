import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location(
    "collective_btc_ingest_config", Path(__file__).with_name("collective_btc_ingest_config.py")
)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class CollectiveBtcIngestConfigTests(unittest.TestCase):
    def test_builds_loopback_only_config_and_preserves_backend_secret(self):
        token = "b" * 64
        config = module.build_config(
            {"backendToken": token},
            Path("/home/bdl/mining/ckpool/collective-logs"),
            Path("/var/lib/bdl-pool-collective/btc-share-cursors.json"),
        )
        self.assertEqual(config["stratumListen"], "127.0.0.1:3337")
        self.assertEqual(config["stratumUpstream"], "127.0.0.1:3338")
        self.assertEqual(config["coordinatorUrl"], "http://127.0.0.1:8082/api/v1/collective")
        self.assertEqual(config["backendToken"], token)
        self.assertEqual(config["shareLogDirectory"], "/home/bdl/mining/ckpool/collective-logs")
        self.assertEqual(config["cursorPath"], "/var/lib/bdl-pool-collective/btc-share-cursors.json")

    def test_rejects_bad_token_and_relative_paths(self):
        for config in ({}, {"backendToken":"bad"}, {"backendToken":"G" * 64}):
            with self.assertRaises(ValueError):
                module.build_config(config, Path("/logs"), Path("/cursor"))
        with self.assertRaises(ValueError):
            module.build_config({"backendToken":"a" * 64}, Path("logs"), Path("/cursor"))
        with self.assertRaises(ValueError):
            module.build_config({"backendToken":"a" * 64}, Path("/logs"), Path("cursor"))

    def test_private_ingest_config_is_written_mode_0600(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "nested" / "btc-ingest.json"
            module.write_private(path, {"backendToken":"c" * 64})
            self.assertEqual(json.loads(path.read_text())["backendToken"], "c" * 64)
            self.assertEqual(path.stat().st_mode & 0o777, 0o600)
            self.assertEqual(path.parent.stat().st_mode & 0o777, 0o700)


if __name__ == "__main__":
    unittest.main()
