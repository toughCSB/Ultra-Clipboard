//! Output actions for edited screenshots. The editor renders the final pixels and
//! sends raw RGBA; Rust encodes once and routes the result to clipboard, file,
//! pin, or drag-out.

use std::io::Cursor;
use std::path::PathBuf;
use std::sync::Arc;

use chrono::{DateTime, Local};
use clipboard_rs::common::RustImage;
use clipboard_rs::{Clipboard, ClipboardContext, RustImageData};
use image::{ImageBuffer, ImageFormat, Rgba};
use serde::Serialize;
use tauri::ipc::{InvokeBody, Request};
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;

use super::pin;
use super::state::{CapturedImage, DragFile, ScreenshotState};
use crate::clipboard::{
    build_item_with_settings, maybe_play_copy, persist_and_notify, write_to_clipboard,
    ClipboardPayload, ImagePayload, ImageStore, WatcherPause, WritebackGuard,
};
use crate::core::{AppError, Result};
use crate::db::DatabaseState;
use crate::i18n::screenshot_menu::{self, Key};
use crate::settings::SettingsStore;

const LABEL_HEADER: &str = "x-screenshot-label";
const ACTION_HEADER: &str = "x-screenshot-action";
const WIDTH_HEADER: &str = "x-screenshot-width";
const HEIGHT_HEADER: &str = "x-screenshot-height";
const OFFSET_X_HEADER: &str = "x-screenshot-offset-x";
const OFFSET_Y_HEADER: &str = "x-screenshot-offset-y";

/// Largest side a WebView canvas can hand over reliably.
const MAX_EXPORT_SIDE: u32 = 32_767;
const DRAG_DIR: &str = "screenshot-drag";
const DRAG_PREVIEW_MAX: u32 = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportAction {
    Copy,
    Save,
    Pin,
    PrepareDrag,
    /// Recognizes text in the pixels and copies it.
    Ocr,
}

impl ExportAction {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "copy" => Some(Self::Copy),
            "save" => Some(Self::Save),
            "pin" => Some(Self::Pin),
            "prepareDrag" => Some(Self::PrepareDrag),
            "ocr" => Some(Self::Ocr),
            _ => None,
        }
    }
}

pub struct ExportRequest {
    pub label: String,
    pub action: ExportAction,
    pub width: u32,
    pub height: u32,
    /// Offset of the exported area inside the editor image, in image pixels.
    pub offset_x: i32,
    pub offset_y: i32,
    pub rgba: Vec<u8>,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub saved_path: Option<String>,
    pub history_recorded: bool,
    /// Recognized text, copied to the clipboard when it is not empty.
    pub text: Option<String>,
}

pub fn parse_request(request: &Request<'_>) -> Result<ExportRequest> {
    let InvokeBody::Raw(body) = request.body() else {
        return Err(invalid("screenshot export must send raw pixels"));
    };
    let header = |name: &str| -> Result<&str> {
        request
            .headers()
            .get(name)
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| invalid("screenshot export headers are incomplete"))
    };

    let action = ExportAction::parse(header(ACTION_HEADER)?)
        .ok_or_else(|| invalid("screenshot export action is invalid"))?;
    let width = parse_number::<u32>(header(WIDTH_HEADER)?)?;
    let height = parse_number::<u32>(header(HEIGHT_HEADER)?)?;
    let offset_x = parse_number::<i32>(header(OFFSET_X_HEADER)?)?;
    let offset_y = parse_number::<i32>(header(OFFSET_Y_HEADER)?)?;
    validate_pixels(width, height, body.len())?;

    Ok(ExportRequest {
        label: header(LABEL_HEADER)?.to_owned(),
        action,
        width,
        height,
        offset_x,
        offset_y,
        rgba: body.clone(),
    })
}

