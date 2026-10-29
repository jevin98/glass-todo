# MCP integration and tool reference

GlassTodo exposes **data operations**, not desktop-control tools. It uses Streamable HTTP on `http://127.0.0.1:57841/mcp` by default, in the same process as the application. Every request needs the local bearer token. No OAuth service, cloud account or model API key is involved.

## Connect a client

1. Start GlassTodo. For an isolated developer instance, use another `--data-dir`, `--port` and `--no-autostart`.
2. Open `%LOCALAPPDATA%\GlassTodo\mcp-configs` (or `mcp-configs` under the selected data directory). The app generates `codex.toml`, `claude-code.json` and `cursor.json` with the matching URL and token.
3. Merge the relevant server entry into the client's configuration. Do not replace unrelated entries. Reload that client's MCP connection.
4. Ask the agent to list todos and status definitions before requesting a write.

The safe [templates](../examples/mcp) contain `YOUR_LOCAL_TOKEN` placeholders. The app-generated versions contain a real secret and must stay private.

### Codex

Merge the generated `[mcp_servers.glass_todo]` section into the user's `~/.codex/config.toml`, or configure a Streamable HTTP server in MCP settings. Reload/Restart the connection after schema changes. This project does not modify global agent rules or register itself automatically in a client.

```toml
[mcp_servers.glass_todo]
url = "http://127.0.0.1:57841/mcp"
http_headers = { Authorization = "Bearer YOUR_LOCAL_TOKEN" }
```

### Claude Code

Use the generated HTTP server entry in the desired scope. The wrapper is `mcpServers`; this server's entry uses `type: "http"`, `url` and `headers.Authorization`. For project scope, `.mcp.json` belongs to the project, but a file containing your real token must remain untracked. Use `/mcp` to check connection state.

### Cursor

Merge the generated `mcpServers.glass_todo` entry into the user's or project's MCP configuration. It uses `url` and `headers.Authorization`. A project `.cursor/mcp.json` with a real token must remain untracked.

References: [Codex MCP](https://learn.chatgpt.com/docs/extend/mcp?surface=cli), [Claude Code MCP](https://code.claude.com/docs/en/mcp), [Cursor MCP](https://cursor.com/docs/mcp). Standard old/new protocol flows are tested; every client's UI/remote-host combination has not been independently exercised.

## Tool contract (0.1.12)

| Tool | Arguments | Result / effect |
| --- | --- | --- |
| `list_todos` | Optional `status`, `status_id` | Non-deleted todos in manual order; optional classification/definition filter |
| `get_todo` | `id` | One non-deleted todo including current version |
| `create_todo` | `content` | One pending todo; newlines stay inside its body |
| `update_todo` | `id`; optional `content`, `status`, `status_id`, `expected_version` | Change content and/or assignment; at least one change field required |
| `delete_todo` | `id`; optional `expected_version` | Soft-delete; recoverable through undo |
| `undo_delete` | Empty object | Restore the most recently deleted todo |
| `reorder_todos` | `ids` | All active IDs exactly once, in the requested order |
| `list_statuses` | Empty object | Built-in and custom definitions with IDs and versions |
| `create_status` | `name`, `kind`, `color` | Create a named custom definition |
| `update_status` | `id`, `expected_version`; optional `name`, `kind`, `color` | Edit a custom definition |
| `delete_status` | `id`, `replacement_id`, `expected_version` | Atomically reassign references, then remove the custom definition |

`status` / `kind` values are `pending`, `in_progress`, `done`. Colors are `gray`, `blue`, `amber`, `green`, `purple`, `rose`. Names are 1–10 Unicode scalar values, and at most 24 definitions exist including the three fixed built-ins. See [data model](data-model.md) for exact persistence and undo semantics.

The tools return a JSON-serialized result in a text content block. Business failures set `isError`; protocol/authentication failures may instead be HTTP or JSON-RPC errors. A write succeeds only after SQLite commits.

## Safe update sequence

1. Read the target record. Keep its current `version`.
2. Discover custom definitions using `list_statuses`; use the returned ID, not a hard-coded assumed number.
3. Call `update_todo` with `status_id` and `expected_version`. Omit `content` for a status-only change. Do not provide both `status` and `status_id`.
4. Re-read the item after success. On conflict, re-read and reconcile; do not overwrite blindly.

Illustrative arguments (replace both ID and version with a fresh read):

```json
{
  "id": 42,
  "status_id": "custom_1",
  "expected_version": 4
}
```

The returned todo includes `id`, `content`, `status`, `position`, `version`, `status_id`, `status_name`, `status_color`. `status` is a compatibility classification. For example, a custom “发布测试” definition can have `kind: "in_progress"` while retaining its own ID, name and purple badge.

Prefer versions on legacy todo update/delete even though their fields remain optional for backward compatibility. Changing/deleting a status classification can also increment affected todo versions. Re-query after these operations.

`create_todo` creates one record even when its body contains blank lines. The UI's batch-creation behavior is a separate shared-core operation, not a hidden interpretation of the MCP create tool. Search existing records before retrying an uncertain create call to avoid duplicates.

## Direct data client

If an agent cached an older tool catalog, `scripts/glass_todo_mcp.py` calls the **same HTTP MCP** with current arguments. Run from a source checkout using Python 3.11+:

```powershell
python scripts/glass_todo_mcp.py inspect
python scripts/glass_todo_mcp.py tools
python scripts/glass_todo_mcp.py call list_statuses
python scripts/glass_todo_mcp.py call update_todo --arguments-file .scratch/update-arguments.json
```

The argument file is a UTF-8 JSON object such as the example above. `--data-dir` and `--url` may precede the subcommand for an isolated local instance. The default URL is read from the app-generated config, and the token is read from the data directory. Only local HTTP `/mcp` URLs are accepted. The helper closes its own session and never automatically retries a write.

This helper neither manipulates the UI nor writes SQLite directly. A missing cached tool does not imply that a custom state is unsupported. Reload the agent connection or inspect current server schemas; do not substitute GUI clicking for a requested data operation.

## Availability and access

Hidden-to-tray operation keeps MCP available. Exiting stops it. `--headless` exposes the same data operations without creating a UI; supervise that process or use `--exit-after` for bounded automation. Do not start another process against an occupied port or casually share a data directory between instances.

The endpoint is local to the machine running GlassTodo. A remote SSH/cloud agent cannot use its own loopback address to reach your Windows host. Public exposure and multi-user authorization are outside the current design; see [security](../SECURITY.md).
