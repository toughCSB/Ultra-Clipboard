pub fn init() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    #[cfg(debug_assertions)]
    {
        tauri_plugin_prevent_default::debug()
    }

    #[cfg(not(debug_assertions))]
    tauri_plugin_prevent_default::init()
}
