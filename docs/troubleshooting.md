# Troubleshooting

## MCP connection refused

Keep GlassTodo running. Hiding to the tray retains the service; exiting stops it. Check that the client URL matches the generated config and that another process is not using port 57841. Use a separate port/data directory for development. Remote/cloud agents cannot reach another machine's `127.0.0.1` by using this URL unchanged.

## Unauthorized / invalid token

Use the token from the selected data directory, without extra whitespace. Generated client examples contain the matching header. Do not paste the token into issues or logs. If rotating a token, stop the app, remove only the old token file, restart and update authorized client configs. Do not remove the database just to fix authentication.

## Only three states are visible to an agent

There are three built-in **classifications**, but custom states are supported. The server has eleven tools, including `list_statuses`; `update_todo` accepts `status_id`. Reload the MCP connection if the client cached an older schema. The [direct data client](mcp.md#direct-data-client) can inspect current server tools without UI automation.

`inspect` currently reports the SDK's server metadata; its version is not the GlassTodo application version. Check the EXE version, release tag or `Cargo.toml` for that.

## Version conflict

Another UI/agent write changed the item or its status classification. Read the record again, reconcile the intended change and submit its new `expected_version`. Do not blindly replay a create call after an uncertain result; look for the original item first.

## Glass looks different on Windows 10

The Windows 10 fallback paints tint, an opaque text layer and edge highlights. It does not implement live desktop blur/refraction. Increase background遮挡 in settings to improve readability. Windows 11 material availability depends on the OS/DWM path; matching a browser design preview pixel-for-pixel is not a supported guarantee.

## A docked preview stays open

Editors, settings, the status popup, menus, capture or an active drag intentionally retain the preview. Close/save them and move the pointer away. Version 0.1.12 addresses transparent-margin leave events, animation tracking and repeated data refresh delaying collapse. Include edge, DPI, animation setting and exact pointer path in a new bug report if it still occurs.

## Long text / Chinese IME

The native input uses plain text with a matching insertion background. Save before restarting the app. `Shift+Enter` inserts a line; Enter submits only outside composition. On 0.1.11+ long content supports the native vertical scrollbar and wheel input. A minimized reproduction should include fake text, IME name and Windows version, rather than a private todo screenshot.

## Build cannot find a compiler or resource tool

For MSVC, use a Visual Studio Developer PowerShell with the C++ tools and Windows SDK. For GNU, provide matching MinGW-w64 `gcc`/`ar`/`windres`. See [development](development.md); the optional cache script cannot install missing tools.

## Update cannot replace the EXE

Save open drafts and exit the running GUI before replacement, or package to a different `-OutputDirectory`. Do not force-kill a personal editor to unlock a file. The persistent data folder is separate from the EXE; backing it up is described in [usage](usage.md).
