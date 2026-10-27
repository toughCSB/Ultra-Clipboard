use serde_json::json;
use tauri::AppHandle;

use crate::backup::legacy::{
    self, LegacyImportSelection, OnboardingLegacyDataDetection, OnboardingLegacyImportResult,
};
use crate::core::Result;
use crate::settings::change;
use crate::settings::Settings;
use crate::window;

const MAX_ONBOARDING_STEP: u32 = 20;
#[tauri::command]
pub async fn open_onboarding(app: AppHandle) -> Result<()> {
    window::open_onboarding(&app)
}

#[tauri::command]
pub async fn set_onboarding_step(app: AppHandle, step: u32) -> Result<Settings> {
    change::commit_patch(
        &app,
        json!({
            "onboarding": {
                "lastStep": step.min(MAX_ONBOARDING_STEP),
            },
        }),
    )
    .await
}

#[tauri::command]
pub async fn finish_onboarding(app: AppHandle) -> Result<Settings> {
    let next = change::commit_patch(
        &app,
        json!({
            "onboarding": {
                "completed": true,
                "lastStep": 0,
            },
        }),
    )
    .await?;

    if let Err(err) = window::hide_window(&app, window::ONBOARDING_WINDOW_LABEL) {
        log::warn!("hide onboarding window after finish failed: {err}");
    }

    window::show_window(&app, window::CLIPBOARD_WINDOW_LABEL)?;

    Ok(next)
}

#[tauri::command]
pub async fn detect_legacy_data(app: AppHandle) -> Result<OnboardingLegacyDataDetection> {
    let detection = legacy::inspect_legacy_data().await?;

    change::commit_patch(
        &app,
        json!({
            "onboarding": {
                "legacyImport": {
                    "checked": true,
                },
            },
        }),
    )
    .await?;

    Ok(detection)
}

#[tauri::command]
pub async fn import_legacy_data(
    app: AppHandle,
    types: Vec<LegacyImportSelection>,
) -> Result<OnboardingLegacyImportResult> {
    let result = legacy::import_legacy_rows(&app, types).await?;
    let imported = result.imported > 0;

    change::commit_patch(
        &app,
        json!({
            "onboarding": {
                "legacyImport": {
                    "checked": true,
                    "imported": imported,
                    "importTypes": result.selected_types,
                    "importedAt": result.imported_at,
                },
            },
        }),
    )
    .await?;

    crate::clipboard::emit_clipboard_updated(
        &app,
        crate::clipboard::ClipboardUpdated::Imported { imported },
    );

    Ok(result)
}
