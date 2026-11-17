use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context;
use sqlx::SqlitePool;

use super::StorageAccess;
use crate::core::Result;
use crate::db::DatabaseState;

pub(crate) async fn clean_resource_files(
    access: &StorageAccess,
    db: &DatabaseState,
    resolve_roots: impl FnOnce() -> Result<(PathBuf, HashSet<String>)>,
) -> Result<CleanCacheStats> {
    let _resource_access = access.exclusive_owned().await;
    let pool = db.pool().await;
    let (resources_dir, cached_icons) = resolve_roots()?;
    let image_files = referenced_image_files(&pool).await?;
    let mut app_icon_files = referenced_app_icon_files(&pool).await?;
    app_icon_files.extend(cached_icons);
    let file_icon_files = referenced_file_icon_files(&pool).await?;

    let mut removed = CleanCacheStats::default();
    clean_sharded_files(
        &resources_dir.join("clipboard-images").join("origin"),
        &image_files,
        &mut removed,
    )?;
    clean_sharded_files(
        &resources_dir.join("clipboard-images").join("thumbnails"),
        &image_files,
        &mut removed,
    )?;
    clean_flat_files(
        &resources_dir.join("app-icons"),
        &app_icon_files,
        &mut removed,
    )?;
    clean_flat_files(
        &resources_dir.join("file-icons"),
        &file_icon_files,
        &mut removed,
    )?;

    Ok(removed)
}

async fn referenced_image_files(pool: &SqlitePool) -> Result<HashSet<String>> {
    let rows =
        sqlx::query_scalar::<_, String>("SELECT content FROM clipboard_items WHERE kind = 'image'")
            .fetch_all(pool)
            .await
            .context("failed to query referenced image files")?;

    Ok(rows.into_iter().collect())
}

async fn referenced_app_icon_files(pool: &SqlitePool) -> Result<HashSet<String>> {
    let rows = sqlx::query_scalar::<_, String>(
        "SELECT DISTINCT icon_file FROM clipboard_apps WHERE icon_file IS NOT NULL",
    )
    .fetch_all(pool)
    .await
    .context("failed to query referenced app icon files")?;

    Ok(rows.into_iter().collect())
}

async fn referenced_file_icon_files(pool: &SqlitePool) -> Result<HashSet<String>> {
    let rows = sqlx::query_scalar::<_, String>("SELECT DISTINCT icon_file FROM file_type_icons")
        .fetch_all(pool)
        .await
        .context("failed to query referenced file icon files")?;

    Ok(rows.into_iter().collect())
}

#[derive(Default)]
pub(crate) struct CleanCacheStats {
    pub(crate) files: u64,
    pub(crate) bytes: u64,
}

fn clean_flat_files(
    root: &Path,
    referenced_files: &HashSet<String>,
    removed: &mut CleanCacheStats,
) -> Result<()> {
    if !root.exists() {
        return Ok(());
    }

    for entry in
        fs::read_dir(root).with_context(|| format!("failed to read directory at {root:?}"))?
    {
        let entry = entry.with_context(|| format!("failed to read entry under {root:?}"))?;
        let path = entry.path();
        let metadata = entry
            .metadata()
            .with_context(|| format!("failed to read metadata at {path:?}"))?;

        if metadata.is_dir() {
            clean_flat_files(&path, referenced_files, removed)?;
            remove_dir_if_empty(&path);
            continue;
        }

        remove_unreferenced_file(&path, metadata.len(), referenced_files, removed)?;
    }

    remove_dir_if_empty(root);

    Ok(())
}

fn clean_sharded_files(
    root: &Path,
    referenced_files: &HashSet<String>,
    removed: &mut CleanCacheStats,
) -> Result<()> {
    clean_flat_files(root, referenced_files, removed)
}

fn remove_unreferenced_file(
    path: &Path,
    file_bytes: u64,
    referenced_files: &HashSet<String>,
    removed: &mut CleanCacheStats,
) -> Result<()> {
    let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
        return Ok(());
    };
    if referenced_files.contains(file_name) {
        return Ok(());
    }

    fs::remove_file(path).with_context(|| format!("failed to remove cache file {path:?}"))?;
    removed.files += 1;
    removed.bytes += file_bytes;

    Ok(())
}

