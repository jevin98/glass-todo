"""Call GlassTodo's data MCP directly, including with a stale agent tool catalog.

No desktop automation or direct database writes. No automatic retries of writes.
Credentials come from the app's local data folder and are never printed.
"""
import argparse
import json
import os
from pathlib import Path
import sys
import tomllib
import urllib.parse
import urllib.request


class Client:
    def __init__(self, data_dir=None, url=None):
        root = Path(data_dir) if data_dir else Path(os.environ["LOCALAPPDATA"]) / "GlassTodo"
        if url is None:
            config = tomllib.loads((root / "mcp-configs/codex.toml").read_text(encoding="utf-8-sig"))
            url = config["mcp_servers"]["glass_todo"]["url"]
        parsed = urllib.parse.urlsplit(url)
        if (parsed.scheme != "http" or parsed.hostname not in {"127.0.0.1", "localhost", "::1"}
                or parsed.username or parsed.password or parsed.path != "/mcp"
                or parsed.query or parsed.fragment):
            raise ValueError("GlassTodo MCP URL must be a local HTTP /mcp endpoint")
        self.url = url
        self.sequence = 0
        self.headers = {
            "Authorization": "Bearer " + (root / "mcp-token.txt").read_text().strip(),
            "Content-Type": "application/json",
            "Accept": "application/json, text/event-stream",
            "MCP-Protocol-Version": "2025-03-26",
        }
        # Loopback must stay local even if the shell has an HTTP proxy configured.
        self.http = urllib.request.build_opener(urllib.request.ProxyHandler({}))
        self.info = self.request("initialize", {
            "protocolVersion": "2025-03-26", "capabilities": {},
            "clientInfo": {"name": "glass-todo-data-client", "version": "1"},
        })
        self.request("notifications/initialized", {}, notification=True)

    def request(self, method, params, notification=False):
        self.sequence += 1
        body = {"jsonrpc": "2.0", "method": method, "params": params}
        if not notification:
            body["id"] = self.sequence
        request = urllib.request.Request(self.url, json.dumps(body).encode(), self.headers)
        with self.http.open(request, timeout=15) as response:
            if response.headers.get("Mcp-Session-Id"):
                self.headers["Mcp-Session-Id"] = response.headers["Mcp-Session-Id"]
            if notification:
                return None
            if "text/event-stream" in response.headers.get("Content-Type", ""):
                result = None
                for line in response:
                    if line.startswith(b"data:") and line[5:].strip():
                        candidate = json.loads(line[5:])
                        if candidate.get("id") == body["id"]:
                            result = candidate
                            break
                if result is None:
                    raise RuntimeError("MCP returned no matching result")
            else:
                result = json.load(response)
        if "error" in result:
            raise RuntimeError(result["error"].get("message", "MCP request failed"))
        return result["result"]

    def tools(self):
        return self.request("tools/list", {})["tools"]

    def close(self):
        if "Mcp-Session-Id" in self.headers:
            try:
                request = urllib.request.Request(self.url, headers=self.headers, method="DELETE")
                with self.http.open(request, timeout=3):
                    pass
            except OSError:
                pass  # Session termination is optional in Streamable HTTP.
            self.headers.pop("Mcp-Session-Id", None)

    def __enter__(self):
        return self

    def __exit__(self, *_):
        self.close()

    def call(self, name, arguments=None):
        result = self.request("tools/call", {"name": name, "arguments": arguments or {}})
        text = next((block["text"] for block in result.get("content", []) if block["type"] == "text"), "")
        if result.get("isError"):
            raise RuntimeError(text or "GlassTodo data operation failed")
        return json.loads(text) if text else result.get("structuredContent")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--data-dir", type=Path, help="Local GlassTodo data folder")
    parser.add_argument("--url", help="Override the local MCP URL")
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("inspect", help="Read current server capabilities, without todo contents")
    commands.add_parser("tools", help="Read the current tool schemas from the server")
    call = commands.add_parser("call", help="Invoke one MCP data tool")
    call.add_argument("tool")
    arguments = call.add_mutually_exclusive_group()
    arguments.add_argument("--arguments-file", type=Path, help="UTF-8 JSON object")
    arguments.add_argument("--arguments-json", help="JSON object; prefer a file for multiline content")
    args = parser.parse_args()
    with Client(args.data_dir, args.url) as client:
        result = run(args, client)
    print(json.dumps(result, ensure_ascii=False, indent=2))


def run(args, client):
    if args.command == "tools":
        result = client.tools()
    elif args.command == "inspect":
        tools = client.tools()
        update = next((item for item in tools if item["name"] == "update_todo"), {})
        result = {
            "server": client.info.get("serverInfo"), "transport": "HTTP MCP data API",
            "tool_count": len(tools), "tools": [item["name"] for item in tools],
            "custom_status_assignment": "status_id" in update.get("inputSchema", {}).get("properties", {}),
        }
    else:
        value = args.arguments_file.read_text(encoding="utf-8-sig") if args.arguments_file else args.arguments_json or "{}"
        arguments = json.loads(value)
        if not isinstance(arguments, dict):
            raise ValueError("Tool arguments must be a JSON object")
        result = client.call(args.tool, arguments)
    return result


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stderr.reconfigure(encoding="utf-8")
    try:
        main()
    except Exception as error:
        print(f"GlassTodo MCP: {error}", file=sys.stderr)
        sys.exit(1)
