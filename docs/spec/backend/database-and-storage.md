# Database and Storage

## SQLite Access

The app owns one active SQLite pool through `DatabaseState`. Commands call
`db.pool().await` to clone the current pool. This indirection matters because
backup overwrite import and storage directory switching can close and replace
the active pool atomically.

Reference files:
- `src-tauri/src/db/init.rs`
- `src-tauri/src/db/state.rs`
- `src-tauri/src/commands/storage.rs`
- `src-tauri/src/backup/mod.rs` (export/import flow, received-backup slot)
- `src-tauri/src/backup/container.rs`, `archive.rs`, `merge.rs` (container
  format and encryption, zip payload, row merge)
- `src-tauri/src/backup/legacy.rs` (EcoPaste legacy history import)

Do not create a fresh pool in normal commands. Repository functions accept
`&SqlitePool` and return `crate::core::Result<T>`.

## Migrations and Models

Migrations live in `src-tauri/migrations/`. `0001_init.sql` defines
`clipboard_groups`, `clipboard_apps`, `clipboard_items`, and `file_type_icons`.
`0002_fts.sql` defines the FTS5 table and triggers.

The app has shipped a stable release line, so schema changes must be expressed
as new migrations with an explicit upgrade path. When changing schema, update all
of these together:

- SQL migrations.
- `src-tauri/src/db/models.rs`.
- Repository `SELECT`, `INSERT`, `UPDATE`, `bind`, and `query_as` calls.
- TypeScript mirrors in `src/types/`.
- Tests that construct model literals.

`sqlx::query` and `query_as` are the local style. Avoid `query!` macros because
this project does not maintain offline sqlx metadata.

## Timestamp Semantics

All tables have `created_at` and `updated_at` as `TEXT NOT NULL`.

For `clipboard_items`, `updated_at` means the content was reused or deduped.
Do not update it for metadata-only edits:

- `toggle_item_favorite` and `toggle_item_pinned` return state without changing
  `updated_at`.
- `update_item_note` and `update_item_group` preserve recent-use ordering.
- `increment_item_use_count` is the path that refreshes `updated_at`.

This distinction feeds the default `updatedAtDesc` sort used by the clipboard
list.

## Search and Pagination

List search is owned by `db::items::query_items_page`:

- Keywords with 3 or more characters use FTS5 trigram search.
- Keywords with 1 or 2 characters use escaped `LIKE` across `search_text` and
  `note`.
- UI tab filters are represented by `ClipboardGroupFilter` and override explicit
  `kind`/`favorite` fields.
- Pinned items are always sorted before the chosen sort order.
- The query returns `(items, total)`, and commands return `hasMore` as
  `offset + list.len() < total`.

React should not approximate `hasMore` with page size or reimplement short-keyword
search.

## Repository Tests

Use `db::test_support::memory_pool()` for repository tests. It creates a
single-connection in-memory SQLite pool, enables foreign keys, and runs the real
migrations. This catches contracts such as `clipboard_groups` deletion setting
`clipboard_items.group_id` to `NULL`.

Examples:
- `src-tauri/src/db/items.rs`
- `src-tauri/src/db/groups.rs`
- `src-tauri/src/db/apps.rs`

## Resource Paths

`core::paths` is the single entry for persistent roots:

- `app_data_dir` resolves the environment-specific data root.
- `db_dir`, `resources_dir`, `config_dir`, and `state_dir` derive semantic
  subdirectories.
- Dev and release data are separated by `dev/` and `prod/`.
- A bootstrap `storage.json` can point at a custom data root with an
  `.ecopaste-storage.json` identity file.

Do not resolve `app_local_data_dir` independently in feature modules. Leaf file
names remain owned by the modules that write them, such as `settings.json`,
`clipboard.db`, and image/icon stores.

## Storage Switching and Backups

`storage/change_location.rs` moves the data root while the app is running.
`storage/overwrite.rs` replaces live data from a validated backup. Both use
`storage::change` to pause the watcher, stop sync, acquire exclusive storage
access, replace the database pool with recovery, apply required store/settings
changes, release exclusive access, and reconfigure sync. Settings writes defer
their sync effect until that exclusive access is released. The operation mutex
covers preparation through committed cleanup, so two live changes cannot use a
stale root. No backup or move command opens an independent pool.

Relocation copies a closed database into a sibling staging directory, journals
which destination paths it touches, and restores only those paths on failure.
The bootstrap `storage.json` is never swapped. Post-commit cleanup removes only
the migrated `db`, `resources`, `config`, `state`, and storage identity paths;
unrecognized files remain in the old root. Backup overwrite checks the
incoming SQLite database and settings before changing live data, then retains
the previous database and resources until required finalization succeeds. A
failed install or finalization attempts to reopen the previous database. If
reopening fails, recovery files are retained and the returned error includes
their path. Cleanup failure after a committed relocation is logged rather than
reported as a failed move. Multi-file storage changes are not an atomic
filesystem transaction; tests exercise the rollback paths with temporary data.

## Managed Resource Lifetime

`StorageAccess` coordinates resource files with their SQLite references.
Clipboard capture, screenshot output, sync receive, legacy import, app icon
materialization, and file icon enrichment hold a read permit from before file
creation until the corresponding database write completes. Backup merge holds
one read permit while copying missing resources before merging rows; a copy
failure cannot leave newly merged rows pointing to absent files. The list preview's
background thumbnail generation acquires its own permit after the list returns.

Cache cleanup takes an exclusive permit **before** reading the current pool,
resource root, and referenced filenames, then keeps it through the last file
deletion. The cached running-app icon names are also live roots because the
running-app preview can store an icon without creating a database row. History
deletion and retention cleanup hold an exclusive permit from row removal through
image file removal, preventing a simultaneous same-digest insert from losing
its file. Acquire permits before `DatabaseState::pool()` and path resolution.
Never acquire a second read permit while holding the first: Tokio's writer-
preferring lock can block the nested read when an exclusive operation is queued.
Normal settings writes acquire the storage operation mutex before a read permit
so sync reconfiguration cannot overlap a live root change. Data replacement
uses its dedicated storage-swap settings path while holding exclusive access.
Cache cleanup can briefly delay new clipboard captures while it scans files.
Copy, paste, and drag-out hold a read permit while resolving managed image
paths. An image save releases its permit while the user chooses a destination,
then re-reads the selected item and source path under a new permit before
copying. On macOS, file drag-out retains the permit until the native drag-end
callback, since starting the session returns before the user drops the file.

`backup/mod.rs` exports `.ecopastebak` packages in two modes:

- Plain mode: ZIP payload (`backup/archive.rs`).
- Encrypted mode: EcoPaste container with Argon2id and XChaCha20Poly1305
  (`backup/container.rs`).

The backup receive flow uses a pending slot when the preference window has been
destroyed, mirroring the preference-highlight pending-slot pattern in
`window/mod.rs`. Do not emit directly to a possibly rebuilding preference
window.
