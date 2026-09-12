use std::io::Write;

use anyhow::{anyhow, Context};
use tauri::AppHandle;
use tempfile::Builder;

use crate::backup::{
    BackupExportMode, BackupImportStrategy, ExportHistoryBackupOptions, ImportHistoryBackupInput,
    ImportHistoryBackupOptions, ImportHistoryBackupResult,
};
use crate::core::Result;
use crate::db::DatabaseState;
use crate::settings::SettingsStore;
use crate::webdav::{self, WebDavSyncResult};

fn require_configured(store: &SettingsStore) -> Result<crate::settings::WebDav> {
    let settings = store.snapshot().webdav;
    if !settings.enabled {
        return Err(anyhow!("WebDAV sync is turned off").into());
    }
    if settings.url.trim().is_empty() {
        return Err(anyhow!("WebDAV URL is empty").into());
    }
    if settings.username.trim().is_empty() {
        return Err(anyhow!("WebDAV username is empty").into());
    }
    Ok(settings)
}

#[tauri::command]
pub async fn push_webdav_backup(
    app: AppHandle,
    db: tauri::State<'_, DatabaseState>,
    store: tauri::State<'_, SettingsStore>,
) -> Result<WebDavSyncResult> {
    let webdav = require_configured(&store)?;
    let pool = db.pool().await;
    let temp = Builder::new()
        .prefix("ultra-clipboard-webdav-")
        .suffix(".ecopastebak")
        .tempfile()
        .context("failed to create WebDAV upload temp file")?;
    let target = temp.path().to_string_lossy().into_owned();

    crate::backup::export_history_backup(
        &app,
        &pool,
        target,
        ExportHistoryBackupOptions {
            mode: BackupExportMode::Plain,
            password: None,
        },
    )
    .await?;

    let body = std::fs::read(temp.path()).context("failed to read WebDAV upload payload")?;
    webdav::put_bytes(&webdav, body).await
}

#[tauri::command]
pub async fn pull_webdav_backup(
    app: AppHandle,
    db: tauri::State<'_, DatabaseState>,
    store: tauri::State<'_, SettingsStore>,
) -> Result<ImportHistoryBackupResult> {
    let webdav = require_configured(&store)?;
    let (body, _) = webdav::get_bytes(&webdav).await?;
    let mut temp = Builder::new()
        .prefix("ultra-clipboard-webdav-")
        .suffix(".ecopastebak")
        .tempfile()
        .context("failed to create WebDAV download temp file")?;
    temp.write_all(&body)
        .context("failed to write WebDAV download payload")?;
    temp.flush()
        .context("failed to flush WebDAV download payload")?;

    crate::backup::import_history_backup(
        &app,
        &db,
        ImportHistoryBackupInput {
            path: temp.path().to_string_lossy().into_owned(),
            password: None,
        },
        ImportHistoryBackupOptions {
            strategy: BackupImportStrategy::Merge,
        },
    )
    .await
}
