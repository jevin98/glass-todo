# Verification and known limits

## Recorded baseline

The development baseline was measured on **2026-10-27** using Windows 10 Enterprise LTSC 19044 x64, 144 DPI (150%), Rust 1.99.1 and `x86_64-pc-windows-gnu`. Release uses size optimization, LTO, one codegen unit, stripping and panic abort.

Version 0.1.12 passed **51 Rust tests**, Clippy on all targets, real HTTP MCP smoke tests and the direct-data-client integration test. Protocol flows `2025-03-26` and `2026-07-28` have been exercised. CI runs a separate Windows MSVC build; its result and artifacts are shown in GitHub Actions rather than inferred from the local GNU sample.

## Resource samples

Samples use the complete normal native-window process kept hidden, 100 synthetic todos, one active MCP session, a six-second settling period and approximately five seconds of idle CPU sampling. No personal data or desktop input is used.

| Metric | Normal note (0.1.10) | Right dock icon (0.1.12) |
| --- | ---: | ---: |
| Private working set | 13.53 MiB | 4.61 MiB |
| Working set | 30.62 MiB | 18.09 MiB |
| Private commit | 22.80 MiB | 6.95 MiB |
| Idle CPU, single-core basis | 0.000% | 0.000% |

These columns are different recorded versions, not a controlled same-build comparison. The docked sample excludes first opening Rich Edit, settings and the input method. The measured GNU 0.1.12 EXE was **2,908,160 bytes** (about 2.91 MB). MSVC and future releases can differ.

Goals: ZIP ≤5 MB, executable ≤10 MB, and idle private working set preferably ≤40 MiB for a representative 100-item workload. These are targets, not hard limits for all systems, fonts and interactions.

## Reproduce

```powershell
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo build --release --locked
python scripts/smoke_mcp.py
$env:MCP_TEST_PROTOCOL = '2026-07-28'
python scripts/smoke_mcp.py
Remove-Item Env:MCP_TEST_PROTOCOL
python scripts/test_mcp_data_client.py
$env:GLASS_TODO_SETTLE_SECONDS = '6'
$env:GLASS_TODO_TEST_DOCK_EDGE = 'right'
python scripts/measure.py
Remove-Item Env:GLASS_TODO_TEST_DOCK_EDGE
```

Developer outputs stay under `.scratch/`, which is excluded from Git. They can contain private test credentials even when the todo text is synthetic. Do not upload that directory. Public PNG previews are selected renderer outputs from synthetic fixtures.

## What the tests cover

- Durable multi-line text, blank-line batch creation and atomic validation.
- Optimistic version conflicts, manual reorder, soft deletion and undo across reopen.
- Legacy schema migration, custom status validation, classification changes and reference-safe deletion/reassignment including soft-deleted rows.
- Direct MCP authorization, discovery, CRUD, status definitions, conflict rejection, crash/reopen durability and session lifecycle.
- Off-screen native drawing, alpha, hit regions, badge centering and multi-line row alignment.
- Native empty/focused/typed/composing input behavior, matching insertion background and long-text wheel/scrollbar behavior.
- Four-edge geometry and DPI, fresh docking hover gating, animation reversal and timer settlement, transparent-margin leave, first-deadline preservation and stale/canceled timer handling.
- One settings host, page reuse, draft preservation, theme synchronization and durable appearance.

## Checks still requiring real use

- Windows 11 Acrylic is implemented as a capability-dependent path, but has not been validated on a real Windows 11 machine in this baseline.
- Actual Chinese candidate selection, multi-monitor transitions, tray behavior, hover smoothness and accessibility settings need physical interaction checks in addition to native/off-screen regressions.
- The three client configuration formats are supplied and protocol behavior is tested. Not every Codex/Claude Code/Cursor version or remote executor combination has been exercised.
- There is no signed installer, automatic update service, encrypted database, public network service or cloud synchronization.

For reproducible setup and packaging commands see [development](development.md); for a report use [contributing](../CONTRIBUTING.md).
