use tauri::AppHandle;

use crate::autostart;
use crate::core::Result;

#[tauri::command]
pub async fn get_autostart(app: AppHandle) -> Result<bool> {
    autostart::is_enabled(&app)
}
