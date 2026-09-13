//! Clipboard item queries and mutations, including FTS filtering and cleanup.

use anyhow::Context;
use blake3::Hasher;
use chrono::Utc;
use sqlx::{QueryBuilder, Sqlite, SqlitePool};

use crate::core::Result;
use crate::db::models::{
    ClipboardGroupFilter, ClipboardItem, ClipboardItemQuery, ClipboardItemSort, ClipboardKind,
};

const SELECT_ITEM: &str = "SELECT id, kind, sub_kind, group_id, source_app_id, content, \
     content_hash, search_text, summary, file_types, size, width, height, use_count, is_favorite, is_pinned, \
     is_sensitive, platform, note, created_at, updated_at FROM clipboard_items";

const LIST_SELECT_ITEM: &str = "SELECT clipboard_items.id, clipboard_items.kind, \
     clipboard_items.sub_kind, clipboard_items.group_id, clipboard_items.source_app_id, \
     CASE WHEN clipboard_items.kind = 'text' THEN '' ELSE clipboard_items.content END AS content, \
     clipboard_items.content_hash, \
     CASE WHEN clipboard_items.kind = 'text' THEN NULL ELSE clipboard_items.search_text END AS search_text, \
     clipboard_items.summary, clipboard_items.file_types, clipboard_items.size, \
     clipboard_items.width, clipboard_items.height, clipboard_items.use_count, \
     clipboard_items.is_favorite, clipboard_items.is_pinned, \
     clipboard_items.is_sensitive, \
     clipboard_items.platform, clipboard_items.note, \
     clipboard_items.created_at, clipboard_items.updated_at, \
     clipboard_apps.name AS source_app_name, \
     clipboard_apps.icon_file AS source_app_icon_file \
     FROM clipboard_items \
     LEFT JOIN clipboard_apps ON clipboard_apps.id = clipboard_items.source_app_id";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpsertResult {
    pub id: String,
    pub deduplicated: bool,
}

pub fn content_hash(kind: ClipboardKind, content: &str) -> String {
    let mut hasher = Hasher::new();
    hasher.update(kind_tag(kind).as_bytes());
    hasher.update(b":");
    hasher.update(content.as_bytes());
    hasher.finalize().to_hex().to_string()
}

fn kind_tag(kind: ClipboardKind) -> &'static str {
    match kind {
        ClipboardKind::Text => "text",
        ClipboardKind::Image => "image",
        ClipboardKind::Files => "files",
    }
}

pub async fn upsert_item(pool: &SqlitePool, item: &ClipboardItem) -> Result<UpsertResult> {
    if let Some(existing) = find_item_by_content_hash(pool, &item.content_hash).await? {
        increment_item_use_count(pool, &existing.id).await?;
        return Ok(UpsertResult {
            id: existing.id,
            deduplicated: true,
        });
    }

    insert_item(pool, item).await?;
    Ok(UpsertResult {
        id: item.id.clone(),
        deduplicated: false,
    })
}

pub async fn find_item_by_content_hash(
    pool: &SqlitePool,
    hash: &str,
) -> Result<Option<ClipboardItem>> {
    let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new(SELECT_ITEM);
    qb.push(" WHERE content_hash = ")
        .push_bind(hash.to_owned())
        .push(" ORDER BY created_at DESC LIMIT 1");

    let item = qb
        .build_query_as::<ClipboardItem>()
        .fetch_optional(pool)
        .await
        .context("failed to find clipboard item by content_hash")?;
    Ok(item)
}

