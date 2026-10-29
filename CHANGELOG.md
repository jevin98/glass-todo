# Changelog

## 0.1.12

First public source release. Includes the native floating note, durable SQLite data, appearance/status settings, eleven direct MCP data tools and the features from earlier development builds below.

- Fix dock previews not collapsing after a leave in transparent padding.
- Restore client/nonclient leave tracking after expansion animations.
- Preserve the first collapse deadline through continuous data refreshes.
- Ignore canceled/old timer messages when a newer hover deadline is active.
- Add public documentation, safe configuration templates, MIT license, contribution/security guides and Windows CI.

## 0.1.11

- Add native vertical scrollbars and explicit wheel routing for long editor content.
- Accumulate high-resolution wheel input and respect system line/page scrolling settings.
- Preserve text and native IME behavior while scrolling.

## 0.1.10

- Replace the system status menu with a rounded, colored native picker.
- Unify appearance and custom status management in one settings window.
- Preserve status-name drafts across tabs and theme changes.

## 0.1.9

- Add durable custom status definitions, colors and classification mapping.
- Add status-definition MCP CRUD and `status_id` assignment/filtering.
- Center status text and vertically align badges on multi-line rows.
- Migrate legacy data transactionally to schema version 2.

## Earlier development builds

- Native Rust/Win32 note, software Direct2D/DirectWrite rendering and local HTTP MCP.
- Persistent window state, optional sign-in autostart, tray and compact mode.
- Plain-text Rich Edit input, IME-aware submit, blank-line batch creation and scrollable lists.
- Resizing, four-edge docking, hover gating, finite spring animations, soft deletion and durable undo.
