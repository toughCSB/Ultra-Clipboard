//! OS-level drag-out for files, images, and text.
//!
//! macOS uses a local implementation so preview pixel density stays separate
//! from display size. Windows uses `drag` for files and a custom data object
//! for text and rich text. Every entry point runs on the owning window thread.

use std::path::PathBuf;

use tauri::WebviewWindow;

use crate::core::{AppError, Result};

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "windows")]
mod windows_ghost;

/// Starts a file drag. `preview_png` is preferred; when absent, the first path
/// is used for platform fallback preview generation.
pub fn start_drag_files(
    window: &WebviewWindow,
    paths: Vec<PathBuf>,
    preview_png: Option<Vec<u8>>,
) -> Result<()> {
    start_drag_files_with_ghost(window, paths, preview_png, true)
}

/// Same as [`start_drag_files`], but on Windows `with_ghost` controls whether
/// the window's WebView2 drop target is wrapped for the OLE ghost preview.
/// That wrapping reinterprets an internal WebView2 window property as a raw
/// COM pointer, so callers unsure of a window's WebView2 setup (freshly
/// created, borderless windows) can pass `false` to keep only the file
/// transfer itself. macOS ignores this flag; its preview path is separate.
pub fn start_drag_files_with_ghost(
    window: &WebviewWindow,
    paths: Vec<PathBuf>,
    preview_png: Option<Vec<u8>>,
    with_ghost: bool,
) -> Result<()> {
    if paths.is_empty() {
        return Err(AppError::Clipboard("drag-out: empty path list".to_string()));
    }

    #[cfg(target_os = "macos")]
    {
        let _ = with_ghost;
        macos::start_drag_files(window, paths, preview_png, |result| {
            log::debug!("drag-out finished: {result:?}");
        })
    }

    #[cfg(target_os = "windows")]
    {
        windows::start_drag_files_with_ghost(window, paths, preview_png, with_ghost)
    }
}

/// Starts a text drag with plain text and optional HTML or RTF representations.
pub fn start_drag_text(
    window: &WebviewWindow,
    plain: String,
    html: Option<String>,
    rtf: Option<String>,
    preview_png: Option<Vec<u8>>,
) -> Result<()> {
    if plain.is_empty() {
        return Err(AppError::Clipboard("drag-out: empty text".to_string()));
    }

    #[cfg(target_os = "macos")]
    {
        macos::start_drag_text(window, plain, html, rtf, preview_png, |result| {
            log::debug!("drag-out finished: {result:?}");
        })
    }

    #[cfg(target_os = "windows")]
    {
        windows::start_drag_text(window, &plain, html.as_deref(), rtf.as_deref(), preview_png)
    }
}
