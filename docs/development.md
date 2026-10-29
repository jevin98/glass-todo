# Development and validation

## Prerequisites

- Windows x64. Windows 10 / 11 are the intended desktop systems.
- Rust stable with `rustfmt` and `clippy`. `Cargo.toml` declares Rust 1.89; local validation used 1.99.1. The declared minimum has not been separately tested as an MSRV matrix.
- A C compiler and Windows resource compiler. SQLite is compiled from the bundled crate source.
- Python **3.11+** for the standard-library developer scripts, not for the application itself.

Keep `Cargo.lock`; build and test with `--locked`.

## MSVC (recommended for a regular Windows setup)

Install Visual Studio Build Tools with **Desktop development with C++** and a Windows SDK. Open the matching x64 Developer PowerShell so `cl.exe`, `link.exe` and `rc.exe` are on PATH. Use the `x86_64-pc-windows-msvc` Rust host/target.

```powershell
rustup component add rustfmt clippy
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --release --locked
.\scripts\build.ps1 -SkipTests -UseCurrentToolchain
```

An ordinary PowerShell without the SDK environment may fail to find `rc.exe` even when the SDK is installed. `build.rs` embeds the manifest/icon using `rc.exe` for MSVC and `windres` for GNU.

## GNU toolchain

Use a matching `x86_64-pc-windows-gnu` Rust toolchain with MinGW-w64 `gcc`, `ar` and `windres` available. Configure the linker and C compiler for that target; a GNU linker cannot consume an arbitrary MSVC environment.

The optional `scripts/dev-env.ps1` is for an isolated local layout under a toolchain root: `cargo/`, `rustup/`, `mingw/w64devkit/bin/`. It accepts `-ToolchainRoot`; its default is `%USERPROFILE%\.cache\glass-todo-build`. It locates an existing toolchain, not an installer. Normal contributors can use their existing MSVC/GNU environment instead.

## Safe manual development

Use a different port and data directory so you do not touch personal todos or startup settings:

```powershell
.\target\debug\glass-todo.exe --data-dir .scratch/manual --port 57842 --no-autostart
```

The same port identifies an existing GUI instance; launching another GUI process on that port activates it instead of creating a second note. `--headless` is a data-only process and still needs an unused port. Never point two development instances at production data.

## Checks

| Command | Coverage |
| --- | --- |
| `cargo test --locked` | Store/migrations, concurrency, native rendering, input, docking and settings regressions |
| `cargo clippy --locked --all-targets -- -D warnings` | Rust lint checks, including tests |
| `python scripts/smoke_mcp.py` | Real release HTTP server, authorization, CRUD, statuses, conflicts, order, undo and crash/reopen |
| `python scripts/test_mcp_data_client.py` | Direct client with a private headless server, JSON arguments, custom statuses and conflict handling |

The scripts default to `target/release/glass-todo.exe`. The smoke and measurement scripts accept an EXE path as their first argument. Smoke tests keep their own random-port data under `.scratch/` and deliberately terminate only their own private test instance.

Test the newer protocol flow too:

```powershell
$env:MCP_TEST_PROTOCOL = '2026-07-28'
python scripts/smoke_mcp.py
Remove-Item Env:MCP_TEST_PROTOCOL
```

## Rendering previews

Tests emit `.bgra` snapshots using synthetic records. Run `python scripts/render_previews.py` after `cargo test --locked` to export the selected public PNG previews. These are renderer images, not captures of personal desktop windows.

## Resource measurements

```powershell
$env:GLASS_TODO_SETTLE_SECONDS = '6'
python scripts/measure.py
$env:GLASS_TODO_TEST_DOCK_EDGE = 'right'
python scripts/measure.py
Remove-Item Env:GLASS_TODO_TEST_DOCK_EDGE
```

The normal native-window path stays hidden, seeds 100 private records and keeps one MCP session active. It measures process working set/private commit and about five seconds of idle CPU, then verifies timed exit. It does not measure editor/IME overhead, interactive animation quality or every hardware configuration. Remove the optional settle environment variable when finished.

## Packaging and CI

`scripts/build.ps1` runs tests unless `-SkipTests` is given, builds Release, and packages the EXE, licenses and documentation with a SHA-256 file. `-UseCurrentToolchain` preserves the shell's toolchain instead of selecting the optional local cache. `-OutputDirectory artifacts/release` avoids replacing an EXE that is currently in use.

Windows CI checks formatting, Clippy, unit/native tests, Release, both MCP protocol flows, the direct client and portable packaging. It uses the MSVC Developer environment and uploads the ZIP/checksum as a build artifact. The local GNU resource sample in [verification](verification.md) is a different compiler target; do not present its numbers as measurements of the CI build.

For releases, follow [CONTRIBUTING](../CONTRIBUTING.md). There is no automatic application updater or hidden credential provisioning step.

When updating dependencies or changing the release toolchain, run `python scripts/collect_licenses.py` and review [third-party notices](../third-party/README.md). Packaging includes those committed texts without requiring Python; CI regenerates them for its own build.
