//! Every event name Rust emits. Each constant name and value matches a key in
//! `src/constants/events.ts`; `core::ipc_contract` tests enforce it.
//!
//! An entry emitted on one platform only carries `#[cfg(any(<platform>, test))]`
//! so the other platform has no dead constant; `test` keeps it in `ALL`.

macro_rules! events {
    ($($(#[$meta:meta])* $name:ident = $value:literal,)*) => {
        $($(#[$meta])* pub const $name: &str = $value;)*

        #[cfg(test)]
        pub const ALL: &[(&str, &str)] = &[$((stringify!($name), $name),)*];
    };
}

events! {
    BACKUP_RECEIVED = "backup://received",
    CLIPBOARD_GROUPS_UPDATED = "clipboard-groups://updated",
    /// Rust emits this only from the macOS native menu; Windows menus are webviews.
    #[cfg(any(target_os = "macos", test))]
    CLIPBOARD_MENU_ACTION = "clipboard://menu-action",
    CLIPBOARD_UPDATED = "clipboard://updated",
    /// Windows webview context menus only.
    #[cfg(any(target_os = "windows", test))]
    CONTEXT_MENU_SHOW = "context-menu://show",
    /// Windows webview context menus only.
    #[cfg(any(target_os = "windows", test))]
    CONTEXT_SUBMENU_SHOW = "context-submenu://show",
    /// Windows low-level keyboard hook only.
    #[cfg(any(target_os = "windows", test))]
    KEYBOARD_NAV = "keyboard://nav",
    PREFERENCE_HIGHLIGHT_SETTING = "preference://highlight-setting",
    PREVIEW_UPDATED = "preview://updated",
    SCREENSHOT_EDITOR_CLOSE_REQUESTED = "screenshot://editor-close-requested",
    SCREENSHOT_OVERLAY_SESSION = "screenshot://overlay-session",
    SETTINGS_UPDATED = "settings://updated",
    SHORTCUT_CONFLICT = "shortcut://conflict",
    UPDATE_PROGRESS = "update://progress",
    WINDOW_BEFORE_DESTROY = "window://before-destroy",
    WINDOW_LIFECYCLE = "window://lifecycle",
    WINDOW_VISIBILITY = "window://visibility",
}
