//! Screenshot commands. Each parses its arguments and makes one call into
//! `crate::screenshot::api`, which owns label policy and main-thread ordering.

use tauri::ipc::{Request, Response};
use tauri::AppHandle;

use crate::core::Result;
use crate::screenshot::api::{any, editor_window, image_window, overlay, pin_window};
use crate::screenshot::{
    CaptureMode, ExportResult, ImageInfo, OverlayState, PixelRect, TranslationStatus,
};

#[tauri::command]
pub async fn start_screenshot_capture(app: AppHandle, mode: CaptureMode) -> Result<()> {
    any::start_capture(&app, mode);

    Ok(())
}

#[tauri::command]
pub async fn get_screenshot_overlay_state(
    app: AppHandle,
    label: String,
) -> Result<Option<OverlayState>> {
    overlay::state(&app, &label)
}

#[tauri::command]
pub async fn get_screenshot_overlay_frame(
    app: AppHandle,
    label: String,
    session_id: u64,
) -> Result<Response> {
    Ok(Response::new(
        overlay::frame(&app, &label, session_id).await?,
    ))
}

#[tauri::command]
pub async fn notify_screenshot_overlay_ready(
    app: AppHandle,
    label: String,
    session_id: u64,
) -> Result<()> {
    overlay::ready(&app, label, session_id).await
}

#[tauri::command]
pub async fn commit_screenshot_selection(
    app: AppHandle,
    label: String,
    session_id: u64,
    rect: PixelRect,
) -> Result<()> {
    overlay::commit_selection(&app, label, session_id, rect).await
}

#[tauri::command]
pub async fn cancel_screenshot_capture(app: AppHandle, session_id: Option<u64>) -> Result<()> {
    any::cancel_capture(&app, session_id).await
}

#[tauri::command]
pub async fn get_screenshot_image_info(app: AppHandle, label: String) -> Result<ImageInfo> {
    image_window::info(&app, &label)
}

#[tauri::command]
pub async fn get_screenshot_image(app: AppHandle, label: String) -> Result<Response> {
    Ok(Response::new(image_window::pixels(&app, &label)?))
}

#[tauri::command]
pub async fn notify_screenshot_window_ready(app: AppHandle, label: String) -> Result<()> {
    image_window::ready(&app, label).await
}

#[tauri::command]
pub async fn close_screenshot_window(app: AppHandle, label: String) -> Result<()> {
    image_window::close(&app, label).await
}

#[tauri::command]
pub async fn export_screenshot(app: AppHandle, request: Request<'_>) -> Result<ExportResult> {
    image_window::export(&app, &request).await
}

#[tauri::command]
pub async fn get_screenshot_translation_status() -> Result<TranslationStatus> {
    any::translation_status().await
}

#[tauri::command]
pub async fn set_screenshot_translation_key(key: String) -> Result<()> {
    any::set_translation_key(key).await
}

#[tauri::command]
pub async fn remove_screenshot_translation_key() -> Result<()> {
    any::remove_translation_key().await
}

#[tauri::command]
pub async fn cancel_screenshot_translation(
    app: AppHandle,
    label: String,
    request_id: u64,
) -> Result<()> {
    editor_window::cancel_translation(&app, &label, request_id)
}

#[tauri::command]
pub async fn start_screenshot_drag(app: AppHandle, label: String) -> Result<()> {
    editor_window::start_drag(&app, &label).await
}

#[tauri::command]
pub async fn copy_screenshot_color(color: String) -> Result<()> {
    any::copy_color(&color).await
}

#[tauri::command]
pub async fn copy_screenshot_text(app: AppHandle, label: String, text: String) -> Result<()> {
    editor_window::copy_text(&app, &label, &text).await
}

#[tauri::command]
pub async fn read_screenshot_clipboard_image() -> Result<Response> {
    Ok(Response::new(any::read_clipboard_image().await?))
}

#[tauri::command]
pub async fn set_screenshot_pin_scale(
    app: AppHandle,
    label: String,
    scale: f64,
    anchor_x: f64,
    anchor_y: f64,
) -> Result<()> {
    pin_window::set_scale(&app, label, scale, anchor_x, anchor_y).await
}

#[tauri::command]
pub async fn show_screenshot_pin_menu(app: AppHandle, label: String) -> Result<()> {
    pin_window::show_menu(&app, label).await
}