pub async fn export(app: &AppHandle, request: ExportRequest) -> Result<ExportResult> {
    let state = app.state::<ScreenshotState>();
    let source = state
        .image(&request.label)
        .ok_or_else(|| invalid("screenshot window is closed"))?;
    let ExportRequest {
        label,
        action,
        width,
        height,
        offset_x,
        offset_y,
        rgba,
    } = request;

    match action {
        ExportAction::Copy => {
            let png = run_blocking(move || encode_png(width, height, &rgba)).await?;
            let history_recorded = copy_png(app, png, width, height).await?;

            Ok(ExportResult {
                history_recorded,
                ..ExportResult::default()
            })
        }
        ExportAction::Save => {
            let png = run_blocking(move || encode_png(width, height, &rgba)).await?;
            let saved_path = save_png(app, &label, png, source.captured_at).await?;

            Ok(ExportResult {
                saved_path,
                ..ExportResult::default()
            })
        }
        ExportAction::Pin => {
            pin::open(
                app,
                CapturedImage {
                    width,
                    height,
                    scale_factor: source.scale_factor,
                    origin_x: source.origin_x.saturating_add(offset_x),
                    origin_y: source.origin_y.saturating_add(offset_y),
                    rgba,
                    captured_at: source.captured_at,
                },
            )?;

            Ok(ExportResult::default())
        }
        ExportAction::PrepareDrag => {
            let dir = app
                .path()
                .app_cache_dir()
                .map_err(|err| anyhow::anyhow!(err))?
                .join(DRAG_DIR)
                .join(&label);
            let path = dir.join(file_name(source.captured_at));
            let file = run_blocking(move || {
                let preview_png = drag_preview(width, height, &rgba);
                let png = encode_png(width, height, &rgba)?;
                std::fs::create_dir_all(&dir).map_err(io_error)?;
                std::fs::write(&path, png).map_err(io_error)?;

                Ok(DragFile { path, preview_png })
            })
            .await?;
            state.set_drag_file(&label, file);

            Ok(ExportResult::default())
        }
        ExportAction::Ocr => {
            let text = run_blocking(move || {
                let text = super::ocr::recognize(width, height, &rgba)?;
                if !text.is_empty() {
                    let context = ClipboardContext::new().map_err(clipboard_error)?;
                    context.set_text(text.clone()).map_err(clipboard_error)?;
                }

                Ok(text)
            })
            .await?;

            Ok(ExportResult {
                text: Some(text),
                ..ExportResult::default()
            })
        }
    }
}

/// Starts a native drag with the file prepared by the last `PrepareDrag` export.
pub async fn start_drag(app: &AppHandle, label: &str) -> Result<()> {
    let file = app
        .state::<ScreenshotState>()
        .drag_file(label)
        .ok_or_else(|| invalid("drag image is not ready"))?;
    let window = app
        .get_webview_window(label)
        .ok_or_else(|| invalid("screenshot window is closed"))?;

    #[cfg(target_os = "macos")]
    {
        app.run_on_main_thread(move || {
            if let Err(err) =
                crate::drag_out::start_drag_files(&window, vec![file.path], file.preview_png)
            {
                log::error!("start screenshot drag (macos) failed: {err}");
            }
        })
        .map_err(|err| anyhow::anyhow!(err))?;
    }

    #[cfg(target_os = "windows")]
    {
        let (tx, rx) = std::sync::mpsc::channel();
        app.run_on_main_thread(move || {
            // The editor window is a fresh, borderless WebView2 window each
            // capture; skip the ghost-preview wrap (see start_drag_files_with_ghost).
            let _ = tx.send(crate::drag_out::start_drag_files_with_ghost(
                &window,
                vec![file.path],
                file.preview_png,
                false,
            ));
        })
        .map_err(|err| anyhow::anyhow!(err))?;

        rx.recv()
            .map_err(|err| anyhow::anyhow!("drag result channel closed: {err}"))??;
    }

    Ok(())
}

pub async fn copy_window_image(app: &AppHandle, label: &str) -> Result<()> {
    let image = window_image(app, label)?;
    let encode_image = Arc::clone(&image);
    let png = run_blocking(move || {
        encode_png(encode_image.width, encode_image.height, &encode_image.rgba)
    })
    .await?;

    copy_png(app, png, image.width, image.height).await?;
    Ok(())
}

pub async fn save_window_image(app: &AppHandle, label: &str) -> Result<()> {
    let image = window_image(app, label)?;
    let encode_image = Arc::clone(&image);
    let png = run_blocking(move || {
        encode_png(encode_image.width, encode_image.height, &encode_image.rgba)
    })
    .await?;

    save_png(app, label, png, image.captured_at).await?;
    Ok(())
}

pub async fn copy_color(color: &str) -> Result<()> {
    let color = normalize_hex_color(color).ok_or_else(|| invalid("color is invalid"))?;

    run_blocking(move || {
        let context = ClipboardContext::new().map_err(clipboard_error)?;
        context.set_text(color).map_err(clipboard_error)
    })
    .await
}

