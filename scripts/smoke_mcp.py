"""Exercise a real Release server, including authorization, conflicts and crash/reopen.

Usage: python scripts/smoke_mcp.py [path/to/glass-todo.exe]
Test data stays under .scratch/qa-mcp-*; no personal client configuration is changed.
"""
from pathlib import Path
import json, os, socket, subprocess, sys, time, urllib.request, urllib.error

ROOT = Path(__file__).resolve().parents[1]
EXE = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else ROOT / "target/release/glass-todo.exe"
DATA = ROOT / ".scratch" / ("qa-mcp-" + str(time.time_ns()))
DATA.mkdir(parents=True)
with socket.socket() as sock:
    sock.bind(("127.0.0.1", 0))
    PORT = sock.getsockname()[1]
URL = f"http://127.0.0.1:{PORT}/mcp"
token, session, request_id = "", None, 0
PROTOCOL = os.environ.get("MCP_TEST_PROTOCOL", "2025-03-26")

def start():
    global token, session
    session = None
    process = subprocess.Popen([str(EXE), "--headless", "--no-autostart", "--data-dir", str(DATA), "--port", str(PORT)], creationflags=subprocess.CREATE_NO_WINDOW)
    for _ in range(100):
        if process.poll() is not None:
            raise RuntimeError("Application exited before MCP became ready")
        try:
            token = (DATA / "mcp-token.txt").read_text().strip()
            with socket.create_connection(("127.0.0.1", PORT), timeout=.2):
                return process
        except (OSError, FileNotFoundError):
            time.sleep(.1)
    process.kill()
    raise RuntimeError("MCP did not become ready")

def call(method, params=None, notification=False, auth=True):
    global request_id, session
    request_id += 1
    payload = {"jsonrpc":"2.0", "method":method}
    if not notification:
        payload["id"] = request_id
    if params is not None:
        payload["params"] = params
    headers = {"Content-Type":"application/json", "Accept":"application/json, text/event-stream", "MCP-Protocol-Version":PROTOCOL}
    if PROTOCOL == "2026-07-28":
        payload.setdefault("params", {})["_meta"] = {
            "io.modelcontextprotocol/protocolVersion": PROTOCOL,
            "io.modelcontextprotocol/clientInfo": {"name":"glass-todo-smoke","version":"1"},
            "io.modelcontextprotocol/clientCapabilities": {},
        }
        headers["Mcp-Method"] = method
        if method == "tools/call":
            headers["Mcp-Name"] = payload["params"]["name"]
    if auth:
        headers["Authorization"] = "Bearer " + token
    if session:
        headers["Mcp-Session-Id"] = session
    req = urllib.request.Request(URL, data=json.dumps(payload).encode(), headers=headers)
    with urllib.request.urlopen(req, timeout=15) as response:
        session = response.getheader("Mcp-Session-Id") or session
        if response.status == 202:
            return None
        if "text/event-stream" in response.headers.get("Content-Type", ""):
            for line in response:
                if line.startswith(b"data:"):
                    data=line[5:].strip()
                    if not data:
                        continue
                    value = json.loads(data)
                    if value.get("id") == payload.get("id"):
                        return value
            raise RuntimeError("Missing MCP SSE response")
        raw=response.read()
        return json.loads(raw) if raw else None

def initialize():
    global PROTOCOL
    if PROTOCOL == "2026-07-28":
        result=call("server/discover", {})
        assert "2026-07-28" in result["result"]["supportedVersions"], result
        return
    result=call("initialize", {"protocolVersion":PROTOCOL, "capabilities":{}, "clientInfo":{"name":"glass-todo-smoke", "version":"1"}})
    assert "result" in result, result
    PROTOCOL=result["result"]["protocolVersion"]
    call("notifications/initialized", notification=True)

def tool(name, arguments, error=False):
    response=call("tools/call", {"name":name,"arguments":arguments})
    assert "result" in response, response
    result=response["result"]
    assert bool(result.get("isError")) == error, result
    text=result["content"][0]["text"]
    return text if error else json.loads(text)

