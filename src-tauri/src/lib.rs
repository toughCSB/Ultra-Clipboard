mod admin;
mod autostart;
mod backup;
mod clipboard;
mod commands;
mod core;
mod db;
mod drag_out;
mod i18n;
#[cfg(target_os = "windows")]
mod keyboard;
mod keystroke;
mod menu;
#[cfg(target_os = "windows")]
mod mouse;
mod settings;
mod shortcut;
mod sync;
mod tray;
mod update;
mod webdav;
mod window;

use tauri::{Manager, WindowEvent};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    admin::handle_startup_auto_elevation();

    let mut log_targets = vec![tauri_plugin_log::Target::new(
        tauri_plugin_log::TargetKind::LogDir { file_name: None },
    )];
    if cfg!(debug_assertions) {
        log_targets.push(tauri_plugin_log::Target::new(
            tauri_plugin_log::TargetKind::Stdout,
        ));
        log_targets.push(tauri_plugin_log::Target::new(
            tauri_plugin_log::TargetKind::Webview,
        ));
    }

    let log_plugin = tauri_plugin_log::Builder::new()
        .level(if cfg!(debug_assertions) {
            log::LevelFilter::Debug
        } else {
            log::LevelFilter::Info
        })
        .targets(log_targets)
        .build();

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(
            |app_handle, argv, _cwd| {
                if let Some(path) = backup::backup_path_from_args(&argv) {
                    if let Err(err) = backup::emit_received_backup(
                        app_handle,
                        path,
                        backup::BackupReceiveSource::OpenFile,
                    ) {
                        log::error!("receive backup from second instance failed: {err:?}");
                    }
                    return;
                }

                if autostart::is_autostart_launch(&argv) {
                    return;
                }

                if let Err(err) = show_default_foreground_window(app_handle) {
                    log::error!("show foreground window on second instance failed: {err:?}");
                }
            },
        ))
        .plugin(log_plugin)
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build());

    #[cfg(target_os = "macos")]
    let builder = builder.plugin(tauri_plugin_macos_permissions::init());

    builder
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(core::prevent_default::init())
        .invoke_handler(tauri::generate_handler![
            commands::get_run_as_admin_status,
            commands::set_run_as_admin,
            commands::restart_as_admin,
            commands::read_clipboard,
            commands::list_clipboard_items,
            commands::open_external_url,
            commands::list_clipboard_groups,
            commands::create_clipboard_group,
            commands::update_clipboard_group,
            commands::update_clipboard_groups_layout,
            commands::delete_clipboard_group,
            commands::import_clipboard_group_svg,
            commands::get_clipboard_item,
            commands::list_clipboard_apps,
            commands::list_all_apps,
            commands::add_clipboard_app_from_path,
            commands::delete_unreferenced_clipboard_apps,
            commands::get_clipboard_preview_payload,
            commands::play_copy_sound,
            commands::get_clipboard_image_path,
            commands::get_clipboard_app_icon_path,
            commands::save_clipboard_image_to_file,
            commands::get_file_icon_path,
            commands::write_to_clipboard,
            commands::paste_clipboard_item,
            commands::start_drag_clipboard_item,
            commands::toggle_clipboard_item_favorite,
            commands::toggle_clipboard_item_pinned,
            commands::delete_clipboard_item,
            commands::clear_clipboard_items,
            commands::update_clipboard_item_note,
            commands::update_clipboard_item_group,
            commands::open_clipboard_item_link,
            commands::reveal_clipboard_item,
            commands::show_window,
            commands::hide_window,
            commands::show_context_submenu,
            commands::hide_context_submenu,
            commands::hide_context_menus,
            commands::get_context_menu_payload,
            commands::get_context_submenu_payload,
            commands::toggle_window,
            commands::notify_window_ready,
            commands::set_window_dirty,
            commands::acquire_window_keepalive,
            commands::release_window_keepalive,
            commands::get_window_lifecycle_snapshot,
            commands::open_preference_with_highlight,
            commands::take_pending_preference_highlight,
            commands::open_update_window,
            commands::open_onboarding,
            commands::set_onboarding_step,
            commands::finish_onboarding,
            commands::detect_legacy_data,
            commands::import_legacy_data,
            commands::show_taskbar_icon,
            commands::position_window,
            commands::set_clipboard_window_pinned,
            commands::set_clipboard_window_auto_hide_suspended,
            commands::set_clipboard_window_editing,
            commands::show_clipboard_preview,
            commands::close_clipboard_preview,
            commands::get_clipboard_preview_state,
            commands::get_settings,
            commands::suspend_global_shortcuts,
            commands::resume_global_shortcuts,
            commands::update_settings,
            commands::reset_settings,
            commands::set_sync_peer_secret,
            commands::replace_sync_peer_secret,
            commands::delete_sync_peer_secret,
            commands::test_sync_peer_secret,
            commands::export_history_backup,
            commands::inspect_history_backup,
            commands::take_pending_backup,
            commands::import_history_backup,
            commands::push_webdav_backup,
            commands::pull_webdav_backup,
            commands::get_storage_usage,
            commands::get_storage_location,
            commands::change_storage_location,
            commands::reset_storage_location,
            commands::clean_resource_cache,
            commands::open_preference_directory,
            commands::get_autostart,
            commands::set_autostart,
            commands::get_update_status,
            commands::check_for_updates,
            commands::download_update,
            commands::install_update,
            commands::skip_update_version,
            menu::clipboard_item::popup_clipboard_item_menu,
        ])
        .on_menu_event(|app, event| {
            menu::clipboard_item::handle_menu_event(app, event.id().as_ref());
        })
        .setup(move |app| {
            let handle = app.handle().clone();

            #[cfg(target_os = "macos")]
            window::macos::register_plugin(&handle);

            let window_state_store = window::WindowStateStore::new(&handle).map_err(|err| {
                log::error!("window state store initialization failed: {err:?}");
                err
            })?;
            handle.manage(window_state_store);

            handle.manage(window::lifecycle::WindowLifecycleManager::new());
            update::init(&handle);

            let settings = settings::init(&handle).map_err(|err| {
                log::error!("settings initialization failed: {err:?}");
                err
            })?;

            let handle_db = handle.clone();
            tauri::async_runtime::block_on(async move {
                let pool = db::init(&handle_db).await.map_err(|err| {
                    log::error!("database initialization failed: {err:?}");
                    err
                })?;
                handle_db.manage(db::DatabaseState::new(pool));
                clipboard::init(&handle_db)?;
                if let Err(err) = sync::init(&handle_db).await {
                    log::warn!(
                        "clipboard sync initialization failed; continuing with sync stopped: {err}"
                    );
                }
                Ok::<_, anyhow::Error>(())
            })?;

            shortcut::init(&handle, &settings.shortcuts).map_err(|err| {
                log::error!("global shortcut initialization failed: {err:?}");
                err
            })?;

            autostart::init(&handle).map_err(|err| {
                log::error!("autostart initialization failed: {err:?}");
                err
            })?;
            if let Err(err) = autostart::sync_enabled(&handle, settings.general.auto_start) {
                log::warn!("autostart setting sync failed: {err}");
            }

            if let Err(err) = tray::init(&handle, &settings) {
                log::error!("tray initialization failed: {err:?}");
            }

            #[cfg(target_os = "macos")]
            if let Err(err) = window::macos::setup_clipboard_panel(&handle) {
                log::error!("setup clipboard NSPanel failed: {err:?}");
            }

            menu::clipboard_item::init(&handle);

            #[cfg(target_os = "windows")]
            menu::context_window::init(&handle);

            if !settings.onboarding.completed {
                if let Err(err) = window::open_onboarding(&handle) {
                    log::error!("open onboarding window failed: {err:?}");
                }
            }

            update::schedule_auto_check(&handle);

            #[cfg(target_os = "windows")]
            if let Some(path) = backup::backup_path_from_args(&std::env::args().collect::<Vec<_>>())
            {
                if let Err(err) = backup::emit_received_backup(
                    &handle,
                    path,
                    backup::BackupReceiveSource::OpenFile,
                ) {
                    log::error!("receive backup from launch args failed: {err:?}");
                }
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window::intercept_close_request(window) {
                    api.prevent_close();
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|app_handle, event| {
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Ready = &event {
                backup::mark_app_ready(app_handle);
            }

            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen {
                has_visible_windows,
                ..
            } = event
            {
                window::handle_reopen(app_handle, has_visible_windows);
            }

            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Opened { urls } = &event {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    for url in urls {
                        if url.scheme() != "file" {
                            continue;
                        }
                        let Ok(path) = url.to_file_path() else {
                            continue;
                        };
                        if !backup::is_backup_path(&path) {
                            continue;
                        }
                        backup::handle_open_file(
                            app_handle,
                            path,
                            backup::BackupReceiveSource::OpenFile,
                        );
                        break;
                    }
                }));

                if result.is_err() {
                    log::error!("panic while handling Opened event (caught at C boundary)");
                }
            }

            if let tauri::RunEvent::ExitRequested { .. } = event {
                window::save_all_window_states(app_handle);
            }
        });
}

fn show_default_foreground_window(app_handle: &tauri::AppHandle) -> core::Result<()> {
    if let Some(settings_store) = app_handle.try_state::<settings::SettingsStore>() {
        if !settings_store.snapshot().onboarding.completed {
            return window::open_onboarding(app_handle);
        }
    }

    window::show_window(app_handle, window::PREFERENCE_WINDOW_LABEL)
}
