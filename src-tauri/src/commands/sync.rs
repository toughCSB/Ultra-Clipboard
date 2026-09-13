use tauri::{AppHandle, Manager};

use anyhow::anyhow;

use crate::core::AppError;
use crate::core::Result;
use crate::settings::SettingsStore;
use crate::sync::{self, SyncRuntime};

#[tauri::command]
pub async fn set_sync_peer_secret(app: AppHandle, peer_id: String, secret: String) -> Result<()> {
    set_or_replace(&app, &peer_id, &secret).await
}

#[tauri::command]
pub async fn replace_sync_peer_secret(
    app: AppHandle,
    peer_id: String,
    secret: String,
) -> Result<()> {
    set_or_replace(&app, &peer_id, &secret).await
}

#[tauri::command]
pub async fn delete_sync_peer_secret(app: AppHandle, peer_id: String) -> Result<()> {
    if app
        .state::<SettingsStore>()
        .snapshot()
        .sync
        .peers
        .iter()
        .any(|peer| peer.id == peer_id)
    {
        return Err(AppError::Other(anyhow!(
            "remove the sync peer before deleting its credential"
        )));
    }
    let reference = peer_reference(&peer_id)?;
    let runtime = app.state::<SyncRuntime>().inner().clone();
    runtime.delete_peer_secret(&reference)?;
    Ok(())
}

#[tauri::command]
pub async fn test_sync_peer_secret(app: AppHandle, peer_id: String) -> Result<()> {
    let reference = peer_reference(&peer_id)?;
    app.state::<SyncRuntime>().test_peer_secret(&reference)
}

async fn set_or_replace(app: &AppHandle, peer_id: &str, secret: &str) -> Result<()> {
    let reference = peer_reference(peer_id)?;
    app.state::<SyncRuntime>()
        .set_peer_secret(&reference, secret)?;
    if app
        .state::<SettingsStore>()
        .snapshot()
        .sync
        .peers
        .iter()
        .any(|peer| peer.id == peer_id)
    {
        sync::reconfigure(app).await?;
    }
    Ok(())
}

fn peer_reference(peer_id: &str) -> Result<String> {
    let parsed = uuid::Uuid::parse_str(peer_id)
        .map_err(|_| AppError::Other(anyhow!("sync peer credential operation failed")))?;
    if parsed.to_string() != peer_id {
        return Err(AppError::Other(anyhow!(
            "sync peer credential operation failed"
        )));
    }
    Ok(format!("sync-peer:{peer_id}"))
}
