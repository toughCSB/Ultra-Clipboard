use tauri::{AppHandle, Manager};

use crate::core::Result;
use crate::settings::change::{self, OnEffectFailure, SettingsWrite};
use crate::settings::{Settings, SettingsStore};
use crate::shortcut;

#[tauri::command]
pub async fn get_settings(app: AppHandle) -> Result<Settings> {
    Ok(app.state::<SettingsStore>().snapshot())
}

#[tauri::command]
pub async fn suspend_global_shortcuts(app: AppHandle) -> Result<()> {
    shortcut::suspend(&app)
}

#[tauri::command]
pub async fn resume_global_shortcuts(app: AppHandle) -> Result<()> {
    shortcut::resume(&app)
}

#[tauri::command]
pub async fn update_settings(app: AppHandle, patch: serde_json::Value) -> Result<Settings> {
    change::commit_patch(&app, patch).await
}

#[tauri::command]
pub async fn reset_settings(app: AppHandle) -> Result<Settings> {
    change::commit(&app, SettingsWrite::Reset, OnEffectFailure::RollBack).await
}