/// Reads the clipboard image as PNG for pasting into the editor. Returns an
/// empty buffer when the clipboard holds no image.
pub async fn read_clipboard_png() -> Result<Vec<u8>> {
    run_blocking(|| {
        let context = ClipboardContext::new().map_err(clipboard_error)?;
        let Ok(image) = context.get_image() else {
            return Ok(Vec::new());
        };
        let png = image.to_png().map_err(clipboard_error)?;

        Ok(png.get_bytes().to_vec())
    })
    .await
}

/// Encodes RGBA pixels with the same encoder `clipboard-rs` uses, so a copied
/// image keeps the same bytes and content hash when the watcher reads it back.
pub fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>> {
    let image = ImageBuffer::<Rgba<u8>, &[u8]>::from_raw(width, height, rgba)
        .ok_or_else(|| invalid("image size does not match its pixels"))?;
    let mut bytes = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
        .map_err(|err| anyhow::anyhow!("encode screenshot png: {err}"))?;

    Ok(bytes)
}

/// Writes a PNG to the clipboard and records it in history when capture allows it.
/// Returns whether a history item was written.
async fn copy_png(app: &AppHandle, png: Vec<u8>, width: u32, height: u32) -> Result<bool> {
    let settings = app.state::<SettingsStore>().snapshot();
    let store = app.state::<ImageStore>().inner().clone();
    let guard = app.state::<Arc<WritebackGuard>>().inner().clone();
    let listening = !app
        .try_state::<WatcherPause>()
        .is_some_and(|pause| pause.is_paused());
    let payload = ClipboardPayload::Image(ImagePayload {
        bytes: png,
        width,
        height,
    });

    let item = if listening {
        build_item_with_settings(
            &store,
            &payload,
            &settings.clipboard.capture,
            &settings.clipboard.sensitive,
            false,
        )?
    } else {
        None
    };

    let Some(item) = item else {
        let ClipboardPayload::Image(image) = payload else {
            return Ok(false);
        };
        run_blocking(move || write_png(&image.bytes)).await?;
        maybe_play_copy(app);

        return Ok(false);
    };

    let pool = app.state::<DatabaseState>().pool().await;
    persist_and_notify(app, &pool, &item, None).await?;
    if let Some(runtime) = app.try_state::<crate::sync::SyncRuntime>() {
        runtime.publish_local(&payload);
    }
    run_blocking(move || write_to_clipboard(&store, guard.as_ref(), &item, false)).await?;
    maybe_play_copy(app);

    Ok(true)
}

async fn save_png(
    app: &AppHandle,
    parent_label: &str,
    png: Vec<u8>,
    captured_at: DateTime<Local>,
) -> Result<Option<String>> {
    let lang = crate::i18n::current_language(app);
    let mut dialog = app
        .dialog()
        .file()
        .add_filter("PNG", &["png"])
        .set_can_create_directories(true)
        .set_file_name(file_name(captured_at))
        .set_title(screenshot_menu::label(lang, Key::SaveDialogTitle));

    if let Ok(directory) = app
        .path()
        .picture_dir()
        .or_else(|_| app.path().download_dir())
    {
        dialog = dialog.set_directory(directory);
    }
    if let Some(window) = app.get_webview_window(parent_label) {
        dialog = dialog.set_parent(&window);
    }

    let Some(target) = dialog.blocking_save_file() else {
        return Ok(None);
    };
    let target = target
        .into_path()
        .map_err(|err| AppError::Clipboard(format!("save path is invalid: {err}")))?;
    let write_target: PathBuf = target.clone();
    run_blocking(move || std::fs::write(&write_target, png).map_err(io_error)).await?;

    Ok(Some(target.to_string_lossy().into_owned()))
}

fn window_image(app: &AppHandle, label: &str) -> Result<Arc<CapturedImage>> {
    app.state::<ScreenshotState>()
        .image(label)
        .ok_or_else(|| invalid("screenshot window is closed"))
}

fn write_png(png: &[u8]) -> Result<()> {
    let context = ClipboardContext::new().map_err(clipboard_error)?;
    let image = RustImageData::from_bytes(png).map_err(clipboard_error)?;

    context.set_image(image).map_err(clipboard_error)
}

fn drag_preview(width: u32, height: u32, rgba: &[u8]) -> Option<Vec<u8>> {
    let view = ImageBuffer::<Rgba<u8>, &[u8]>::from_raw(width, height, rgba)?;
    let ratio = (f64::from(DRAG_PREVIEW_MAX) / f64::from(width.max(height))).min(1.0);
    let preview_width = ((f64::from(width) * ratio).round() as u32).max(1);
    let preview_height = ((f64::from(height) * ratio).round() as u32).max(1);
    let preview = image::imageops::thumbnail(&view, preview_width, preview_height);

    encode_png(preview_width, preview_height, preview.as_raw()).ok()
}

