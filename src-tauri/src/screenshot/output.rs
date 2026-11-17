//! Output actions for edited screenshots. The editor renders the final pixels and
//! sends raw RGBA; Rust encodes once and routes the result to clipboard, file,
//! pin, or drag-out.

use std::io::{Cursor, Write};
use std::path::PathBuf;
use std::sync::Arc;

use chrono::{DateTime, Local};
use clipboard_rs::common::RustImage;
use clipboard_rs::{Clipboard, ClipboardContext, RustImageData};
use image::{ImageBuffer, ImageFormat, Rgba};
#[cfg(target_os = "macos")]
use objc2_app_kit::NSEvent;
use serde::Serialize;
use tauri::http::HeaderMap;
use tauri::ipc::{InvokeBody, Request};
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;
#[cfg(target_os = "windows")]
use winapi::um::winuser::{GetAsyncKeyState, VK_LBUTTON};

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
const REQUEST_ID_HEADER: &str = "x-screenshot-request-id";
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
    SaveAs,
    Pin,
    PrepareDrag,
    /// Recognizes text in the pixels and copies it.
    Ocr,
    /// Returns located text without changing the clipboard.
    SelectText,
    /// Recognizes text and returns translated layout blocks.
    Translate,
}

impl ExportAction {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "copy" => Some(Self::Copy),
            "save" => Some(Self::Save),
            "saveAs" => Some(Self::SaveAs),
            "pin" => Some(Self::Pin),
            "prepareDrag" => Some(Self::PrepareDrag),
            "ocr" => Some(Self::Ocr),
            "selectText" => Some(Self::SelectText),
            "translate" => Some(Self::Translate),
            _ => None,
        }
    }
}

pub struct ExportRequest {
    pub label: String,
    pub action: ExportAction,
    pub width: u32,
    pub height: u32,
    pub request_id: Option<u64>,
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
    /// Recognized text; only the Ocr action copies it to the clipboard.
    pub text: Option<String>,
    pub text_regions: Option<Vec<super::ocr::TextRegion>>,
    pub translation_blocks: Option<Vec<super::translation::TranslationBlock>>,
    /// The translation request was cancelled; `translation_blocks` is absent.
    pub translation_cancelled: bool,
}

pub fn parse_request(request: &Request<'_>) -> Result<ExportRequest> {
    let InvokeBody::Raw(body) = request.body() else {
        return Err(invalid("screenshot export must send raw pixels"));
    };

    parse_parts(request.headers(), body)
}

fn parse_parts(headers: &HeaderMap, body: &[u8]) -> Result<ExportRequest> {
    let header = |name: &str| -> Result<&str> {
        headers
            .get(name)
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| invalid("screenshot export headers are incomplete"))
    };

    let action = ExportAction::parse(header(ACTION_HEADER)?)
        .ok_or_else(|| invalid("screenshot export action is invalid"))?;
    let width = parse_number::<u32>(header(WIDTH_HEADER)?)?;
    let height = parse_number::<u32>(header(HEIGHT_HEADER)?)?;
    let request_id = if action == ExportAction::Translate {
        let id = parse_number::<u64>(header(REQUEST_ID_HEADER)?)?;
        if id == 0 || id > super::translation::MAX_REQUEST_ID {
            return Err(invalid("screenshot translation request ID is invalid"));
        }
        Some(id)
    } else {
        None
    };
    let offset_x = parse_number::<i32>(header(OFFSET_X_HEADER)?)?;
    let offset_y = parse_number::<i32>(header(OFFSET_Y_HEADER)?)?;
    validate_pixels(width, height, body.len())?;

    Ok(ExportRequest {
        label: header(LABEL_HEADER)?.to_owned(),
        action,
        width,
        height,
        request_id,
        offset_x,
        offset_y,
        rgba: body.to_vec(),
    })
}

