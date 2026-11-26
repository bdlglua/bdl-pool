import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location(
    "collective_btc_config", Path(__file__).with_name("collective_btc_config.py")
)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class CollectiveBtcConfigTests(unittest.TestCase):
    def test_validates_mainnet_base58_and_segwit_addresses(self):
        self.assertTrue(module.valid_mainnet_address("1BoatSLRHtKNngkdXEeobR76b53LETtpyT"))
        self.assertTrue(module.valid_mainnet_address("3J98t1WpEZ73CNmQviecrnyiWrnqRhWNLy"))
        self.assertTrue(module.valid_mainnet_address("bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kygt080"))
        self.assertFalse(module.valid_mainnet_address("not-a-btc-address"))
        self.assertFalse(module.valid_mainnet_address("mipcBbFg9gMiCh81Kj8tqqdgoZub1ZJRfn"))
        self.assertFalse(module.valid_mainnet_address("bc1notvalid"))

    def test_config_preserves_rpc_and_isolates_pool_ports_and_logs(self):
        base = {
            "btcd": [{"url":"127.0.0.1:8332","auth":"rpcuser","pass":"rpcpass","notify":True}],
            "btcaddress":"1BoatSLRHtKNngkdXEeobR76b53LETtpyT",
            "serverurl":["0.0.0.0:3333"],
            "upstream":"pool.example:3333",
            "proxies":[{"url":"pool.example:3333"}],
            "sv2url":["stratum2+tcp://example:3336"],
            "logdir":"logs",
            "donation":2.0,
            "mindiff":1,
            "startdiff":10000,
            "zmqblock":"tcp://127.0.0.1:28332"
        }
        config = module.build_config(
            base,
            "3J98t1WpEZ73CNmQviecrnyiWrnqRhWNLy",
            "/home/bdl/mining/ckpool/collective-logs"
        )
        self.assertEqual(config["btcd"], base["btcd"])
        self.assertEqual(config["zmqblock"], base["zmqblock"])
        self.assertEqual(config["btcaddress"], "3J98t1WpEZ73CNmQviecrnyiWrnqRhWNLy")
        self.assertEqual(config["serverurl"], ["127.0.0.1:3338"])
        self.assertEqual(config["logdir"], "/home/bdl/mining/ckpool/collective-logs/")
        self.assertEqual(config["donation"], 0.0)
        self.assertEqual(config["mindiff"], 1)
        self.assertEqual(config["startdiff"], 1)
        self.assertEqual(config["maxdiff"], 0)
        for key in ("upstream","proxies","sv2url"):
            self.assertNotIn(key, config)
        self.assertEqual(base["serverurl"], ["0.0.0.0:3333"])
        self.assertEqual(base["donation"], 2.0)

    def test_rejects_missing_or_incomplete_bitcoin_rpc_configuration(self):
        address = "1BoatSLRHtKNngkdXEeobR76b53LETtpyT"
        with self.assertRaises(ValueError):
            module.build_config({}, address, "/tmp/logs")
        with self.assertRaises(ValueError):
            module.build_config({"btcd":[{"url":"127.0.0.1:8332","auth":"user"}]}, address, "/tmp/logs")

    def test_first_json_document_allows_ckpool_trailing_comment(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "local.conf"
            path.write_text('{"btcd":[]}\\nComments from here on are ignored.\\n', encoding="utf-8")
            self.assertEqual(module.read_first_json(path), {"btcd":[]})

    def test_private_config_is_atomic_and_mode_0600(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "nested" / "collective.conf"
            module.write_private_json(path, {"donation":0.0})
            self.assertEqual(json.loads(path.read_text()), {"donation":0.0})
            self.assertEqual(path.stat().st_mode & 0o777, 0o600)
            self.assertEqual(path.parent.stat().st_mode & 0o777, 0o700)


if __name__ == "__main__":
    unittest.main()