fn file_name(captured_at: DateTime<Local>) -> String {
    format!(
        "UltraClipboard-screenshot-{}.png",
        captured_at.format("%Y%m%d-%H%M%S")
    )
}

fn validate_pixels(width: u32, height: u32, byte_len: usize) -> Result<()> {
    if width == 0 || height == 0 || width > MAX_EXPORT_SIDE || height > MAX_EXPORT_SIDE {
        return Err(invalid("screenshot export size is invalid"));
    }

    let expected = (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(4));
    if expected != Some(byte_len) {
        return Err(invalid("screenshot export pixels do not match its size"));
    }

    Ok(())
}

fn parse_number<T: std::str::FromStr>(value: &str) -> Result<T> {
    value
        .parse()
        .map_err(|_| invalid("screenshot export headers are invalid"))
}

fn normalize_hex_color(value: &str) -> Option<String> {
    let hex = value.strip_prefix('#')?;
    if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }

    Some(format!("#{}", hex.to_ascii_uppercase()))
}

async fn run_blocking<T: Send + 'static>(
    task: impl FnOnce() -> Result<T> + Send + 'static,
) -> Result<T> {
    tauri::async_runtime::spawn_blocking(task)
        .await
        .map_err(|err| anyhow::anyhow!("screenshot task join failed: {err}"))?
}

fn invalid(message: &str) -> AppError {
    AppError::Other(anyhow::anyhow!(message.to_owned()))
}

fn io_error(err: std::io::Error) -> AppError {
    AppError::Other(anyhow::anyhow!(err))
}

fn clipboard_error<E: std::fmt::Display>(err: E) -> AppError {
    AppError::Clipboard(err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gradient(width: u32, height: u32) -> Vec<u8> {
        (0..height)
            .flat_map(|y| (0..width).flat_map(move |x| [x as u8, y as u8, (x ^ y) as u8, 255]))
            .collect()
    }

    #[test]
    fn png_round_trips_exact_pixels() {
        let rgba = gradient(37, 23);
        let png = encode_png(37, 23, &rgba).unwrap();
        let decoded = image::load_from_memory(&png).unwrap().to_rgba8();

        assert_eq!(decoded.dimensions(), (37, 23));
        assert_eq!(decoded.into_raw(), rgba);
    }

    #[test]
    fn png_bytes_match_clipboard_rs_reencoding() {
        let rgba = gradient(64, 48);
        let png = encode_png(64, 48, &rgba).unwrap();
        let reencoded = RustImageData::from_bytes(&png).unwrap().to_png().unwrap();

        assert_eq!(reencoded.get_bytes(), png.as_slice());
    }

    #[test]
    fn pixel_validation_rejects_mismatched_buffers() {
        assert!(validate_pixels(2, 2, 16).is_ok());
        assert!(validate_pixels(2, 2, 15).is_err());
        assert!(validate_pixels(0, 2, 0).is_err());
        assert!(validate_pixels(MAX_EXPORT_SIDE + 1, 1, 0).is_err());
    }

    #[test]
    fn export_actions_parse_frontend_literals() {
        assert_eq!(ExportAction::parse("copy"), Some(ExportAction::Copy));
        assert_eq!(
            ExportAction::parse("prepareDrag"),
            Some(ExportAction::PrepareDrag)
        );
        assert_eq!(ExportAction::parse("ocr"), Some(ExportAction::Ocr));
        assert_eq!(ExportAction::parse("drag"), None);
    }

    #[test]
    fn hex_colors_are_normalized_and_validated() {
        assert_eq!(normalize_hex_color("#191a1d").as_deref(), Some("#191A1D"));
        assert!(normalize_hex_color("191A1D").is_none());
        assert!(normalize_hex_color("#19GA1D").is_none());
        assert!(normalize_hex_color("#191A1D00").is_none());
    }

    #[test]
    fn drag_preview_fits_inside_limit() {
        let rgba = gradient(1000, 500);
        let preview = drag_preview(1000, 500, &rgba).unwrap();
        let decoded = image::load_from_memory(&preview).unwrap();

        assert_eq!((decoded.width(), decoded.height()), (256, 128));
    }
}