pub async fn export(app: &AppHandle, request: ExportRequest) -> Result<ExportResult> {
    let state = app.state::<ScreenshotState>();
    if request.action == ExportAction::Translate {
        return export_translation(app, request).await;
    }
    let Some(source) = state.image(&request.label) else {
        return Err(invalid("screenshot window is closed"));
    };
    let ExportRequest {
        label,
        action,
        width,
        height,
        request_id: _,
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
        ExportAction::Save | ExportAction::SaveAs => {
            let png = run_blocking(move || encode_png(width, height, &rgba)).await?;
            let saved_path = if action == ExportAction::Save {
                Some(
                    auto_save_png(app, png, source.captured_at, source.source_title.clone())
                        .await?,
                )
            } else {
                save_png(
                    app,
                    &label,
                    png,
                    source.captured_at,
                    source.source_title.as_deref(),
                )
                .await?
            };

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
                    source_title: source.source_title.clone(),
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
            let path = dir.join(file_name(
                source.captured_at,
                source.source_title.as_deref(),
            ));
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
            let text = run_blocking(move || super::ocr::recognize(width, height, &rgba)).await?;
            if !text.is_empty() {
                copy_text(&text).await?;
            }

            Ok(ExportResult {
                text: Some(text),
                ..ExportResult::default()
            })
        }
        ExportAction::SelectText => {
            let recognition =
                run_blocking(move || super::ocr::recognize_regions(width, height, &rgba)).await?;
            Ok(ExportResult {
                text: Some(recognition.text),
                text_regions: Some(recognition.regions),
                ..ExportResult::default()
            })
        }
        ExportAction::Translate => Err(invalid("screenshot translation is handled separately")),
    }
}

/// Hands the pixels to the translation flow, which owns registration, cancellation
/// and cleanup. The window check runs inside the flow so a window closed mid-request
/// reads as cancelled, not as an error.
async fn export_translation(app: &AppHandle, request: ExportRequest) -> Result<ExportResult> {
    let id = request
        .request_id
        .ok_or_else(|| invalid("screenshot translation request ID is missing"))?;
    let ExportRequest {
        label,
        width,
        height,
        rgba,
        ..
    } = request;
    let flights = app.state::<super::translation::TranslationFlights>();
    let handle = app.clone();
    let window_label = label.clone();
    let outcome = flights
        .run(&label, id, move |cancelled| async move {
            if handle
                .state::<ScreenshotState>()
                .image(&window_label)
                .is_none()
            {
                return Err(invalid("screenshot window is closed"));
            }
            super::translation::recognize_and_translate(cancelled, width, height, rgba).await
        })
        .await?;

    Ok(match outcome {
        super::translation::TranslationOutcome::Blocks(blocks) => ExportResult {
            translation_blocks: Some(blocks),
            ..ExportResult::default()
        },
        super::translation::TranslationOutcome::Cancelled => ExportResult {
            translation_cancelled: true,
            ..ExportResult::default()
        },
    })
}

pub async fn copy_text(text: &str) -> Result<()> {
    if text.trim().is_empty() {
        return Err(invalid("screenshot text is empty"));
    }
    if text.len() > 1024 * 1024 {
        return Err(invalid("screenshot text is too large"));
    }
    let text = text.to_owned();
    run_blocking(move || {
        let context = ClipboardContext::new().map_err(clipboard_error)?;
        context.set_text(text).map_err(clipboard_error)
    })
    .await
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
            if NSEvent::pressedMouseButtons() & 1 == 0 {
                log::debug!("screenshot drag skipped after pointer release");
                return;
            }
            if let Err(err) = crate::drag_out::start_drag_files_with_ghost(
                &window,
                vec![file.path],
                file.preview_png,
                false,
            ) {
                log::error!("start screenshot drag (macos) failed: {err}");
            }
        })
        .map_err(|err| anyhow::anyhow!(err))?;
    }

    #[cfg(target_os = "windows")]
    {
        let (tx, rx) = std::sync::mpsc::channel();
        let main_app = app.clone();
        app.run_on_main_thread(move || {
            // SAFETY: GetAsyncKeyState reads current input state without owning a handle.
            if unsafe { GetAsyncKeyState(VK_LBUTTON) as u16 } & 0x8000 == 0 {
                log::debug!("screenshot drag skipped after pointer release");
                let _ = tx.send(Ok(()));
                return;
            }
            let restore_nav = NavigationResume::suspend(&main_app);
            // The editor window is a fresh, borderless WebView2 window each
            // capture; skip the ghost-preview wrap (see start_drag_files_with_ghost).
            let result = crate::drag_out::start_drag_files_with_ghost(
                &window,
                vec![file.path],
                file.preview_png,
                false,
            );
            drop(restore_nav);
            let _ = tx.send(result);
        })
        .map_err(|err| anyhow::anyhow!(err))?;

        rx.recv()
            .map_err(|err| anyhow::anyhow!("drag result channel closed: {err}"))??;
    }

    Ok(())
}

