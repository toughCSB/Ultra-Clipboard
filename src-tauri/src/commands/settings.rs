use tauri::{AppHandle, Emitter, Manager};

use crate::core::Result;
use crate::settings::{Settings, SettingsStore};
use crate::{admin, autostart, shortcut, tray, window};

const SETTINGS_UPDATED_EVENT: &str = "settings://updated";

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
    let previous = app.state::<SettingsStore>().snapshot();
    let patch_obj = patch.as_object();
    let touches_shortcuts = patch_obj
        .map(|m| m.contains_key("shortcuts"))
        .unwrap_or(false);
    let touches_tray = patch_obj
        .map(|m| {
            m.get("general")
                .and_then(|v| v.as_object())
                .is_some_and(|g| g.contains_key("trayIcon"))
                || m.get("appearance")
                    .and_then(|v| v.as_object())
                    .is_some_and(|a| a.contains_key("language"))
        })
        .unwrap_or(false);
    let touches_run_as_admin = patch_obj
        .map(|m| {
            m.get("general")
                .and_then(|v| v.as_object())
                .is_some_and(|g| g.contains_key("runAsAdmin"))
        })
        .unwrap_or(false);
    let touches_sync = patch_obj
        .map(|map| map.contains_key("sync"))
        .unwrap_or(false);

    let next = app.state::<SettingsStore>().update(patch)?;

    if touches_shortcuts {
        if let Err(err) = shortcut::apply(&app, &next.shortcuts) {
            log::warn!("re-apply shortcuts after settings update failed: {err}");
        }
    }

    if touches_tray {
        if let Err(err) = tray::apply(&app, &next) {
            log::warn!("re-apply tray after settings update failed: {err}");
        }
    }

    if touches_run_as_admin {
        admin::sync_scheduled_task(next.general.run_as_admin);
    }

    if touches_sync {
        if let Err(err) = crate::sync::reconfigure(&app).await {
            app.state::<SettingsStore>().restore(previous)?;
            if let Err(restore_err) = crate::sync::reconfigure(&app).await {
                log::warn!("restore clipboard sync after settings rollback failed: {restore_err}");
            }
            return Err(err);
        }
    }

    emit_settings_updated(&app, &next);

    Ok(next)
}

#[tauri::command]
pub async fn reset_settings(app: AppHandle) -> Result<Settings> {
    let previous = app.state::<SettingsStore>().snapshot();
    let next = app.state::<SettingsStore>().reset()?;

    apply_reset_side_effects(&app, &next);
    if let Err(err) = crate::sync::reconfigure(&app).await {
        app.state::<SettingsStore>().restore(previous.clone())?;
        apply_reset_side_effects(&app, &previous);
        if let Err(restore_err) = crate::sync::reconfigure(&app).await {
            log::warn!(
                "restore clipboard sync after settings reset rollback failed: {restore_err}"
            );
        }
        return Err(err);
    }
    emit_settings_updated(&app, &next);

    Ok(next)
}

fn apply_reset_side_effects(app: &AppHandle, settings: &Settings) {
    if let Err(err) = autostart::set_enabled(app, settings.general.auto_start) {
        log::warn!("reset autostart failed: {err}");
    }

    if let Err(err) = shortcut::apply(app, &settings.shortcuts) {
        log::warn!("reset shortcuts failed: {err}");
    }

    if let Err(err) = tray::apply(app, settings) {
        log::warn!("reset tray failed: {err}");
    }

    if let Err(err) = window::show_taskbar_icon(app, settings.general.dock_icon) {
        log::warn!("reset taskbar icon failed: {err}");
    }

    admin::sync_scheduled_task(settings.general.run_as_admin);
}

pub(crate) fn emit_settings_updated(app: &AppHandle, settings: &Settings) {
    if let Err(err) = app.emit(SETTINGS_UPDATED_EVENT, settings) {
        log::warn!("emit settings updated event failed: {err}");
    }
}
