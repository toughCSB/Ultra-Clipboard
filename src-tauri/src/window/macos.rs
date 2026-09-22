//! macOS window management. The clipboard window uses an NSPanel, while other
//! windows use the regular show and hide path.

#![allow(clippy::unused_unit)]

use std::time::Duration;

use tauri::{AppHandle, Manager};
use tauri_nspanel::{
    tauri_panel, CollectionBehavior, ManagerExt, PanelLevel, StyleMask, WebviewWindowExt,
};

use super::{get_window, CLIPBOARD_WINDOW_LABEL, ONBOARDING_WINDOW_LABEL, PREFERENCE_WINDOW_LABEL};
use crate::core::Result;
use crate::settings::SettingsStore;

const CLIPBOARD_PANEL_SHOW_DELAY: Duration = Duration::from_millis(16);

tauri_panel! {
    panel!(MainPanel {
        config: {
            is_floating_panel: true,
            can_become_key_window: true,
            can_become_main_window: false
        }
    })

    panel_event!(MainPanelEventHandler {
        window_did_resign_key(notification: &NSNotification) -> (),
    })
}

pub fn register_plugin(app_handle: &AppHandle) {
    let _ = app_handle.plugin(tauri_nspanel::init());
}

pub fn setup_clipboard_panel(app_handle: &AppHandle) -> Result<()> {
    show_taskbar_icon(app_handle, false)?;

    let clipboard_window = get_window(app_handle, CLIPBOARD_WINDOW_LABEL)?;

    let panel = clipboard_window
        .to_panel::<MainPanel>()
        .map_err(|e| anyhow::anyhow!("to_panel failed: {e:?}"))?;

    panel.set_corner_radius(16.0);
    panel.set_level(PanelLevel::Dock.value());
    panel.set_style_mask(StyleMask::empty().resizable().nonactivating_panel().into());
    panel.set_collection_behavior(
        CollectionBehavior::new()
            .stationary()
            .move_to_active_space()
            .full_screen_auxiliary()
            .into(),
    );

    let handler = MainPanelEventHandler::new();

    let resign_handle = app_handle.clone();
    handler.window_did_resign_key(move |_| {
        if !super::should_auto_hide_clipboard_window() {
            return;
        }

        if let Err(err) = super::hide_window(&resign_handle, CLIPBOARD_WINDOW_LABEL) {
            log::warn!("auto-hide clipboard window on resign-key failed: {err}");
        }
    });

    panel.set_event_handler(Some(handler.as_ref()));

    Ok(())
}

pub fn show_window(app_handle: &AppHandle, label: &str) -> Result<()> {
    if label == CLIPBOARD_WINDOW_LABEL {
        show_clipboard_panel(app_handle)
    } else {
        let window = get_window(app_handle, label)?;
        window.show().map_err(|e| anyhow::anyhow!(e))?;
        window.unminimize().map_err(|e| anyhow::anyhow!(e))?;
        window.set_focus().map_err(|e| anyhow::anyhow!(e))?;
        Ok(())
    }
}

pub fn hide_window(app_handle: &AppHandle, label: &str) -> Result<()> {
    if label == CLIPBOARD_WINDOW_LABEL {
        hide_clipboard_panel(app_handle)
    } else {
        get_window(app_handle, label)?
            .hide()
            .map_err(|e| anyhow::anyhow!(e))?;
        Ok(())
    }
}

pub fn show_taskbar_icon(app_handle: &AppHandle, visible: bool) -> Result<()> {
    app_handle
        .set_dock_visibility(visible)
        .map_err(|e| anyhow::anyhow!(e))?;
    Ok(())
}

pub fn handle_reopen(app_handle: &AppHandle, has_visible_windows: bool) {
    if has_visible_windows {
        return;
    }

    if let Some(settings_store) = app_handle.try_state::<SettingsStore>() {
        if !settings_store.snapshot().onboarding.completed {
            if let Err(err) = super::show_window(app_handle, ONBOARDING_WINDOW_LABEL) {
                log::error!("show onboarding window on reopen failed: {err:?}");
            }
            return;
        }
    }

    if let Err(err) = super::show_window(app_handle, PREFERENCE_WINDOW_LABEL) {
        log::error!("show preference window on reopen failed: {err:?}");
    }
}

fn show_clipboard_panel(app_handle: &AppHandle) -> Result<()> {
    let handle = app_handle.clone();

    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(CLIPBOARD_PANEL_SHOW_DELAY).await;

        let panel_handle = handle.clone();
        if let Err(err) = handle.run_on_main_thread(move || {
            if let Ok(panel) = panel_handle.get_webview_panel(CLIPBOARD_WINDOW_LABEL) {
                panel.show_and_make_key();

                panel.set_collection_behavior(
                    CollectionBehavior::new()
                        .stationary()
                        .can_join_all_spaces()
                        .full_screen_auxiliary()
                        .into(),
                );
                super::preview::resume_after_clipboard_show();
                super::emit_visibility(&panel_handle, CLIPBOARD_WINDOW_LABEL, true);
                super::lifecycle::on_shown(&panel_handle, CLIPBOARD_WINDOW_LABEL);
            }
        }) {
            log::warn!("show clipboard panel on main thread failed: {err}");
        }
    });

    Ok(())
}

fn hide_clipboard_panel(app_handle: &AppHandle) -> Result<()> {
    let handle = app_handle.clone();
    app_handle
        .run_on_main_thread(move || {
            if let Ok(panel) = handle.get_webview_panel(CLIPBOARD_WINDOW_LABEL) {
                panel.hide();

                panel.set_collection_behavior(
                    CollectionBehavior::new()
                        .stationary()
                        .move_to_active_space()
                        .full_screen_auxiliary()
                        .into(),
                );
            }
        })
        .map_err(|e| anyhow::anyhow!(e))?;
    Ok(())
}

pub fn resign_clipboard_panel_key(app_handle: &AppHandle) -> Result<()> {
    let handle = app_handle.clone();
    app_handle
        .run_on_main_thread(move || {
            if let Ok(panel) = handle.get_webview_panel(CLIPBOARD_WINDOW_LABEL) {
                panel.resign_key_window();
            }
        })
        .map_err(|e| anyhow::anyhow!(e))?;
    Ok(())
}

pub fn make_clipboard_panel_key(app_handle: &AppHandle) -> Result<()> {
    let handle = app_handle.clone();
    app_handle
        .run_on_main_thread(move || {
            if let Ok(panel) = handle.get_webview_panel(CLIPBOARD_WINDOW_LABEL) {
                panel.make_key_window();
            }
        })
        .map_err(|e| anyhow::anyhow!(e))?;
    Ok(())
}