#[cfg(target_os = "windows")]
struct NavigationResume<'a> {
    app: &'a AppHandle,
    was_enabled: bool,
}

#[cfg(target_os = "windows")]
impl<'a> NavigationResume<'a> {
    fn suspend(app: &'a AppHandle) -> Self {
        let was_enabled = crate::keyboard::navigation_keys_enabled();
        if was_enabled {
            crate::keyboard::disable_navigation_keys();
        }
        Self { app, was_enabled }
    }
}

#[cfg(target_os = "windows")]
impl Drop for NavigationResume<'_> {
    fn drop(&mut self) {
        if !self.was_enabled {
            return;
        }
        let clipboard_visible = self
            .app
            .get_webview_window(crate::window::CLIPBOARD_WINDOW_LABEL)
            .and_then(|window| window.is_visible().ok())
            .unwrap_or(false);
        if clipboard_visible {
            crate::keyboard::enable_navigation_keys(self.app);
        }
    }
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

    save_png(
        app,
        label,
        png,
        image.captured_at,
        image.source_title.as_deref(),
    )
    .await?;
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
    let _resource_access = app
        .state::<crate::storage::StorageAccess>()
        .read_owned()
        .await;
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
    source_title: Option<&str>,
) -> Result<Option<String>> {
    let lang = crate::i18n::current_language(app);
    let mut dialog = app
        .dialog()
        .file()
        .add_filter("PNG", &["png"])
        .set_can_create_directories(true)
        .set_file_name(file_name(captured_at, source_title))
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

async fn auto_save_png(
    app: &AppHandle,
    png: Vec<u8>,
    captured_at: DateTime<Local>,
    source_title: Option<String>,
) -> Result<String> {
    let directory = app
        .path()
        .picture_dir()
        .or_else(|_| app.path().download_dir())
        .map_err(|err| AppError::Other(anyhow::anyhow!(err)))?;
    let name = file_name(captured_at, source_title.as_deref());
    let path = run_blocking(move || write_unique_png(&directory, &name, &png)).await?;
    Ok(path.to_string_lossy().into_owned())
}

fn write_unique_png(directory: &std::path::Path, name: &str, png: &[u8]) -> Result<PathBuf> {
    std::fs::create_dir_all(directory).map_err(io_error)?;
    let stem = name.strip_suffix(".png").unwrap_or(name);
    for index in 0..10_000 {
        let name = if index == 0 {
            name.to_owned()
        } else {
            format!("{stem}-{}.png", index + 1)
        };
        let path = directory.join(name);
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path);
        let mut file = match file {
            Ok(file) => file,
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(io_error(err)),
        };
        if let Err(err) = file.write_all(png) {
            drop(file);
            if let Err(cleanup_err) = std::fs::remove_file(&path) {
                log::warn!("remove incomplete screenshot file failed: {cleanup_err}");
            }
            return Err(io_error(err));
        }
        return Ok(path);
    }
    Err(invalid("too many screenshots with the same name"))
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

fn file_name(captured_at: DateTime<Local>, source_title: Option<&str>) -> String {
    let source = source_title
        .map(sanitize_file_stem)
        .filter(|source| !source.is_empty())
        .unwrap_or_else(|| "Screen".to_owned());
    format!("{source}-{}.png", captured_at.format("%Y%m%d-%H%M%S"))
}

fn sanitize_file_stem(title: &str) -> String {
    let mut stem = String::new();
    let mut separator = false;
    for character in title.chars() {
        if stem.chars().count() >= 72 {
            break;
        }
        if character.is_alphanumeric() {
            stem.push(character);
            separator = false;
        } else if !separator && !stem.is_empty() {
            stem.push('-');
            separator = true;
        }
    }
    stem.trim_end_matches('-').to_owned()
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
        assert_eq!(ExportAction::parse("save"), Some(ExportAction::Save));
        assert_eq!(ExportAction::parse("saveAs"), Some(ExportAction::SaveAs));
        assert_eq!(
            ExportAction::parse("prepareDrag"),
            Some(ExportAction::PrepareDrag)
        );
        assert_eq!(ExportAction::parse("ocr"), Some(ExportAction::Ocr));
        assert_eq!(
            ExportAction::parse("selectText"),
            Some(ExportAction::SelectText)
        );
        assert_eq!(
            ExportAction::parse("translate"),
            Some(ExportAction::Translate)
        );
        assert_eq!(ExportAction::parse("drag"), None);
    }

    fn export_headers(action: &str, width: &str, height: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for (name, value) in [
            (LABEL_HEADER, "screenshot-editor-1"),
            (ACTION_HEADER, action),
            (WIDTH_HEADER, width),
            (HEIGHT_HEADER, height),
            (OFFSET_X_HEADER, "3"),
            (OFFSET_Y_HEADER, "-4"),
        ] {
            headers.insert(name, value.parse().unwrap());
        }

        headers
    }

    fn parse_error(headers: &HeaderMap, body: &[u8]) -> String {
        parse_parts(headers, body).err().unwrap().to_string()
    }

    #[test]
    fn export_request_parses_valid_headers() {
        let headers = export_headers("saveAs", "2", "1");
        let request = parse_parts(&headers, &[0; 8]).unwrap();

        assert_eq!(request.label, "screenshot-editor-1");
        assert_eq!(request.action, ExportAction::SaveAs);
        assert_eq!((request.width, request.height), (2, 1));
        assert_eq!((request.offset_x, request.offset_y), (3, -4));
        assert_eq!(request.request_id, None);
        assert_eq!(request.rgba.len(), 8);
    }

    #[test]
    fn export_request_rejects_missing_header() {
        let mut headers = export_headers("copy", "2", "1");
        headers.remove(LABEL_HEADER);
        assert!(parse_error(&headers, &[0; 8]).contains("headers are incomplete"));

        let mut headers = export_headers("copy", "2", "1");
        headers.remove(OFFSET_X_HEADER);
        assert!(parse_error(&headers, &[0; 8]).contains("headers are incomplete"));
    }

    #[test]
    fn export_request_rejects_bad_action_and_numbers() {
        let headers = export_headers("drag", "2", "1");
        assert!(parse_error(&headers, &[0; 8]).contains("action is invalid"));

        let headers = export_headers("copy", "wide", "1");
        assert!(parse_error(&headers, &[0; 8]).contains("headers are invalid"));
    }

    #[test]
    fn export_request_rejects_pixel_length_mismatch() {
        let headers = export_headers("copy", "2", "1");

        assert!(parse_error(&headers, &[0; 7]).contains("pixels do not match"));
    }

    #[test]
    fn translate_request_needs_a_bounded_request_id() {
        let mut headers = export_headers("translate", "2", "1");
        assert!(parse_error(&headers, &[0; 8]).contains("headers are incomplete"));

        for id in [
            "0",
            &(super::super::translation::MAX_REQUEST_ID + 1).to_string(),
        ] {
            headers.insert(REQUEST_ID_HEADER, id.parse().unwrap());
            assert!(parse_error(&headers, &[0; 8]).contains("request ID is invalid"));
        }

        headers.insert(REQUEST_ID_HEADER, "7".parse().unwrap());
        assert_eq!(parse_parts(&headers, &[0; 8]).unwrap().request_id, Some(7));
    }

    #[test]
    fn export_header_names_match_the_frontend_definition() {
        let frontend = include_str!("../../../src/commands/index.ts");

        for name in [
            LABEL_HEADER,
            ACTION_HEADER,
            WIDTH_HEADER,
            HEIGHT_HEADER,
            REQUEST_ID_HEADER,
            OFFSET_X_HEADER,
            OFFSET_Y_HEADER,
        ] {
            assert!(
                frontend.contains(&format!("\"{name}\"")),
                "frontend does not send {name}"
            );
        }
    }

    #[test]
    fn screenshot_name_uses_work_window_and_never_overwrites_same_second() {
        let captured_at = Local::now();
        let title = "Quarterly plan: Q4 / review?";
        let name = file_name(captured_at, Some(title));
        assert!(name.starts_with("Quarterly-plan-Q4-review-"));
        assert!(!name
            .chars()
            .any(|character| ['/', '?', '\\', ':'].contains(&character)));

        let directory = tempfile::tempdir().unwrap();
        let first = write_unique_png(directory.path(), &name, b"first").unwrap();
        let second = write_unique_png(directory.path(), &name, b"second").unwrap();
        assert_ne!(first, second);
        assert_eq!(std::fs::read(first).unwrap(), b"first");
        assert_eq!(std::fs::read(second).unwrap(), b"second");
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
