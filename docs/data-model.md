# Data model and persistence

The database is `todos.sqlite3` in the selected data directory. Schema version is currently **2**, stored in `PRAGMA user_version`. SQLite uses a rollback `DELETE` journal, `synchronous=EXTRA`, foreign keys and a three-second busy timeout. One connection is guarded by `Core`'s mutex.

## Tables

### `todos`

| Column | Meaning |
| --- | --- |
| `id` | Stable auto-incrementing integer ID |
| `content` | Trimmed plain text, nonempty, at most 16,384 Unicode scalar values |
| `status` | Compatibility classification: `pending`, `in_progress`, `done` |
| `position` | Manual ordering value |
| `version` | Optimistic concurrency version, starting at 1 |
| `deleted_at` | Nullable soft-deletion ordering marker; deleted records stay in the database |
| `custom_status_id` | Nullable foreign key to a custom definition |

The API returns joined `status_id`, `status_name` and `status_color` fields. A built-in status uses its classification string as its `status_id`; a custom definition uses `custom_<integer>`. Callers must discover IDs, not assume that a name maps to a particular number.

### `custom_statuses`

`id`, `name`, `kind`, `color`, `version`. `kind` is one of the three compatibility classifications. Colors are `gray`, `blue`, `amber`, `green`, `purple`, `rose`. Names are trimmed, unique under application validation, 1–10 Unicode scalar values and cannot contain control characters. There are at most 24 total definitions, including three fixed built-ins.

Renaming/recoloring a definition is reflected through joined metadata. Changing its classification updates affected todos and increments their versions, including soft-deleted rows. Deleting a custom status atomically moves all references to a valid replacement, then removes the definition. Its numeric ID is not reused.

### `settings`

Key/value JSON records. `window` contains position, full size, collapsed/pinned/autostart flags, completed-section state and optional docking anchor. `appearance` contains theme, background opacity, text size and radius. Older/missing fields deserialize to defaults and values are normalized to supported ranges.

## Concurrency

Read a todo, then pass its `version` as `expected_version` on update/delete. A stale version is rejected instead of overwriting another client. Re-read after a conflict and reconcile the intended change. Expected versions are optional on legacy todo endpoints for compatibility; integrations should always supply them. Custom status update/delete require a version through MCP.

`status` and `status_id` are mutually exclusive when assigning a status. A content-only update retains the custom status. Explicitly assigning a built-in `status` clears the custom reference. This is why clients should use `status_id` to select a named custom workflow state.

## Atomic operations

- Blank-line-separated creation validates the entire batch before committing it. Ordinary line breaks stay inside one todo; editing an existing record never splits it.
- Manual reorder requires all active todo IDs exactly once; invalid orders do not partially apply. Completed rows keep their own order.
- Undo restores the most recently deleted row with its text, current status reference and original position. There is no public permanent-purge tool in this version.
- Status classification changes and status deletion/reassignment are transactions, including references held by deleted rows.

## Migration and backup

Version 2 adds the custom-status table and a nullable reference without rewriting legacy content, IDs, positions or versions. Migration is transactional. A fresh database still starts with only the three built-in definitions.

For a portable backup, save open drafts, stop the application and copy the data directory. The token and generated configs in that folder are also secrets. Prefer a SQLite backup API for a live-database backup; copying a live database file alone is not a documented backup procedure. Do not edit production rows directly: use the MCP tools or the shared Rust operations so version and undo rules remain intact.
