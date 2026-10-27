//! Merges a backup database into the live one, row by row, skipping what
//! already exists.

use std::path::Path;

use anyhow::Context;
use chrono::{DateTime, Utc};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::{Sqlite, SqlitePool};

use crate::core::Result;

pub(super) struct MergeOutcome {
    pub(super) imported_items: u64,
    pub(super) skipped_items: u64,
}

pub(super) async fn open_backup_db(path: &Path) -> Result<SqlitePool> {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .read_only(true)
        .journal_mode(SqliteJournalMode::Wal)
        .foreign_keys(true);

    Ok(SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .with_context(|| format!("failed to open backup database at {path:?}"))?)
}

pub(super) async fn merge_history(
    current: &SqlitePool,
    backup: &SqlitePool,
) -> Result<MergeOutcome> {
    let mut tx = current.begin().await.context("failed to begin import")?;
    merge_groups(&mut tx, backup).await?;
    merge_apps(&mut tx, backup).await?;
    merge_file_type_icons(&mut tx, backup).await?;
    let outcome = merge_items(&mut tx, backup).await?;
    tx.commit().await.context("failed to commit import")?;

    Ok(outcome)
}

async fn merge_groups(tx: &mut sqlx::Transaction<'_, Sqlite>, backup: &SqlitePool) -> Result<()> {
    let rows = sqlx::query_as::<
        _,
        (
            String,
            String,
            String,
            bool,
            i64,
            DateTime<Utc>,
            DateTime<Utc>,
        ),
    >(
        "SELECT id, name, icon, is_hidden, sort_order, created_at, updated_at FROM clipboard_groups",
    )
    .fetch_all(backup)
    .await
    .context("failed to read backup groups")?;

    for row in rows {
        sqlx::query(
            "INSERT OR IGNORE INTO clipboard_groups \
             (id, name, icon, is_hidden, sort_order, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(row.0)
        .bind(row.1)
        .bind(row.2)
        .bind(row.3)
        .bind(row.4)
        .bind(row.5)
        .bind(row.6)
        .execute(&mut **tx)
        .await
        .context("failed to import group")?;
    }

    Ok(())
}

async fn merge_apps(tx: &mut sqlx::Transaction<'_, Sqlite>, backup: &SqlitePool) -> Result<()> {
    let rows = sqlx::query_as::<
        _,
        (
            String,
            String,
            Option<String>,
            String,
            DateTime<Utc>,
            DateTime<Utc>,
        ),
    >(
        "SELECT id, name, icon_file, platform, created_at, updated_at FROM clipboard_apps"
    )
    .fetch_all(backup)
    .await
    .context("failed to read backup apps")?;

    for row in rows {
        sqlx::query(
            "INSERT OR IGNORE INTO clipboard_apps \
             (id, name, icon_file, platform, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(row.0)
        .bind(row.1)
        .bind(row.2)
        .bind(row.3)
        .bind(row.4)
        .bind(row.5)
        .execute(&mut **tx)
        .await
        .context("failed to import app")?;
    }

    Ok(())
}

async fn merge_file_type_icons(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    backup: &SqlitePool,
) -> Result<()> {
    let rows = sqlx::query_as::<_, (String, String, String, DateTime<Utc>, DateTime<Utc>)>(
        "SELECT cache_key, platform, icon_file, created_at, updated_at FROM file_type_icons",
    )
    .fetch_all(backup)
    .await
    .context("failed to read backup file type icons")?;

    for row in rows {
        sqlx::query(
            "INSERT OR IGNORE INTO file_type_icons \
             (cache_key, platform, icon_file, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(row.0)
        .bind(row.1)
        .bind(row.2)
        .bind(row.3)
        .bind(row.4)
        .execute(&mut **tx)
        .await
        .context("failed to import file type icon")?;
    }

    Ok(())
}

async fn merge_items(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    backup: &SqlitePool,
) -> Result<MergeOutcome> {
    let rows = sqlx::query_as::<_, BackupItemRow>(
        "SELECT id, kind, sub_kind, group_id, source_app_id, content, content_hash, search_text, \
         summary, file_types, size, width, height, use_count, is_favorite, is_pinned, is_sensitive, platform, note, \
         created_at, updated_at FROM clipboard_items ORDER BY created_at ASC",
    )
    .fetch_all(backup)
    .await
    .context("failed to read backup items")?;

    let mut imported_items = 0;
    let mut skipped_items = 0;
    for row in rows {
        let exists: Option<i64> = sqlx::query_scalar(
            "SELECT 1 FROM clipboard_items WHERE kind = ? AND content_hash = ? LIMIT 1",
        )
        .bind(row.kind.as_str())
        .bind(row.content_hash.as_str())
        .fetch_optional(&mut **tx)
        .await
        .context("failed to check duplicate item")?;
        if exists.is_some() {
            skipped_items += 1;
            continue;
        }

        sqlx::query(
            "INSERT OR IGNORE INTO clipboard_items \
             (id, kind, sub_kind, group_id, source_app_id, content, content_hash, search_text, \
              summary, file_types, size, width, height, use_count, is_favorite, is_pinned, is_sensitive, platform, note, \
              created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(row.id)
        .bind(row.kind)
        .bind(row.sub_kind)
        .bind(row.group_id)
        .bind(row.source_app_id)
        .bind(row.content)
        .bind(row.content_hash)
        .bind(row.search_text)
        .bind(row.summary)
        .bind(row.file_types)
        .bind(row.size)
        .bind(row.width)
        .bind(row.height)
        .bind(row.use_count)
        .bind(row.is_favorite)
        .bind(row.is_pinned)
        .bind(row.is_sensitive)
        .bind(row.platform)
        .bind(row.note)
        .bind(row.created_at)
        .bind(row.updated_at)
        .execute(&mut **tx)
        .await
        .context("failed to import item")?;
        imported_items += 1;
    }

    Ok(MergeOutcome {
        imported_items,
        skipped_items,
    })
}

#[derive(sqlx::FromRow)]
struct BackupItemRow {
    id: String,
    kind: String,
    sub_kind: Option<String>,
    group_id: Option<String>,
    source_app_id: Option<String>,
    content: String,
    content_hash: String,
    search_text: Option<String>,
    summary: Option<String>,
    file_types: Option<String>,
    size: Option<i64>,
    width: Option<i64>,
    height: Option<i64>,
    use_count: i64,
    is_favorite: bool,
    is_pinned: bool,
    is_sensitive: bool,
    platform: String,
    note: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn merge_preserves_is_sensitive_flag() {
        use crate::db::items::{content_hash, insert_item};
        use crate::db::models::{ClipboardItem, ClipboardKind, Platform};
        use crate::db::test_support::memory_pool;
        use chrono::DateTime;

        // The backup pool stands in for a real backup database, which is a
        // snapshot of the live clipboard.db and therefore already carries the
        // `is_sensitive` flag written by the production insert path.
        let backup = memory_pool().await;
        let current = memory_pool().await;

        let ts = DateTime::from_timestamp(1_700_000_000, 0).unwrap();
        let content = "AKIAIOSFODNN7EXAMPLE".to_owned();
        let item = ClipboardItem {
            id: "secret-1".to_owned(),
            kind: ClipboardKind::Text,
            sub_kind: None,
            group_id: None,
            source_app_id: None,
            content_hash: content_hash(ClipboardKind::Text, &content),
            content,
            search_text: None,
            summary: None,
            file_types: None,
            size: None,
            width: None,
            height: None,
            use_count: 1,
            is_favorite: false,
            is_pinned: false,
            is_sensitive: true,
            platform: Platform::Macos,
            note: None,
            created_at: ts,
            updated_at: ts,
            source_app_name: None,
            source_app_icon_file: None,
            source_app_icon_path: None,
            image_thumbnail_path: None,
            file_entries: None,
            files_preview_kind: None,
            available_actions: Vec::new(),
            color_preview: None,
            display_created_at: String::new(),
        };
        insert_item(&backup, &item).await.unwrap();

        let outcome = merge_history(&current, &backup).await.unwrap();
        assert_eq!(
            outcome.imported_items, 1,
            "sensitive item should be imported"
        );

        // Regression: the merge SELECT/INSERT used to omit `is_sensitive`, so
        // every imported row silently fell back to the column DEFAULT (0) and
        // secrets were rendered in plaintext. The flag must survive the merge.
        let imported: bool =
            sqlx::query_scalar("SELECT is_sensitive FROM clipboard_items WHERE id = ?")
                .bind("secret-1")
                .fetch_one(&current)
                .await
                .unwrap();
        assert!(
            imported,
            "merged sensitive item must keep is_sensitive = true"
        );
    }
}
