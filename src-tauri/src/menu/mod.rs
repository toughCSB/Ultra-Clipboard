//! Clipboard item context menus use native muda on macOS and a non-focusable
//! custom WebView on Windows.

pub mod clipboard_item;

#[cfg(target_os = "windows")]
pub mod context_window;