fn remove_dir_if_empty(path: &Path) {
    let _ = fs::remove_dir(path);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_flat_files_removes_only_unreferenced_files() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("app-icons");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("keep.png"), b"keep").unwrap();
        fs::write(root.join("drop.png"), b"drop-this").unwrap();

        let referenced = HashSet::from(["keep.png".to_string()]);
        let mut removed = CleanCacheStats::default();

        clean_flat_files(&root, &referenced, &mut removed).unwrap();

        assert!(root.join("keep.png").exists());
        assert!(!root.join("drop.png").exists());
        assert_eq!(removed.files, 1);
        assert_eq!(removed.bytes, 9);
    }

    #[tokio::test]
    async fn cache_sweep_waits_for_file_reference_and_preserves_cached_icons() {
        use std::cell::Cell;
        use std::future::Future;
        use std::task::{Context, Poll, Waker};

        let temp = tempfile::tempdir().unwrap();
        let resources = temp.path().join("resources");
        let file_icons = resources.join("file-icons");
        let app_icons = resources.join("app-icons");
        fs::create_dir_all(&file_icons).unwrap();
        fs::create_dir_all(&app_icons).unwrap();
        let referenced = file_icons.join("new.png");
        let orphan = file_icons.join("old.png");
        let cached = app_icons.join("running.png");
        fs::write(&orphan, b"orphan").unwrap();
        fs::write(&cached, b"cached").unwrap();

        let pool = crate::db::test_support::memory_pool().await;
        let access = crate::storage::StorageAccess::default();
        let writer = access.read_owned().await;
        fs::write(&referenced, b"new").unwrap();

        let db = DatabaseState::new(pool.clone());
        let roots_resolved = Cell::new(false);
        let mut sweep = std::pin::pin!(clean_resource_files(&access, &db, || {
            roots_resolved.set(true);
            Ok((resources.clone(), HashSet::from(["running.png".to_owned()])))
        }));
        let mut context = Context::from_waker(Waker::noop());
        assert!(matches!(sweep.as_mut().poll(&mut context), Poll::Pending));
        assert!(!roots_resolved.get());

        sqlx::query("INSERT INTO file_type_icons (cache_key, platform, icon_file, created_at, updated_at) VALUES (?, ?, ?, ?, ?)")
            .bind("test")
            .bind("windows")
            .bind("new.png")
            .bind("2026-10-29T00:00:00Z")
            .bind("2026-10-29T00:00:00Z")
            .execute(&pool)
            .await
            .unwrap();
        drop(writer);
        let stats = sweep.await.unwrap();
        assert_eq!(stats.files, 1);
        assert!(referenced.exists());
        assert!(cached.exists());
        assert!(!orphan.exists());

        let connection = pool.acquire().await.unwrap();
        let mut clean = std::pin::pin!(clean_resource_files(&access, &db, || {
            Ok((resources.clone(), HashSet::from(["running.png".to_owned()])))
        }));
        assert!(matches!(clean.as_mut().poll(&mut context), Poll::Pending));
        let mut next_writer = std::pin::pin!(access.read_owned());
        assert!(matches!(
            next_writer.as_mut().poll(&mut context),
            Poll::Pending
        ));
        drop(connection);
        assert_eq!(clean.await.unwrap().files, 0);
        let _next_writer_guard = next_writer.await;
    }

    #[test]
    fn clean_sharded_files_removes_empty_shard_directories() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("clipboard-images").join("origin");
        let shard = root.join("ab");
        fs::create_dir_all(&shard).unwrap();
        fs::write(shard.join("abandoned.png"), b"x").unwrap();

        let mut removed = CleanCacheStats::default();

        clean_sharded_files(&root, &HashSet::new(), &mut removed).unwrap();

        assert_eq!(removed.files, 1);
        assert!(!shard.exists());
        assert!(!root.exists());
    }
}
