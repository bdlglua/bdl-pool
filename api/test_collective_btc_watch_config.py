import importlib.util
import json
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path
import threading
import tempfile
import unittest

spec = importlib.util.spec_from_file_location(
    "collective_btc_watch_config",
    Path(__file__).with_name("collective_btc_watch_config.py"),
)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class RpcHandler(BaseHTTPRequestHandler):
    authorization = None

    def do_POST(self):
        type(self).authorization = self.headers.get("Authorization")
        length = int(self.headers.get("Content-Length", "0"))
        request = json.loads(self.rfile.read(length))
        body = json.dumps({
            "result": 850000 if request["method"] == "getblockcount" else None,
            "error": None,
            "id": request["id"],
        }).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *_):
        pass


class CollectiveBtcWatchConfigTests(unittest.TestCase):
    def setUp(self):
        RpcHandler.authorization = None
        self.server = HTTPServer(("127.0.0.1", 0), RpcHandler)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()
        self.rpc_url = f"http://127.0.0.1:{self.server.server_port}/"

    def tearDown(self):
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(timeout=2)

    def pool_config(self):
        return {
            "btcaddress": "1BoatSLRHtKNngkdXEeobR76b53LETtpyT",
            "btcd": [{
                "url": self.rpc_url,
                "auth": "bdl-rpc",
                "pass": "private-rpc-password",
            }],
        }

    def coordinator_config(self):
        return {"backendToken": "a" * 64}

    def test_reads_local_bitcoin_core_and_starts_after_current_tip(self):
        url, user, password = module.get_rpc_node(self.pool_config())
        tip = module.rpc_call(url, user, password, "getblockcount")
        self.assertEqual(tip, 850000)
        self.assertEqual(
            RpcHandler.authorization,
            "Basic " + module.base64.b64encode(b"bdl-rpc:private-rpc-password").decode(),
        )
        with tempfile.TemporaryDirectory() as directory:
            config = module.build_config(
                self.pool_config(),
                self.coordinator_config(),
                Path(directory) / "btc-block-cursor.json",
                1_000_000_000,
                tip,
            )
        self.assertEqual(config["startHeight"], 850001)
        self.assertEqual(config["windowWorkUnits"], 1_000_000_000)
        self.assertEqual(config["coordinatorUrl"], module.COORDINATOR_URL)
        self.assertEqual(config["payoutAddress"], self.pool_config()["btcaddress"])

    def test_rejects_remote_rpc_and_invalid_pool_or_coordinator_settings(self):
        pool = self.pool_config()
        pool["btcd"][0]["url"] = "http://203.0.113.8:8332/"
        with self.assertRaises(ValueError):
            module.get_rpc_node(pool)

        with self.assertRaises(ValueError):
            module.build_config(
                self.pool_config(), {}, Path("/var/lib/bdl/cursor.json"), 100, 1
            )
        with self.assertRaises(ValueError):
            module.build_config(
                self.pool_config(), self.coordinator_config(),
                Path("relative-cursor.json"), 100, 1
            )
        with self.assertRaises(ValueError):
            module.build_config(
                self.pool_config(), self.coordinator_config(),
                Path("/var/lib/bdl/cursor.json"), 0, 1
            )
        bad_address = self.pool_config()
        bad_address["btcaddress"] = "not-a-bitcoin-address"
        with self.assertRaises(ValueError):
            module.build_config(
                bad_address, self.coordinator_config(),
                Path("/var/lib/bdl/cursor.json"), 100, 1
            )

    def test_private_watcher_config_is_written_mode_0600(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "nested" / "btc-watch.json"
            module.write_private(path, {"backendToken": "b" * 64})
            self.assertEqual(
                json.loads(path.read_text(encoding="utf-8"))["backendToken"], "b" * 64
            )
            self.assertEqual(path.stat().st_mode & 0o777, 0o600)
            self.assertEqual(path.parent.stat().st_mode & 0o777, 0o700)


if __name__ == "__main__":
    unittest.main()
