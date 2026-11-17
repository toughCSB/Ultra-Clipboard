use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;

use crate::core::Result;
use crate::db::DatabaseState;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageUsage {
    pub total_bytes: u64,
    pub database_bytes: u64,
    pub resources_bytes: u64,
    pub settings_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanCacheResult {
    pub removed_files: u64,
    pub removed_bytes: u64,
    pub storage_usage: StorageUsage,
}

pub type StorageLocation = crate::core::paths::StorageLocation;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeStorageLocationResult {
    pub location: StorageLocation,
    pub storage_usage: StorageUsage,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PreferenceDirectoryTarget {
    Data,
    Logs,
}

#[tauri::command]
pub async fn get_storage_location(app: AppHandle) -> Result<StorageLocation> {
    crate::core::paths::storage_location(&app)
}

#[tauri::command]
pub async fn get_storage_usage(app: AppHandle) -> Result<StorageUsage> {
    let total_bytes = dir_size(&crate::core::paths::app_data_dir(&app)?)?;
    let database_bytes = database_bytes(&app)?;
    let resources_bytes = dir_size(&crate::core::paths::resources_dir(&app)?)?;
    let settings_bytes = settings_bytes(&app)?;

    Ok(StorageUsage {
        total_bytes,
        database_bytes,
        resources_bytes,
        settings_bytes,
    })
}

#[tauri::command]
pub async fn change_storage_location(
    app: AppHandle,
    db: tauri::State<'_, DatabaseState>,
    target_parent_dir: String,
) -> Result<ChangeStorageLocationResult> {
    let target = crate::core::paths::custom_data_dir(Path::new(&target_parent_dir));
    switch_storage_location(app, db.inner(), target).await
}

#[tauri::command]
pub async fn reset_storage_location(
    app: AppHandle,
    db: tauri::State<'_, DatabaseState>,
) -> Result<ChangeStorageLocationResult> {
    let target = crate::core::paths::default_data_dir(&app)?;
    switch_storage_location(app, db.inner(), target).await
}

#[tauri::command]
pub async fn clean_resource_cache(
    app: AppHandle,
    db: tauri::State<'_, DatabaseState>,
) -> Result<CleanCacheResult> {
    let access = app.state::<crate::storage::StorageAccess>();
    let removed = crate::storage::resources::clean_resource_files(&access, &db, || {
        let resources_dir = crate::core::paths::resources_dir(&app)?;
        let cached_icons = app
            .try_state::<crate::clipboard::AppsRegistry>()
            .map(|registry| registry.cached_icon_files())
            .unwrap_or_default();
        Ok((resources_dir, cached_icons))
    })
    .await?;

    Ok(CleanCacheResult {
        removed_files: removed.files,
        removed_bytes: removed.bytes,
        storage_usage: get_storage_usage(app).await?,
    })
}

#[tauri::command]
pub async fn open_preference_directory(
    app: AppHandle,
    target: PreferenceDirectoryTarget,
) -> Result<()> {
    let path = match target {
        PreferenceDirectoryTarget::Data => crate::core::paths::app_data_dir(&app)?,
        PreferenceDirectoryTarget::Logs => app
            .path()
            .app_log_dir()
            .context("failed to resolve app log dir")?,
    };

    fs::create_dir_all(&path).with_context(|| format!("failed to create directory {path:?}"))?;
    let path_str = path
        .to_str()
        .ok_or_else(|| anyhow::Error::msg("directory path is not valid utf-8"))?;

    app.opener()
        .open_path(path_str, None::<&str>)
        .map_err(|err| anyhow::Error::msg(err.to_string()))?;

    Ok(())
}

async fn switch_storage_location(
    app: AppHandle,
    db: &DatabaseState,
    target: PathBuf,
) -> Result<ChangeStorageLocationResult> {
    crate::storage::change_location::change_location(&app, db, target).await?;
    Ok(ChangeStorageLocationResult {
        location: crate::core::paths::storage_location(&app)?,
        storage_usage: get_storage_usage(app).await?,
    })
}
fn database_bytes(app: &AppHandle) -> Result<u64> {
    let db_path = crate::db::db_path(app)?;
    let mut total = file_size(&db_path)?;

    for suffix in ["-wal", "-shm"] {
        total += file_size(Path::new(&format!("{}{}", db_path.display(), suffix)))?;
    }

    Ok(total)
}

fn settings_bytes(app: &AppHandle) -> Result<u64> {
    let settings_path = crate::core::paths::config_dir(app)?.join("settings.json");

    file_size(&settings_path)
}

fn file_size(path: &Path) -> Result<u64> {
    if !path.exists() {
        return Ok(0);
    }

    Ok(fs::metadata(path)
        .with_context(|| format!("failed to read metadata at {path:?}"))?
        .len())
}

fn dir_size(path: &Path) -> Result<u64> {
    if !path.exists() {
        return Ok(0);
    }

    let mut total = 0;
    for entry in
        fs::read_dir(path).with_context(|| format!("failed to read directory at {path:?}"))?
    {
        let entry = entry.with_context(|| format!("failed to read entry under {path:?}"))?;
        let entry_path = entry.path();
        let metadata = entry
            .metadata()
            .with_context(|| format!("failed to read metadata at {entry_path:?}"))?;

        if metadata.is_dir() {
            total += dir_size(&entry_path)?;
            continue;
        }

        total += metadata.len();
    }

    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::change_location::{
        remove_bootstrap_storage_payload, remove_custom_storage_root,
    };

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "ultra-clipboard-clean-cache-{}",
                uuid::Uuid::new_v4()
            ));
            fs::create_dir_all(&path).unwrap();

            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).ok();
        }
    }

    #[test]
    fn bootstrap_storage_cleanup_keeps_only_manifest() {
        let temp = TempDir::new();
        fs::write(temp.path().join("storage.json"), "{}").unwrap();
        fs::write(temp.path().join(".ultra-clipboard-storage.json"), "{}").unwrap();
        fs::create_dir_all(temp.path().join("db")).unwrap();
        fs::write(temp.path().join("db").join("clipboard.db"), b"db").unwrap();
        fs::create_dir_all(temp.path().join("resources")).unwrap();
        fs::write(temp.path().join("resources").join("image.png"), b"image").unwrap();

        remove_bootstrap_storage_payload(temp.path()).unwrap();

        assert!(temp.path().join("storage.json").exists());
        assert!(!temp.path().join(".ultra-clipboard-storage.json").exists());
        assert!(!temp.path().join("db").exists());
        assert!(!temp.path().join("resources").exists());
    }

    #[test]
    fn remove_custom_storage_root_deletes_empty_container() {
        let temp = TempDir::new();
        let root = temp.path().join("UltraClipboardData").join("dev");
        fs::create_dir_all(root.join("config")).unwrap();
        fs::write(root.join("config").join("settings.json"), "{}").unwrap();

        remove_custom_storage_root(&root).unwrap();

        assert!(!root.exists());
        assert!(!temp.path().join("UltraClipboardData").exists());
    }

    #[test]
    fn remove_custom_storage_root_keeps_container_with_sibling_environment() {
        let temp = TempDir::new();
        let container = temp.path().join("UltraClipboardData");
        let root = container.join("dev");
        fs::create_dir_all(root.join("config")).unwrap();
        fs::write(root.join("config").join("settings.json"), "{}").unwrap();
        fs::create_dir_all(container.join("prod")).unwrap();

        remove_custom_storage_root(&root).unwrap();

        assert!(!root.exists());
        assert!(container.exists());
        assert!(container.join("prod").exists());
    }
}
