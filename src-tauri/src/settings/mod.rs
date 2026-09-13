//! Application settings are owned by `SettingsStore`; the frontend accesses
//! them through the `get_settings` and `update_settings` commands.

mod model;
mod store;

pub use model::*;
pub use store::SettingsStore;

use tauri::{AppHandle, Manager};

use crate::core::Result;

pub fn init(app: &AppHandle) -> Result<Settings> {
    let store = SettingsStore::new(app)?;
    let initial = store.snapshot();
    app.manage(store);
    Ok(initial)
}
