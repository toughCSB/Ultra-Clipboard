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
    if paths.is_empty() {
        return Err(AppError::Clipboard("drag-out: empty path list".to_string()));
    }

    #[cfg(target_os = "macos")]
    {
        macos::start_drag_files(window, paths, preview_png, |result| {
            log::debug!("drag-out finished: {result:?}");
        })
    }

    #[cfg(target_os = "windows")]
    {
        windows::start_drag_files(window, paths, preview_png)
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
