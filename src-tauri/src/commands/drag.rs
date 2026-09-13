//! Commands that drag clipboard items from the clipboard window into external applications.

use std::path::PathBuf;

use sqlx::SqlitePool;
use tauri::{AppHandle, State};

use crate::clipboard::{icon_png, FileIconStore, ImageStore};
use crate::core::{AppError, Result};
use crate::db::items::find_item_by_id;
use crate::db::models::{ClipboardItem, ClipboardKind, ClipboardSubKind, Platform};
use crate::db::DatabaseState;
use crate::drag_out;
use crate::settings::Language;
use crate::window::{self, CLIPBOARD_WINDOW_LABEL};

enum DragPayload {
    /// File paths from `Files` or `Image` items.
    Files(Vec<PathBuf>),

    /// Plain text with optional HTML or RTF representations.
    Text {
        plain: String,
        html: Option<String>,
        rtf: Option<String>,
    },
}

#[tauri::command]
pub async fn start_drag_clipboard_item(
    app: AppHandle,
    db: State<'_, DatabaseState>,
    store: State<'_, ImageStore>,
    file_icon_store: State<'_, FileIconStore>,
    id: String,
) -> Result<()> {
    let pool = db.pool().await;
    let item = find_item_by_id(&pool, &id)
        .await?
        .ok_or_else(|| AppError::Clipboard(format!("clipboard item not found: {id}")))?;

    let lang = crate::i18n::current_language(&app);
    let payload = resolve_drag_payload(&item, &store, lang)?;

    let window = window::get_window(&app, CLIPBOARD_WINDOW_LABEL)?;

    let preview = build_preview(&item, &payload, &store, &file_icon_store, &pool).await;

    #[cfg(target_os = "macos")]
    {
        let window = window.clone();
        app.run_on_main_thread(move || {
            if let Err(err) = dispatch_drag(&window, payload, preview) {
                log::error!("start drag (macos) failed: {err}");
            }
        })
        .map_err(|err| AppError::Clipboard(format!("dispatch to main thread failed: {err}")))?;
    }

    #[cfg(target_os = "windows")]
    {
        let (tx, rx) = std::sync::mpsc::channel();
        app.run_on_main_thread(move || {
            let _ = tx.send(dispatch_drag(&window, payload, preview));
        })
        .map_err(|err| AppError::Clipboard(format!("dispatch to main thread failed: {err}")))?;

        rx.recv()
            .map_err(|err| AppError::Clipboard(format!("drag result channel closed: {err}")))??;
    }

    Ok(())
}

fn dispatch_drag(
    window: &tauri::WebviewWindow,
    payload: DragPayload,
    preview: Option<Vec<u8>>,
) -> Result<()> {
    match payload {
        DragPayload::Files(paths) => drag_out::start_drag_files(window, paths, preview),
        DragPayload::Text { plain, html, rtf } => {
            drag_out::start_drag_text(window, plain, html, rtf, preview)
        }
    }
}

/// Resolves an item into the platform-independent payload used by drag-out.
fn resolve_drag_payload(
    item: &ClipboardItem,
    store: &ImageStore,
    lang: Language,
) -> Result<DragPayload> {
    use crate::i18n::commands::Key;

    match item.kind {
        ClipboardKind::Files => {
            let paths: Vec<PathBuf> = item
                .content
                .split('\n')
                .filter(|s| !s.is_empty())
                .map(PathBuf::from)
                .filter(|p| p.exists())
                .collect();

            if paths.is_empty() {
                return Err(AppError::Clipboard(
                    crate::i18n::commands::label(lang, Key::DragSourceFilesMissing).to_string(),
                ));
            }
            Ok(DragPayload::Files(paths))
        }
        ClipboardKind::Image => {
            let path = store.origin_path(&item.content);
            if !path.exists() {
                return Err(AppError::Clipboard(
                    crate::i18n::commands::label(lang, Key::DragImageMissing).to_string(),
                ));
            }
            Ok(DragPayload::Files(vec![path]))
        }
        ClipboardKind::Text => {
            if item.content.is_empty() {
                return Err(AppError::Clipboard(
                    crate::i18n::commands::label(lang, Key::DragTextEmpty).to_string(),
                ));
            }

            // For HTML and RTF, `content` is the rich source and `search_text` is plain text.
            // Other text subtypes use `content` as their plain representation.
            let (plain, html, rtf) = match item.sub_kind {
                Some(ClipboardSubKind::Html) => (
                    item.search_text
                        .clone()
                        .unwrap_or_else(|| item.content.clone()),
                    Some(item.content.clone()),
                    None,
                ),
                Some(ClipboardSubKind::Rtf) => (
                    item.search_text
                        .clone()
                        .unwrap_or_else(|| item.content.clone()),
                    None,
                    Some(item.content.clone()),
                ),
                _ => (item.content.clone(), None, None),
            };
            Ok(DragPayload::Text { plain, html, rtf })
        }
    }
}

async fn build_preview(
    item: &ClipboardItem,
    payload: &DragPayload,
    image_store: &ImageStore,
    file_icon_store: &FileIconStore,
    pool: &SqlitePool,
) -> Option<Vec<u8>> {
    match payload {
        DragPayload::Text { .. } => None,
        DragPayload::Files(paths) => {
            if matches!(item.kind, ClipboardKind::Image) {
                if let Some(bytes) = read_image_thumbnail(image_store, &item.content) {
                    return Some(bytes);
                }
                // Do not synchronously decode a missing thumbnail during drag startup.
            }
            read_cached_file_icon(pool, file_icon_store, &paths[0])
                .await
                .or_else(|| icon_png(&paths[0], None))
        }
    }
}

fn read_image_thumbnail(store: &ImageStore, file_name: &str) -> Option<Vec<u8>> {
    let path = store.thumbnail_path(file_name);
    match std::fs::read(&path) {
        Ok(b) => Some(b),
        Err(err) => {
            log::debug!("thumbnail miss for drag preview {}: {err}", path.display());
            None
        }
    }
}

async fn read_cached_file_icon(
    pool: &SqlitePool,
    store: &FileIconStore,
    path: &std::path::Path,
) -> Option<Vec<u8>> {
    let cache_key = crate::clipboard::get_icon_cache_key(path);
    let platform = current_platform();
    let icon_file = match crate::db::file_icons::get_icon(pool, &cache_key, platform).await {
        Ok(Some(name)) => name,
        Ok(None) => return None,
        Err(err) => {
            log::warn!("query file_type_icons failed for drag preview: {err}");
            return None;
        }
    };
    let icon_path = store.icon_path(&icon_file);
    match std::fs::read(&icon_path) {
        Ok(b) => Some(b),
        Err(err) => {
            log::warn!(
                "read cached file icon failed {}: {err}",
                icon_path.display()
            );
            None
        }
    }
}

fn current_platform() -> Platform {
    #[cfg(target_os = "macos")]
    {
        Platform::Macos
    }
    #[cfg(target_os = "windows")]
    {
        Platform::Windows
    }
}
