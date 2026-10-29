"""Integration test of the direct data client against private headless app data."""
from pathlib import Path
import json
import socket
import subprocess
import sys
import time
import unittest

from glass_todo_mcp import Client


class DataClientTest(unittest.TestCase):
    def test_custom_status_updates_use_mcp_without_any_desktop_window(self):
        root = Path(__file__).resolve().parents[1]
        data = root / ".scratch" / f"qa-data-client-{time.time_ns()}"
        data.mkdir(parents=True)
        with socket.socket() as socket_:
            socket_.bind(("127.0.0.1", 0))
            port = socket_.getsockname()[1]
        url = f"http://127.0.0.1:{port}/mcp"
        process = subprocess.Popen([
            str(root / "target/release/glass-todo.exe"), "--headless", "--no-autostart",
            "--data-dir", str(data), "--port", str(port),
        ], creationflags=subprocess.CREATE_NO_WINDOW)
        try:
            for _ in range(100):
                if process.poll() is not None:
                    self.fail("Private MCP process exited unexpectedly")
                if (data / "mcp-token.txt").exists():
                    try:
                        with socket.create_connection(("127.0.0.1", port), timeout=0.1):
                            break
                    except OSError:
                        pass
                time.sleep(0.05)
            else:
                self.fail("Private MCP process did not start")
            # The default URL must also be discoverable from app-generated config.
            with Client(data) as client:
                self.assertEqual(client.url, url)
                tools = client.tools()
                self.assertEqual(len(tools), 11)
                update = next(item for item in tools if item["name"] == "update_todo")
                self.assertIn("status_id", update["inputSchema"]["properties"])
                status = client.call("create_status", {"name": "发布测试", "kind": "in_progress", "color": "purple"})
                todo = client.call("create_todo", {"content": "私有测试正文\n保留第二行"})
                before = client.call("get_todo", {"id": todo["id"]})
                after = client.call("update_todo", {
                    "id": todo["id"], "status_id": status["id"], "expected_version": before["version"],
                })
                self.assertEqual(after["status_name"], "发布测试")
                self.assertEqual(after["content"], before["content"])
                with self.assertRaises(RuntimeError):
                    client.call("update_todo", {"id": todo["id"], "status": "done", "expected_version": before["version"]})
                self.assertEqual(client.call("get_todo", {"id": todo["id"]}), after)
                arguments = data / "update-arguments.json"
                arguments.write_text(json.dumps({
                    "id": todo["id"], "content": after["content"] + "\n附加测试链接",
                    "status_id": status["id"], "expected_version": after["version"],
                }, ensure_ascii=False), encoding="utf-8")
                result = subprocess.run([
                    sys.executable, str(root / "scripts/glass_todo_mcp.py"), "--data-dir", str(data),
                    "call", "update_todo", "--arguments-file", str(arguments),
                ], capture_output=True, encoding="utf-8", check=True)
                saved = json.loads(result.stdout)
                self.assertEqual(saved["status_id"], status["id"])
                self.assertIn("附加测试链接", saved["content"])
                self.assertEqual(client.call("get_todo", {"id": todo["id"]}), saved)
                session = client.headers.get("Mcp-Session-Id")
                client.close()
                self.assertNotIn("Mcp-Session-Id", client.headers)
                self.assertIsNotNone(session)
            self.assertIsNone(process.poll(), "closing a client session must keep the app running")
            with Client(data) as client:
                self.assertEqual(client.call("get_todo", {"id": todo["id"]}), saved)
            seed = [sys.executable, str(root / "scripts/add_release_states.py"), "--data-dir", str(data)]
            subprocess.run(seed, capture_output=True, encoding="utf-8", check=True)
            repeated = subprocess.run(seed, capture_output=True, encoding="utf-8", check=True)
            self.assertTrue(all(not item["created"] for item in json.loads(repeated.stdout)))
            with Client(data) as client:
                self.assertEqual(len(client.call("list_statuses")), 5)
            with self.assertRaises(ValueError):
                Client(data, "https://example.com/mcp")
        finally:
            process.terminate()  # Only this private, windowless test process.
            process.wait(timeout=5)


if __name__ == "__main__":
    unittest.main()
