use anyhow::Context;
use chrono::Utc;
use sqlx::{QueryBuilder, Sqlite, SqlitePool};

use crate::core::Result;
use crate::db::models::ClipboardApp;

const SELECT_APP: &str = "SELECT id, name, icon_file, platform, created_at, updated_at \
     FROM clipboard_apps";

#[allow(dead_code)]
pub async fn find_app_by_id(pool: &SqlitePool, id: &str) -> Result<Option<ClipboardApp>> {
    let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new(SELECT_APP);
    qb.push(" WHERE id = ").push_bind(id.to_owned());
    let row = qb
        .build_query_as::<ClipboardApp>()
        .fetch_optional(pool)
        .await
        .context("failed to find clipboard app by id")?;
    Ok(row)
}

pub async fn list_apps_by_ids(pool: &SqlitePool, ids: &[String]) -> Result<Vec<ClipboardApp>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new(SELECT_APP);
    qb.push(" WHERE id IN (");
    let mut sep = qb.separated(", ");
    for id in ids {
        sep.push_bind(id.clone());
    }
    qb.push(")");
    let rows = qb
        .build_query_as::<ClipboardApp>()
        .fetch_all(pool)
        .await
        .context("failed to list clipboard apps by ids")?;
    Ok(rows)
}

pub async fn list_all_apps(pool: &SqlitePool) -> Result<Vec<ClipboardApp>> {
    let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new(SELECT_APP);
    qb.push(" ORDER BY name COLLATE NOCASE ASC, id ASC");
    let rows = qb
        .build_query_as::<ClipboardApp>()
        .fetch_all(pool)
        .await
        .context("failed to list all clipboard apps")?;
    Ok(rows)
}

pub async fn delete_unreferenced_apps(pool: &SqlitePool, ids: &[String]) -> Result<Vec<String>> {
    let mut deleted = Vec::new();

    for id in ids {
        let result = sqlx::query(
            "DELETE FROM clipboard_apps \
             WHERE id = ? \
               AND NOT EXISTS (SELECT 1 FROM clipboard_items WHERE source_app_id = ? LIMIT 1)",
        )
        .bind(id)
        .bind(id)
        .execute(pool)
        .await
        .context("failed to delete unreferenced clipboard app")?;

        if result.rows_affected() > 0 {
            deleted.push(id.clone());
        }
    }

    Ok(deleted)
}

pub async fn upsert_app(pool: &SqlitePool, app: &ClipboardApp) -> Result<()> {
    let now = Utc::now();
    let updated = sqlx::query(
        "UPDATE clipboard_apps SET name = ?, icon_file = ?, updated_at = ? WHERE id = ?",
    )
    .bind(app.name.as_str())
    .bind(app.icon_file.as_deref())
    .bind(now)
    .bind(app.id.as_str())
    .execute(pool)
    .await
    .context("failed to update clipboard app")?;

    if updated.rows_affected() > 0 {
        return Ok(());
    }

    sqlx::query(
        "INSERT INTO clipboard_apps (id, name, icon_file, platform, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(app.id.as_str())
    .bind(app.name.as_str())
    .bind(app.icon_file.as_deref())
    .bind(app.platform)
    .bind(app.created_at)
    .bind(app.updated_at)
    .execute(pool)
    .await
    .context("failed to insert clipboard app")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::Platform;
    use crate::db::test_support::memory_pool;
    use chrono::DateTime;

    fn sample_app(id: &str) -> ClipboardApp {
        let ts = DateTime::from_timestamp(1_700_000_000, 0).unwrap();
        ClipboardApp {
            id: id.to_owned(),
            name: format!("name-{id}"),
            icon_file: Some(format!("{id}.png")),
            platform: Platform::Macos,
            created_at: ts,
            updated_at: ts,
        }
    }

    #[tokio::test]
    async fn insert_then_find_roundtrip() {
        let pool = memory_pool().await;
        let app = sample_app("com.example.foo");
        upsert_app(&pool, &app).await.unwrap();

        let got = find_app_by_id(&pool, &app.id).await.unwrap().unwrap();
        assert_eq!(got.id, app.id);
        assert_eq!(got.name, "name-com.example.foo");
        assert_eq!(got.icon_file.as_deref(), Some("com.example.foo.png"));
    }

    #[tokio::test]
    async fn upsert_refreshes_name_and_icon_but_keeps_created_at() {
        let pool = memory_pool().await;
        let mut app = sample_app("com.example.bar");
        upsert_app(&pool, &app).await.unwrap();
        let first = find_app_by_id(&pool, &app.id).await.unwrap().unwrap();

        app.name = "renamed".to_owned();
        app.icon_file = Some("other.png".to_owned());
        upsert_app(&pool, &app).await.unwrap();

        let after = find_app_by_id(&pool, &app.id).await.unwrap().unwrap();
        assert_eq!(after.name, "renamed");
        assert_eq!(after.icon_file.as_deref(), Some("other.png"));

        assert_eq!(after.created_at, first.created_at);
        assert!(after.updated_at >= first.updated_at);
    }

    #[tokio::test]
    async fn find_missing_returns_none() {
        let pool = memory_pool().await;
        assert!(find_app_by_id(&pool, "nope").await.unwrap().is_none());
    }
}
