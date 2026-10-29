# Security policy

## Reporting a vulnerability

Use this repository's **Security → Report a vulnerability** feature to submit a private report. Include the affected version, Windows version and a minimal reproduction with synthetic data. Do not put tokens, databases, generated MCP configs or personal todo text in a public issue.

Security fixes are maintained on `main` and included in subsequent releases. This project does not promise a response-time SLA or long-term support for older versions.

## Local trust boundary

- The MCP server binds to `127.0.0.1` and requires a randomly generated bearer token. It is a local personal-data service, not a public network API.
- An authorized MCP client can read and change all todos and status definitions. A model API key is not required by GlassTodo.
- The token, SQLite database and generated client examples live under `%LOCALAPPDATA%\GlassTodo`. Treat all of them as private. They are not encrypted by the application.
- This is not a sandbox against programs running as the same Windows user or an administrator. Protect the Windows account and choose agent clients you trust.
- Do not expose the listener through port forwarding, reverse proxies or public tunnels. Remote/cloud agents need a separately designed access boundary; this project does not implement one.
- UI and MCP share the same version-checked data layer. `expected_version` protects against accidental lost updates; it is not an access-control mechanism.

See [MCP integration](docs/mcp.md) for connection setup and [architecture](docs/architecture.md) for persistence and process boundaries.
