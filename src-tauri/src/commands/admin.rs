use serde_json::json;
use tauri::{AppHandle, Manager};

use crate::admin::{self, AdminLaunchStatus};
use crate::core::Result;
use crate::settings::change::{self, OnEffectFailure, SettingsWrite};
use crate::settings::{Settings, SettingsStore};

#[tauri::command]
pub async fn get_run_as_admin_status(app: AppHandle) -> Result<AdminLaunchStatus> {
    let configured = app.state::<SettingsStore>().snapshot().general.run_as_admin;

    Ok(admin::status(configured))
}

#[tauri::command]
pub async fn set_run_as_admin(app: AppHandle, enabled: bool) -> Result<Settings> {
    let patch = json!({
        "general": {
            "runAsAdmin": enabled,
        },
    });

    change::commit_patch(&app, patch).await
}

#[tauri::command]
pub async fn restart_as_admin(app: AppHandle) -> Result<()> {
    let previous = app.state::<SettingsStore>().snapshot();
    let patch = json!({
        "general": {
            "runAsAdmin": true,
        },
    });
    change::commit_patch(&app, patch).await?;

    if let Err(err) = admin::launch_elevated_current_process() {
        change::commit(
            &app,
            SettingsWrite::Restore(Box::new(previous)),
            OnEffectFailure::LogOnly,
        )
        .await?;
        return Err(err);
    }

    app.exit(0);

    Ok(())
}