pub async fn insert_item(pool: &SqlitePool, item: &ClipboardItem) -> Result<()> {
    sqlx::query(
        "INSERT INTO clipboard_items \
         (id, kind, sub_kind, group_id, source_app_id, content, content_hash, search_text, \
          summary, file_types, size, width, height, use_count, is_favorite, is_pinned, is_sensitive, platform, note, \
          created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(item.id.as_str())
    .bind(item.kind)
    .bind(item.sub_kind)
    .bind(item.group_id.as_deref())
    .bind(item.source_app_id.as_deref())
    .bind(item.content.as_str())
    .bind(item.content_hash.as_str())
    .bind(item.search_text.as_deref())
    .bind(item.summary.as_deref())
    .bind(item.file_types.as_deref())
    .bind(item.size)
    .bind(item.width)
    .bind(item.height)
    .bind(item.use_count)
    .bind(item.is_favorite)
    .bind(item.is_pinned)
    .bind(item.is_sensitive)
    .bind(item.platform)
    .bind(item.note.as_deref())
    .bind(item.created_at)
    .bind(item.updated_at)
    .execute(pool)
    .await
    .context("failed to insert clipboard item")?;
    Ok(())
}

#[cfg(test)]
pub async fn query_items(pool: &SqlitePool, q: &ClipboardItemQuery) -> Result<Vec<ClipboardItem>> {
    fetch_items(pool, q, KeywordFilter::from_keyword(q.keyword.as_deref())).await
}

pub async fn query_items_page(
    pool: &SqlitePool,
    q: &ClipboardItemQuery,
) -> Result<(Vec<ClipboardItem>, i64)> {
    let keyword = KeywordFilter::from_keyword(q.keyword.as_deref());
    let items = fetch_items(pool, q, keyword.clone()).await?;
    let total = fetch_items_count(pool, q, keyword).await?;
    Ok((items, total))
}

pub async fn find_item_by_id(pool: &SqlitePool, id: &str) -> Result<Option<ClipboardItem>> {
    let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new(SELECT_ITEM);
    qb.push(" WHERE id = ").push_bind(id.to_owned());

    let item = qb
        .build_query_as::<ClipboardItem>()
        .fetch_optional(pool)
        .await
        .context("failed to find clipboard item by id")?;
    Ok(item)
}

pub async fn find_item_for_list_by_id(
    pool: &SqlitePool,
    id: &str,
) -> Result<Option<ClipboardItem>> {
    let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new(LIST_SELECT_ITEM);
    qb.push(" WHERE clipboard_items.id = ")
        .push_bind(id.to_owned());

    let item = qb
        .build_query_as::<ClipboardItem>()
        .fetch_optional(pool)
        .await
        .context("failed to find clipboard item (list view) by id")?;
    Ok(item)
}

pub async fn toggle_item_favorite(pool: &SqlitePool, id: &str) -> Result<bool> {
    let new_value: bool = sqlx::query_scalar(
        "UPDATE clipboard_items SET is_favorite = NOT is_favorite WHERE id = ? RETURNING is_favorite",
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .context("failed to toggle clipboard item favorite")?;
    Ok(new_value)
}

pub async fn mark_item_favorite(pool: &SqlitePool, id: &str) -> Result<()> {
    sqlx::query("UPDATE clipboard_items SET is_favorite = 1 WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .context("failed to mark clipboard item favorite")?;
    Ok(())
}

pub async fn toggle_item_pinned(pool: &SqlitePool, id: &str) -> Result<bool> {
    let new_value: bool = sqlx::query_scalar(
        "UPDATE clipboard_items SET is_pinned = NOT is_pinned WHERE id = ? RETURNING is_pinned",
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .context("failed to toggle clipboard item pinned")?;
    Ok(new_value)
}

pub async fn update_item_note(pool: &SqlitePool, id: &str, note: Option<&str>) -> Result<()> {
    sqlx::query("UPDATE clipboard_items SET note = ? WHERE id = ?")
        .bind(note)
        .bind(id)
        .execute(pool)
        .await
        .context("failed to update clipboard item note")?;
    Ok(())
}

pub async fn update_item_group(pool: &SqlitePool, id: &str, group_id: Option<&str>) -> Result<()> {
    sqlx::query("UPDATE clipboard_items SET group_id = ? WHERE id = ?")
        .bind(group_id)
        .bind(id)
        .execute(pool)
        .await
        .context("failed to update clipboard item group")?;
    Ok(())
}

pub async fn increment_item_use_count(pool: &SqlitePool, id: &str) -> Result<()> {
    sqlx::query(
        "UPDATE clipboard_items SET use_count = use_count + 1, updated_at = ? WHERE id = ?",
    )
    .bind(Utc::now())
    .bind(id)
    .execute(pool)
    .await
    .context("failed to increment clipboard item use_count")?;
    Ok(())
}

pub async fn delete_item(pool: &SqlitePool, id: &str) -> Result<Option<String>> {
    let row = sqlx::query_as::<_, (ClipboardKind, String)>(
        "DELETE FROM clipboard_items WHERE id = ? RETURNING kind, content",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .context("failed to delete clipboard item")?;
    Ok(row.and_then(|(kind, content)| image_file_name(kind, content)))
}

fn image_file_name(kind: ClipboardKind, content: String) -> Option<String> {
    (kind == ClipboardKind::Image).then_some(content)
}

#[allow(dead_code)]
pub async fn delete_items(pool: &SqlitePool, ids: &[String]) -> Result<u64> {
    if ids.is_empty() {
        return Ok(0);
    }

    let mut qb: QueryBuilder<Sqlite> =
        QueryBuilder::new("DELETE FROM clipboard_items WHERE id IN (");
    let mut separated = qb.separated(", ");
    for id in ids {
        separated.push_bind(id);
    }
    qb.push(")");

    let result = qb
        .build()
        .execute(pool)
        .await
        .context("failed to delete clipboard items")?;
    Ok(result.rows_affected())
}

#[derive(Debug, Default)]
pub struct CleanupOutcome {
    pub removed: u64,
    pub image_files: Vec<String>,
}

pub async fn cleanup_history(
    pool: &SqlitePool,
    older_than: Option<chrono::DateTime<chrono::Utc>>,
    max_count: Option<u32>,
) -> Result<CleanupOutcome> {
    let mut outcome = CleanupOutcome::default();

    if let Some(cutoff) = older_than {
        let rows = sqlx::query_as::<_, (ClipboardKind, String)>(
            "DELETE FROM clipboard_items \
             WHERE is_pinned = 0 AND is_favorite = 0 AND created_at < ? \
             RETURNING kind, content",
        )
        .bind(cutoff)
        .fetch_all(pool)
        .await
        .context("failed to cleanup clipboard items by retention")?;
        absorb_deleted(&mut outcome, rows);
    }

    if let Some(max) = max_count.filter(|n| *n > 0) {
        let rows = sqlx::query_as::<_, (ClipboardKind, String)>(
            "DELETE FROM clipboard_items WHERE id IN ( \
                 SELECT id FROM clipboard_items \
                 WHERE is_pinned = 0 AND is_favorite = 0 \
                 ORDER BY created_at DESC \
                 LIMIT -1 OFFSET ? \
             ) \
             RETURNING kind, content",
        )
        .bind(max as i64)
        .fetch_all(pool)
        .await
        .context("failed to cleanup clipboard items by max_count")?;
        absorb_deleted(&mut outcome, rows);
    }

    Ok(outcome)
}

fn absorb_deleted(outcome: &mut CleanupOutcome, rows: Vec<(ClipboardKind, String)>) {
    outcome.removed += rows.len() as u64;
    outcome.image_files.extend(
        rows.into_iter()
            .filter_map(|(kind, content)| image_file_name(kind, content)),
    );
}

pub async fn clear_items(
    pool: &SqlitePool,
    delete_favorites: bool,
    delete_pinned: bool,
) -> Result<CleanupOutcome> {
    let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new("DELETE FROM clipboard_items");

    let mut has_condition = false;
    if !delete_favorites {
        qb.push(" WHERE is_favorite = 0");
        has_condition = true;
    }
    if !delete_pinned {
        qb.push(if has_condition { " AND " } else { " WHERE " });
        qb.push("is_pinned = 0");
    }
    qb.push(" RETURNING kind, content");

    let rows = qb
        .build_query_as::<(ClipboardKind, String)>()
        .fetch_all(pool)
        .await
        .context("failed to clear clipboard items")?;

    let mut outcome = CleanupOutcome::default();
    absorb_deleted(&mut outcome, rows);
    Ok(outcome)
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum KeywordFilter {
    None,

    Fts(String),

    Like(String),
}

impl KeywordFilter {
    fn from_keyword(keyword: Option<&str>) -> Self {
        let Some(trimmed) = keyword.map(str::trim).filter(|s| !s.is_empty()) else {
            return Self::None;
        };

        let fts_viable = trimmed
            .split_whitespace()
            .all(|token| token.chars().count() >= 3);

        if fts_viable {
            if let Some(expr) = build_fts_expr(trimmed) {
                return Self::Fts(expr);
            }
        }

        Self::Like(escape_like(trimmed))
    }
}

fn build_fts_expr(keyword: &str) -> Option<String> {
    let expr = keyword
        .split_whitespace()
        .map(|token| format!("\"{}\"*", token.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" ");

    (!expr.is_empty()).then_some(expr)
}

fn escape_like(keyword: &str) -> String {
    let mut out = String::with_capacity(keyword.len());
    for ch in keyword.chars() {
        if matches!(ch, '\\' | '%' | '_') {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

async fn fetch_items(
    pool: &SqlitePool,
    q: &ClipboardItemQuery,
    keyword: KeywordFilter,
) -> Result<Vec<ClipboardItem>> {
    let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new(LIST_SELECT_ITEM);
    qb.push(" WHERE 1 = 1");
    push_filter_clauses(&mut qb, q, &keyword);

    qb.push(" ORDER BY clipboard_items.is_pinned DESC, ");
    match q.sort {
        ClipboardItemSort::CreatedAt => {
            qb.push("clipboard_items.created_at DESC");
        }
        ClipboardItemSort::UpdatedAt => {
            qb.push("clipboard_items.updated_at DESC, clipboard_items.created_at DESC");
        }
        ClipboardItemSort::UseCount => {
            qb.push("clipboard_items.use_count DESC, clipboard_items.created_at DESC");
        }
    }

    qb.push(" LIMIT ").push_bind(q.limit);
    qb.push(" OFFSET ").push_bind(q.offset);

    let items = qb
        .build_query_as::<ClipboardItem>()
        .fetch_all(pool)
        .await
        .context("failed to query clipboard items")?;
    Ok(items)
}

async fn fetch_items_count(
    pool: &SqlitePool,
    q: &ClipboardItemQuery,
    keyword: KeywordFilter,
) -> Result<i64> {
    let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new("SELECT COUNT(*) FROM clipboard_items");
    qb.push(" WHERE 1 = 1");
    push_filter_clauses(&mut qb, q, &keyword);

    let row: (i64,) = qb
        .build_query_as()
        .fetch_one(pool)
        .await
        .context("failed to count filtered clipboard items")?;
    Ok(row.0)
}

fn push_filter_clauses(
    qb: &mut QueryBuilder<Sqlite>,
    q: &ClipboardItemQuery,
    keyword: &KeywordFilter,
) {
    match keyword {
        KeywordFilter::None => {}
        KeywordFilter::Fts(expr) => {
            qb.push(
                " AND clipboard_items.rowid IN (SELECT rowid FROM clipboard_items_fts WHERE clipboard_items_fts MATCH ",
            )
            .push_bind(expr.clone())
            .push(")");
        }
        KeywordFilter::Like(kw) => {
            let pattern = format!("%{kw}%");
            qb.push(" AND (clipboard_items.search_text LIKE ")
                .push_bind(pattern.clone())
                .push(" ESCAPE '\\' OR clipboard_items.note LIKE ")
                .push_bind(pattern)
                .push(" ESCAPE '\\')");
        }
    }

    let (effective_kind, effective_favorite) = match q.group {
        Some(ClipboardGroupFilter::All) => (None, None),
        Some(ClipboardGroupFilter::Text) => (Some(ClipboardKind::Text), None),
        Some(ClipboardGroupFilter::Image) => (Some(ClipboardKind::Image), None),
        Some(ClipboardGroupFilter::Files) => (Some(ClipboardKind::Files), None),
        Some(ClipboardGroupFilter::Favorite) => (None, Some(true)),
        None => (q.kind, q.favorite),
    };
    if let Some(kind) = effective_kind {
        qb.push(" AND clipboard_items.kind = ").push_bind(kind);
    }
    if let Some(group_id) = &q.group_id {
        qb.push(" AND clipboard_items.group_id = ")
            .push_bind(group_id.clone());
    }
    if let Some(favorite) = effective_favorite {
        qb.push(" AND clipboard_items.is_favorite = ")
            .push_bind(favorite);
    }
    if let Some(pinned) = q.pinned {
        qb.push(" AND clipboard_items.is_pinned = ")
            .push_bind(pinned);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::groups::insert_group;
    use crate::db::models::{ClipboardGroup, ClipboardKind, Platform};
    use crate::db::test_support::memory_pool;
    use chrono::DateTime;

    fn sample_item(id: &str) -> ClipboardItem {
        let ts = DateTime::from_timestamp(1_700_000_000, 0).unwrap();
        let content = format!("content-{id}");
        ClipboardItem {
            id: id.to_owned(),
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
            is_sensitive: false,
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
        }
    }

    fn ids(items: &[ClipboardItem]) -> Vec<&str> {
        items.iter().map(|item| item.id.as_str()).collect()
    }

    #[tokio::test]
    async fn insert_and_find_by_id_roundtrip() {
        let pool = memory_pool().await;
        insert_item(&pool, &sample_item("a")).await.unwrap();

        let found = find_item_by_id(&pool, "a")
            .await
            .unwrap()
            .expect("item should exist");
        assert_eq!(found.id, "a");
        assert_eq!(found.content, "content-a");
        assert_eq!(found.kind, ClipboardKind::Text);
        assert_eq!(found.use_count, 1);

        assert!(find_item_by_id(&pool, "missing").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn upsert_inserts_when_content_is_new() {
        let pool = memory_pool().await;
        let result = upsert_item(&pool, &sample_item("a")).await.unwrap();
        assert_eq!(
            result,
            UpsertResult {
                id: "a".to_owned(),
                deduplicated: false,
            }
        );
        assert_eq!(
            find_item_by_id(&pool, "a")
                .await
                .unwrap()
                .unwrap()
                .use_count,
            1
        );
    }

    #[tokio::test]
    async fn upsert_dedups_same_content_bumping_use_count() {
        let pool = memory_pool().await;
        let first = sample_item("first");
        upsert_item(&pool, &first).await.unwrap();

        let mut dup = sample_item("second");
        dup.content = first.content.clone();
        dup.content_hash = first.content_hash.clone();
        let result = upsert_item(&pool, &dup).await.unwrap();

        assert_eq!(
            result,
            UpsertResult {
                id: "first".to_owned(),
                deduplicated: true,
            }
        );

        let all = query_items(&pool, &ClipboardItemQuery::default())
            .await
            .unwrap();
        assert_eq!(ids(&all), ["first"]);
        assert_eq!(all[0].use_count, 2);
        assert!(find_item_by_id(&pool, "second").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn upsert_keeps_distinct_content_separate() {
        let pool = memory_pool().await;
        upsert_item(&pool, &sample_item("a")).await.unwrap();
        upsert_item(&pool, &sample_item("b")).await.unwrap();

        let all = query_items(&pool, &ClipboardItemQuery::default())
            .await
            .unwrap();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn query_default_sorts_by_updated_at() {
        assert_eq!(
            ClipboardItemQuery::default().sort,
            ClipboardItemSort::UpdatedAt
        );
    }

    #[test]
    fn content_hash_is_stable_and_kind_scoped() {
        assert_eq!(
            content_hash(ClipboardKind::Text, "hello"),
            content_hash(ClipboardKind::Text, "hello")
        );

        assert_ne!(
            content_hash(ClipboardKind::Text, "hello"),
            content_hash(ClipboardKind::Text, "world")
        );

        assert_ne!(
            content_hash(ClipboardKind::Text, "same"),
            content_hash(ClipboardKind::Files, "same")
        );
    }

    #[tokio::test]
    async fn query_filters_by_kind() {
        let pool = memory_pool().await;
        let mut text = sample_item("text");
        text.kind = ClipboardKind::Text;
        let mut image = sample_item("image");
        image.kind = ClipboardKind::Image;
        insert_item(&pool, &text).await.unwrap();
        insert_item(&pool, &image).await.unwrap();

        let q = ClipboardItemQuery {
            kind: Some(ClipboardKind::Image),
            ..Default::default()
        };
        assert_eq!(ids(&query_items(&pool, &q).await.unwrap()), ["image"]);
    }

    #[tokio::test]
    async fn query_filters_by_favorite_and_pinned() {
        let pool = memory_pool().await;
        let plain = sample_item("plain");
        let mut fav = sample_item("fav");
        fav.is_favorite = true;
        let mut pin = sample_item("pin");
        pin.is_pinned = true;
        for item in [&plain, &fav, &pin] {
            insert_item(&pool, item).await.unwrap();
        }

        let favs = query_items(
            &pool,
            &ClipboardItemQuery {
                favorite: Some(true),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(ids(&favs), ["fav"]);

        let pins = query_items(
            &pool,
            &ClipboardItemQuery {
                pinned: Some(true),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(ids(&pins), ["pin"]);
    }

    #[tokio::test]
    async fn query_filters_by_group() {
        let pool = memory_pool().await;
        let group = ClipboardGroup {
            id: "g1".to_owned(),
            name: "G1".to_owned(),
            icon: "i-lets-icons:folder".to_owned(),
            is_hidden: false,
            sort_order: 0,
            created_at: DateTime::from_timestamp(1_700_000_000, 0).unwrap(),
            updated_at: DateTime::from_timestamp(1_700_000_000, 0).unwrap(),
        };
        insert_group(&pool, &group).await.unwrap();

        let mut grouped = sample_item("grouped");
        grouped.group_id = Some("g1".to_owned());
        insert_item(&pool, &grouped).await.unwrap();
        insert_item(&pool, &sample_item("ungrouped")).await.unwrap();

        let q = ClipboardItemQuery {
            group_id: Some("g1".to_owned()),
            ..Default::default()
        };
        assert_eq!(ids(&query_items(&pool, &q).await.unwrap()), ["grouped"]);
    }

    #[tokio::test]
    async fn update_item_group_does_not_refresh_updated_at() {
        let pool = memory_pool().await;
        let group = ClipboardGroup {
            id: "g1".to_owned(),
            name: "G1".to_owned(),
            icon: "i-lets-icons:folder".to_owned(),
            is_hidden: false,
            sort_order: 0,
            created_at: DateTime::from_timestamp(1_700_000_000, 0).unwrap(),
            updated_at: DateTime::from_timestamp(1_700_000_000, 0).unwrap(),
        };
        insert_group(&pool, &group).await.unwrap();

        let item = sample_item("item");
        let original_updated_at = item.updated_at;
        insert_item(&pool, &item).await.unwrap();

        update_item_group(&pool, "item", Some("g1")).await.unwrap();

        let moved = find_item_by_id(&pool, "item").await.unwrap().unwrap();
        assert_eq!(moved.group_id, Some("g1".to_owned()));
        assert_eq!(moved.updated_at, original_updated_at);
    }

    #[tokio::test]
    async fn query_orders_pinned_first_then_by_sort() {
        let pool = memory_pool().await;
        let mut a = sample_item("a");
        a.created_at = DateTime::from_timestamp(1_700_000_000, 0).unwrap();
        a.updated_at = DateTime::from_timestamp(1_700_000_010, 0).unwrap();
        a.use_count = 9;
        let mut b = sample_item("b");
        b.created_at = DateTime::from_timestamp(1_700_000_010, 0).unwrap();
        b.updated_at = DateTime::from_timestamp(1_700_000_020, 0).unwrap();
        b.is_pinned = true;
        b.use_count = 1;
        let mut c = sample_item("c");
        c.created_at = DateTime::from_timestamp(1_700_000_020, 0).unwrap();
        c.updated_at = DateTime::from_timestamp(1_700_000_000, 0).unwrap();
        c.use_count = 5;
        for item in [&a, &b, &c] {
            insert_item(&pool, item).await.unwrap();
        }

        let by_time = query_items(
            &pool,
            &ClipboardItemQuery {
                sort: ClipboardItemSort::CreatedAt,
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(ids(&by_time), ["b", "c", "a"]);

        let by_updated = query_items(
            &pool,
            &ClipboardItemQuery {
                sort: ClipboardItemSort::UpdatedAt,
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(ids(&by_updated), ["b", "a", "c"]);

        let by_use = query_items(
            &pool,
            &ClipboardItemQuery {
                sort: ClipboardItemSort::UseCount,
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(ids(&by_use), ["b", "a", "c"]);
    }

    #[tokio::test]
    async fn query_paginates_with_limit_and_offset() {
        let pool = memory_pool().await;
        for n in 0..5i64 {
            let mut item = sample_item(&format!("id{n}"));
            item.created_at = DateTime::from_timestamp(1_700_000_000 + n, 0).unwrap();
            insert_item(&pool, &item).await.unwrap();
        }

        let page = |limit, offset| ClipboardItemQuery {
            limit,
            offset,
            ..Default::default()
        };

        assert_eq!(
            ids(&query_items(&pool, &page(2, 0)).await.unwrap()),
            ["id4", "id3"]
        );
        assert_eq!(
            ids(&query_items(&pool, &page(2, 2)).await.unwrap()),
            ["id2", "id1"]
        );
        assert_eq!(
            ids(&query_items(&pool, &page(2, 4)).await.unwrap()),
            ["id0"]
        );
    }

    #[tokio::test]
    async fn search_fts_matches_prefix_across_columns() {
        let pool = memory_pool().await;
        let mut a = sample_item("a");
        a.content = "hello rustacean".to_owned();
        a.search_text = Some("hello rustacean".to_owned());
        let mut b = sample_item("b");
        b.content = "goodbye world".to_owned();
        b.search_text = Some("searchable token".to_owned());
        let mut c = sample_item("c");
        c.content = "plain".to_owned();
        c.note = Some("annotated".to_owned());
        for item in [&a, &b, &c] {
            insert_item(&pool, item).await.unwrap();
        }

        let search = |kw: &str| ClipboardItemQuery {
            keyword: Some(kw.to_owned()),
            ..Default::default()
        };
        assert_eq!(
            ids(&query_items(&pool, &search("rust")).await.unwrap()),
            ["a"]
        );
        assert_eq!(
            ids(&query_items(&pool, &search("token")).await.unwrap()),
            ["b"]
        );
        assert_eq!(
            ids(&query_items(&pool, &search("annot")).await.unwrap()),
            ["c"]
        );
        assert!(query_items(&pool, &search("zzz")).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn search_fts_still_applies_filters() {
        let pool = memory_pool().await;
        let mut x = sample_item("x");
        x.content = "shared text".to_owned();
        x.search_text = Some("shared text".to_owned());
        x.is_favorite = true;
        let mut y = sample_item("y");
        y.content = "shared note".to_owned();
        y.search_text = Some("shared note".to_owned());
        insert_item(&pool, &x).await.unwrap();
        insert_item(&pool, &y).await.unwrap();

        let q = ClipboardItemQuery {
            keyword: Some("shared".to_owned()),
            favorite: Some(true),
            ..Default::default()
        };
        assert_eq!(ids(&query_items(&pool, &q).await.unwrap()), ["x"]);
    }

    #[tokio::test]
    async fn toggle_favorite_and_pinned_flip_flags() {
        let pool = memory_pool().await;
        insert_item(&pool, &sample_item("a")).await.unwrap();

        toggle_item_favorite(&pool, "a").await.unwrap();
        assert!(
            find_item_by_id(&pool, "a")
                .await
                .unwrap()
                .unwrap()
                .is_favorite
        );
        toggle_item_favorite(&pool, "a").await.unwrap();
        assert!(
            !find_item_by_id(&pool, "a")
                .await
                .unwrap()
                .unwrap()
                .is_favorite
        );

        toggle_item_pinned(&pool, "a").await.unwrap();
        assert!(
            find_item_by_id(&pool, "a")
                .await
                .unwrap()
                .unwrap()
                .is_pinned
        );
        toggle_item_pinned(&pool, "a").await.unwrap();
        assert!(
            !find_item_by_id(&pool, "a")
                .await
                .unwrap()
                .unwrap()
                .is_pinned
        );
    }

    #[tokio::test]
    async fn update_note_sets_and_clears() {
        let pool = memory_pool().await;
        insert_item(&pool, &sample_item("a")).await.unwrap();

        update_item_note(&pool, "a", Some("my note")).await.unwrap();
        assert_eq!(
            find_item_by_id(&pool, "a")
                .await
                .unwrap()
                .unwrap()
                .note
                .as_deref(),
            Some("my note")
        );

        update_item_note(&pool, "a", None).await.unwrap();
        assert_eq!(
            find_item_by_id(&pool, "a").await.unwrap().unwrap().note,
            None
        );
    }

    #[tokio::test]
    async fn increment_use_count_bumps_count_and_updated_at() {
        let pool = memory_pool().await;
        let item = sample_item("a");
        let original_updated_at = item.updated_at;
        insert_item(&pool, &item).await.unwrap();

        increment_item_use_count(&pool, "a").await.unwrap();

        let after = find_item_by_id(&pool, "a").await.unwrap().unwrap();
        assert_eq!(after.use_count, 2);
        assert!(after.updated_at > original_updated_at);
    }

    #[tokio::test]
    async fn delete_item_removes_row() {
        let pool = memory_pool().await;
        insert_item(&pool, &sample_item("a")).await.unwrap();

        assert_eq!(delete_item(&pool, "a").await.unwrap(), None);
        assert!(find_item_by_id(&pool, "a").await.unwrap().is_none());

        assert_eq!(delete_item(&pool, "missing").await.unwrap(), None);
    }

    #[tokio::test]
    async fn delete_image_item_returns_file_name() {
        let pool = memory_pool().await;
        let mut img = sample_item("img");
        img.kind = ClipboardKind::Image;
        img.content = "deadbeef.png".to_owned();
        img.content_hash = content_hash(ClipboardKind::Image, "deadbeef.png");
        insert_item(&pool, &img).await.unwrap();

        assert_eq!(
            delete_item(&pool, "img").await.unwrap().as_deref(),
            Some("deadbeef.png")
        );
        assert!(find_item_by_id(&pool, "img").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn delete_items_returns_count_and_ignores_empty() {
        let pool = memory_pool().await;
        for id in ["a", "b", "c"] {
            insert_item(&pool, &sample_item(id)).await.unwrap();
        }

        assert_eq!(delete_items(&pool, &[]).await.unwrap(), 0);

        let removed = delete_items(
            &pool,
            &["a".to_owned(), "b".to_owned(), "missing".to_owned()],
        )
        .await
        .unwrap();
        assert_eq!(removed, 2);
        assert_eq!(
            ids(&query_items(&pool, &ClipboardItemQuery::default())
                .await
                .unwrap()),
            ["c"]
        );
    }

    #[tokio::test]
    async fn clear_items_optionally_keeps_favorites_and_pinned() {
        let pool = memory_pool().await;
        let mut fav = sample_item("fav");
        fav.is_favorite = true;
        fav.created_at = DateTime::from_timestamp(1_700_000_001, 0).unwrap();
        insert_item(&pool, &fav).await.unwrap();
        let mut pin = sample_item("pin");
        pin.is_pinned = true;
        pin.created_at = DateTime::from_timestamp(1_700_000_002, 0).unwrap();
        insert_item(&pool, &pin).await.unwrap();
        let mut both = sample_item("both");
        both.is_favorite = true;
        both.is_pinned = true;
        both.created_at = DateTime::from_timestamp(1_700_000_003, 0).unwrap();
        insert_item(&pool, &both).await.unwrap();
        let mut plain = sample_item("plain");
        plain.created_at = DateTime::from_timestamp(1_700_000_004, 0).unwrap();
        insert_item(&pool, &plain).await.unwrap();

        assert_eq!(clear_items(&pool, false, false).await.unwrap().removed, 1);
        assert_eq!(
            ids(&query_items(&pool, &ClipboardItemQuery::default())
                .await
                .unwrap()),
            ["both", "pin", "fav"]
        );

        assert_eq!(clear_items(&pool, true, false).await.unwrap().removed, 1);
        assert_eq!(
            ids(&query_items(&pool, &ClipboardItemQuery::default())
                .await
                .unwrap()),
            ["both", "pin"]
        );

        assert_eq!(clear_items(&pool, true, true).await.unwrap().removed, 2);
        assert!(query_items(&pool, &ClipboardItemQuery::default())
            .await
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn cleanup_history_drops_old_items_keeping_pinned_and_favorite() {
        let pool = memory_pool().await;
        let mk = |id: &str, ts: i64, fav: bool, pin: bool| {
            let mut it = sample_item(id);
            it.created_at = DateTime::from_timestamp(ts, 0).unwrap();
            it.is_favorite = fav;
            it.is_pinned = pin;
            it
        };
        let old_plain = mk("old", 1_000, false, false);
        let old_fav = mk("old-fav", 1_000, true, false);
        let old_pin = mk("old-pin", 1_000, false, true);
        let recent = mk("recent", 9_000, false, false);
        for item in [&old_plain, &old_fav, &old_pin, &recent] {
            insert_item(&pool, item).await.unwrap();
        }

        let cutoff = DateTime::from_timestamp(5_000, 0).unwrap();
        let outcome = cleanup_history(&pool, Some(cutoff), None).await.unwrap();
        assert_eq!(outcome.removed, 1);
        assert!(outcome.image_files.is_empty());

        let all = query_items(&pool, &ClipboardItemQuery::default())
            .await
            .unwrap();

        assert_eq!(ids(&all), ["old-pin", "recent", "old-fav"]);
    }

    #[tokio::test]
    async fn cleanup_history_enforces_max_count_keeping_pinned_and_favorite() {
        let pool = memory_pool().await;

        for n in 0..5i64 {
            let mut it = sample_item(&format!("p{n}"));
            it.created_at = DateTime::from_timestamp(1_000 + n, 0).unwrap();
            insert_item(&pool, &it).await.unwrap();
        }
        let mut fav = sample_item("fav");
        fav.is_favorite = true;
        fav.created_at = DateTime::from_timestamp(500, 0).unwrap();
        insert_item(&pool, &fav).await.unwrap();
        let mut pin = sample_item("pin");
        pin.is_pinned = true;
        pin.created_at = DateTime::from_timestamp(400, 0).unwrap();
        insert_item(&pool, &pin).await.unwrap();

        let outcome = cleanup_history(&pool, None, Some(2)).await.unwrap();
        assert_eq!(outcome.removed, 3);
        assert!(outcome.image_files.is_empty());

        let all = query_items(&pool, &ClipboardItemQuery::default())
            .await
            .unwrap();
        assert_eq!(ids(&all), ["pin", "p4", "p3", "fav"]);
    }

    #[tokio::test]
    async fn cleanup_history_no_op_when_both_disabled() {
        let pool = memory_pool().await;
        insert_item(&pool, &sample_item("a")).await.unwrap();
        assert_eq!(cleanup_history(&pool, None, None).await.unwrap().removed, 0);
        assert_eq!(
            cleanup_history(&pool, None, Some(0)).await.unwrap().removed,
            0
        );
        let all = query_items(&pool, &ClipboardItemQuery::default())
            .await
            .unwrap();
        assert_eq!(ids(&all), ["a"]);
    }

    #[tokio::test]
    async fn cleanup_history_collects_deleted_image_file_names() {
        let pool = memory_pool().await;

        let mut img = sample_item("img");
        img.kind = ClipboardKind::Image;
        img.content = "cafe1234.png".to_owned();
        img.content_hash = content_hash(ClipboardKind::Image, "cafe1234.png");
        img.created_at = DateTime::from_timestamp(1_000, 0).unwrap();
        let mut txt = sample_item("txt");
        txt.created_at = DateTime::from_timestamp(1_000, 0).unwrap();
        insert_item(&pool, &img).await.unwrap();
        insert_item(&pool, &txt).await.unwrap();

        let cutoff = DateTime::from_timestamp(5_000, 0).unwrap();
        let outcome = cleanup_history(&pool, Some(cutoff), None).await.unwrap();
        assert_eq!(outcome.removed, 2);
        assert_eq!(outcome.image_files, vec!["cafe1234.png".to_owned()]);
    }

    #[test]
    fn keyword_filter_classifies_by_char_length() {
        assert_eq!(KeywordFilter::from_keyword(None), KeywordFilter::None);
        assert_eq!(
            KeywordFilter::from_keyword(Some("   ")),
            KeywordFilter::None
        );

        assert_eq!(
            KeywordFilter::from_keyword(Some("a")),
            KeywordFilter::Like("a".to_owned())
        );
        assert_eq!(
            KeywordFilter::from_keyword(Some("한국")),
            KeywordFilter::Like("한국".to_owned())
        );

        assert_eq!(
            KeywordFilter::from_keyword(Some("foo")),
            KeywordFilter::Fts("\"foo\"*".to_owned())
        );
        assert_eq!(
            KeywordFilter::from_keyword(Some("foo bar")),
            KeywordFilter::Fts("\"foo\"* \"bar\"*".to_owned())
        );
        assert_eq!(
            KeywordFilter::from_keyword(Some("a\"b")),
            KeywordFilter::Fts("\"a\"\"b\"*".to_owned())
        );
    }

    #[test]
    fn keyword_filter_drops_multi_token_short_words_to_like() {
        assert_eq!(
            KeywordFilter::from_keyword(Some("a b")),
            KeywordFilter::Like("a b".to_owned())
        );
        assert_eq!(
            KeywordFilter::from_keyword(Some("ab cd")),
            KeywordFilter::Like("ab cd".to_owned())
        );

        assert_eq!(
            KeywordFilter::from_keyword(Some("foo b")),
            KeywordFilter::Like("foo b".to_owned())
        );

        assert_eq!(
            KeywordFilter::from_keyword(Some("foo bar baz")),
            KeywordFilter::Fts("\"foo\"* \"bar\"* \"baz\"*".to_owned())
        );
    }

    #[tokio::test]
    async fn multi_token_short_keyword_falls_back_to_like() {
        let pool = memory_pool().await;
        let mut item = sample_item("short");
        item.content = "ab cd ef".to_owned();
        item.content_hash = content_hash(ClipboardKind::Text, &item.content);
        item.search_text = Some("ab cd ef".to_owned());
        insert_item(&pool, &item).await.unwrap();

        let q = ClipboardItemQuery {
            keyword: Some("ab cd".to_owned()),
            ..Default::default()
        };

        let found = query_items(&pool, &q).await.unwrap();
        assert_eq!(ids(&found), ["short"]);
    }

    #[test]
    fn escape_like_escapes_special_chars() {
        assert_eq!(escape_like("100%"), "100\\%");
        assert_eq!(escape_like("a_b"), "a\\_b");
        assert_eq!(escape_like("c:\\path"), "c:\\\\path");
    }
}
