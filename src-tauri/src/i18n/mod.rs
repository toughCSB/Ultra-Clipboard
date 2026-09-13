//! Rust-owned user-visible labels for tray menus, context menus, and command errors.
//! Internal logs and technical error context do not belong in this module.

pub mod clipboard_menu;
pub mod commands;
mod en_us;
mod keys;
mod ko_kr;
pub mod tray;

use tauri::{AppHandle, Manager};

use crate::settings::{Language, SettingsStore};

pub fn current_language(app: &AppHandle) -> Language {
    app.try_state::<SettingsStore>()
        .map(|s| s.snapshot().appearance.language)
        .unwrap_or_default()
}
