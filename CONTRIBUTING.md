# Contributing to GlassTodo

Small changes that preserve the native application, durable local data and low idle resource use are welcome. Please open an issue before adding a new framework, network service or large dependency.

## Development loop

1. Follow [development setup](docs/development.md). Use a separate data directory, a different port and `--no-autostart` for manual testing.
2. Keep business operations in `Core` / `Store`. Both the UI and MCP must use those operations; an agent should never need to click the UI to change data.
3. Add a focused regression for behavior changes. Native window tests must use private synthetic data and avoid activating windows or injecting global input.
4. Run:

   ```powershell
   cargo fmt --all -- --check
   cargo clippy --locked --all-targets -- -D warnings
   cargo test --locked
   cargo build --release --locked
   python scripts/smoke_mcp.py
   python scripts/test_mcp_data_client.py
   ```

5. Describe the visible change, why it is needed and the validation performed. Include synthetic screenshots for visual changes, and resource measurements when changing rendering, timers or background services.

## Code conventions

- Format Rust with `rustfmt`; keep `Cargo.lock` committed for reproducible application builds.
- Keep Win32 / Direct2D objects on their owning UI thread. Review reentrant messages, raw-pointer lifetimes, HWND destruction, timers and capture ownership when editing `ui.rs`.
- Stop animation and delayed-action timers when their work finishes. Use event notifications instead of idle polling.
- Preserve optimistic version checks and atomic SQLite transactions. Status changes must not discard multi-line content or break undo for deleted records.
- Existing MCP names, built-in classification values and payload fields are compatibility boundaries. Add optional fields when possible; document any change in the changelog and MCP reference.
- Never commit generated client configs, bearer tokens, databases, real todo content, personal screenshots or `.scratch` files. Examples must use placeholders and synthetic records.

## Pull requests and reports

Use the issue and PR templates. For native UI bugs, report Windows version, DPI, docking edge, theme, animation setting and reproducible steps. Use [private reporting](SECURITY.md) for security issues.

## Releases

1. Update `Cargo.toml`, the root package in `Cargo.lock`, `assets/app.rc` and `assets/app.manifest` together. Add a changelog entry.
2. Complete the checks above and wait for Windows CI. Windows 10 / Windows 11 material changes need a real OS check in addition to off-screen tests.
3. Package into a fresh output directory:

   ```powershell
   .\scripts\build.ps1 -UseCurrentToolchain -OutputDirectory artifacts/release
   ```

4. Review the ZIP contents and SHA-256 file. Tag the exact tested commit, then upload the ZIP and checksum to a GitHub Release. Keep credentials and local test data outside the archive.
5. Record the compiler target and tested OS. Resource samples are measurements, not a promise for every machine or input method.

Contributions are provided under the project's [MIT license](LICENSE).
