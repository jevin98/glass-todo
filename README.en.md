# GlassTodo

[简体中文](README.md) · [Windows downloads](https://github.com/jevin98/glass-todo/releases) · [Development](docs/development.md) · [MCP reference](docs/mcp.md)

A small native Windows floating todo app with a built-in local MCP data server. Pin it anywhere, dock it to a screen edge, and let agents manage the same durable todos through MCP.

Built with Rust, Win32, Direct2D / DirectWrite and SQLite. No WebView, Electron, Node.js runtime or model API key is needed.

<img src="docs/images/note-light.png" width="316" alt="Native note renderer with synthetic sample data" /> <img src="docs/images/status-picker.png" width="244" alt="Status picker; custom release states are examples" />

The UI is currently in Chinese. Previews use synthetic data. Custom statuses shown in the picker are examples, not preinstalled defaults.

## Features

- Draggable, pinnable and resizable note, compact bar and four-edge docking with hover previews.
- Multi-line text, blank-line-separated batch creation, manual ordering, soft deletion and durable undo.
- Pending / in-progress / done classifications, plus custom names, six colors and classification mapping.
- Saved appearance settings: system/light/dark theme, background opacity, text size and corner radius.
- Native Rich Edit input with IME handling and long-text scrolling.
- Persisted todos and window state, optional Windows sign-in autostart and a system tray entry.
- Eleven version-aware MCP data tools sharing the exact same SQLite-backed business layer as the UI.

## Install and use

Download the `windows-x64.zip` from [Releases](https://github.com/jevin98/glass-todo/releases), extract it to a stable location, and run `GlassTodo.exe`. First normal startup enables current-user sign-in autostart; disable it in the context menu if desired.

Data lives in `%LOCALAPPDATA%\GlassTodo`, outside the executable folder. Double-click a todo to edit. `Shift+Enter` inserts a line; `Enter` saves; `Esc` cancels. Separate batch items with a blank line. Use the gear for appearance/status settings.

Windows 10 / Windows 11 x64 are the intended platforms. Windows 10 LTSC 19044 at 150% DPI has been tested. The Windows 10 material is a tinted/rim-lit fallback, not live desktop blur. The optional Windows 11 Acrylic path still needs real Windows 11 validation.

## Size and memory

The tested 0.1.12 GNU release executable is about **2.91 MB**. A hidden native dock-icon sample with 100 synthetic todos and one active MCP session used about **4.61 MiB private working set**, with **0.000% idle CPU** during an approximately five-second sample on Windows 10.

These figures exclude opening an editor/settings panel and IME overhead, and are not universal limits. Compiler targets, fonts, OS and interaction state affect the result. See [verification](docs/verification.md).

## Build

Use a Visual Studio Developer PowerShell with the C++ desktop build tools, Windows SDK and Rust stable:

```powershell
git clone https://github.com/jevin98/glass-todo.git
cd glass-todo
cargo test --locked
.\scripts\build.ps1 -UseCurrentToolchain
```

The portable ZIP and SHA-256 file are created under `dist/`. Python 3.11+ is only needed for integration tests and developer utilities. No Python runtime is required by the app. See [development](docs/development.md) for GNU builds and isolated test instances.

## Connect an agent

Run the app once, then open the generated examples under `%LOCALAPPDATA%\GlassTodo\mcp-configs`. Merge the relevant entry into Codex, Claude Code or Cursor, and reload that client's MCP connection. The generated examples contain a real local bearer token: keep them private.

The default endpoint is `http://127.0.0.1:57841/mcp`. MCP tools operate directly on persisted application data, including while the note is hidden. They do not click the UI. The server stops when the app exits; `--headless` can run the same data service without a UI.

For a custom status, call `list_statuses`, then `update_todo` with that `status_id` and the latest `expected_version`. The three `status` enum values are compatibility classifications, not a limit on custom statuses. Read again after a successful write; re-read on a version conflict.

See the [MCP reference](docs/mcp.md) for all eleven tools, JSON examples, client config templates and a standard-library HTTP client for stale tool catalogs.

## Understand and contribute

- [Architecture](docs/architecture.md): modules, process/thread boundaries, rendering and notifications.
- [Data model](docs/data-model.md): schema, migrations, optimistic concurrency, undo and status reassignment.
- [Development](docs/development.md): checks, previews, packaging and Windows CI.
- [Troubleshooting](docs/troubleshooting.md) and [verification](docs/verification.md).
- [Contributing](CONTRIBUTING.md), [security](SECURITY.md), [changelog](CHANGELOG.md).

Cloud sync, multi-user access, remote networking, an embedded AI model and automatic update downloads are outside the current scope.

Licensed under [MIT](LICENSE).

See [third-party notices](third-party/README.md) for the locked dependency inventory and upstream license texts.