process=None
try:
    process=start()
    try:
        call("tools/list", {}, auth=False)
        raise AssertionError("Unauthenticated request was accepted")
    except urllib.error.HTTPError as e:
        assert e.code == 401
    initialize()
    tools=call("tools/list", {})["result"]["tools"]
    assert {"create_todo","update_todo","delete_todo","undo_delete","reorder_todos"} <= {t["name"] for t in tools}
    first=tool("create_todo", {"content":"中文测试\n第二行"})
    second=tool("create_todo", {"content":"另一条"})
    active=tool("update_todo", {"id":first["id"],"status":"in_progress","expected_version":first["version"]})
    assert active["status"] == "in_progress"
    conflict=tool("update_todo", {"id":first["id"],"content":"不应覆盖","expected_version":first["version"]}, error=True)
    assert "版本冲突" in conflict
    tool("reorder_todos", {"ids":[second["id"],first["id"]]})
    assert tool("list_todos", {})[0]["id"] == second["id"]
    tool("delete_todo", {"id":second["id"]})
    assert len(tool("list_todos", {})) == 1
    # Deliberately terminate the test process after an acknowledged transaction.
    process.kill(); process.wait(timeout=10)
    process=start(); initialize()
    saved=tool("get_todo", {"id":first["id"]})
    assert saved["status"] == "in_progress" and saved["content"] == "中文测试\n第二行"
    assert tool("undo_delete", {})["id"] == second["id"]
    done=tool("update_todo", {"id":first["id"],"status":"done"})
    assert done["status"] == "done"
    pending=tool("update_todo", {"id":first["id"],"status":"pending"})
    assert pending["status"] == "pending"
    assert {'list_statuses','create_status','update_status','delete_status'} <= {t['name'] for t in tools}
    assert len(tool('list_statuses',{})) == 3
    definition=tool('create_status',{'name':'等待反馈','kind':'in_progress','color':'purple'})
    assigned=tool('update_todo',{'id':first['id'],'status_id':definition['id'],'expected_version':pending['version']})
    assert (assigned['status_id'],assigned['status_name'],assigned['status_color'],assigned['status'])==(definition['id'],'等待反馈','purple','in_progress')
    assert len(tool('list_todos',{'status_id':definition['id']}))==1
    tool('create_status',{'name':'等待反馈','kind':'pending','color':'gray'},error=True)
    process.kill(); process.wait(timeout=10)
    process=start(); initialize()
    assert tool('get_todo',{'id':first['id']})['status_id']==definition['id']
    assert any(s['id']==definition['id'] for s in tool('list_statuses',{}))
    changed=tool('update_status',{'id':definition['id'],'name':'已验收','kind':'done','color':'green','expected_version':definition['version']})
    after=tool('get_todo',{'id':first['id']})
    assert after['status']=='done' and after['status_name']=='已验收' and after['version']==assigned['version']+1
    tool('update_todo',{'id':first['id'],'content':'不应覆盖','expected_version':assigned['version']},error=True)
    tool('update_status',{'id':definition['id'],'name':'不应覆盖','expected_version':definition['version']},error=True)
    second_current=tool('get_todo',{'id':second['id']})
    second_custom=tool('update_todo',{'id':second['id'],'status_id':definition['id'],'expected_version':second_current['version']})
    tool('delete_todo',{'id':second['id'],'expected_version':second_custom['version']})
    tool('delete_status',{'id':definition['id'],'replacement_id':'pending','expected_version':changed['version']})
    after=tool('get_todo',{'id':first['id']})
    assert after['status_id']=='pending' and after['content']=='中文测试\n第二行'
    assert tool('undo_delete',{})['status_id']=='pending'
    print("PASS: HTTP MCP authorization, discovery, CRUD, status definitions, custom status assignment/edit/delete, version conflicts, reorder, durable crash/reopen, undo")
    print("Test directory:", DATA)
finally:
    if process and process.poll() is None:
        process.kill(); process.wait(timeout=10)
