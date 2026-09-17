//! Screenshot commands. They validate window labels and delegate to `crate::screenshot`.

use tauri::ipc::{Request, Response};
use tauri::AppHandle;

use crate::core::{AppError, Result};
use crate::screenshot::{
    self, CaptureMode, ExportResult, ImageInfo, OverlayState, PixelRect,
    EDITOR_WINDOW_LABEL_PREFIX, OVERLAY_WINDOW_LABEL_PREFIX, PIN_WINDOW_LABEL_PREFIX,
};

#[tauri::command]
pub async fn start_screenshot_capture(app: AppHandle, mode: CaptureMode) -> Result<()> {
    screenshot::start_capture(&app, mode);

    Ok(())
}

#[tauri::command]
pub async fn get_screenshot_overlay_state(
    app: AppHandle,
    label: String,
) -> Result<Option<OverlayState>> {
    require_prefix(&label, &[OVERLAY_WINDOW_LABEL_PREFIX])?;

    Ok(screenshot::overlay_state(&app, &label))
}

#[tauri::command]
pub async fn get_screenshot_overlay_frame(
    app: AppHandle,
    label: String,
    session_id: u64,
) -> Result<Response> {
    require_prefix(&label, &[OVERLAY_WINDOW_LABEL_PREFIX])?;

    Ok(Response::new(screenshot::overlay_frame(
        &app, &label, session_id,
    )?))
}

#[tauri::command]
pub async fn notify_screenshot_overlay_ready(
    app: AppHandle,
    label: String,
    session_id: u64,
) -> Result<()> {
    require_prefix(&label, &[OVERLAY_WINDOW_LABEL_PREFIX])?;

    screenshot::reveal_overlay(&app, &label, session_id)
}

#[tauri::command]
pub async fn commit_screenshot_selection(
    app: AppHandle,
    label: String,
    session_id: u64,
    rect: PixelRect,
) -> Result<()> {
    require_prefix(&label, &[OVERLAY_WINDOW_LABEL_PREFIX])?;

    screenshot::commit_selection(&app, &label, session_id, rect)
}

#[tauri::command]
pub async fn cancel_screenshot_capture(app: AppHandle, session_id: Option<u64>) -> Result<()> {
    screenshot::cancel_capture(&app, session_id);

    Ok(())
}

#[tauri::command]
pub async fn get_screenshot_image_info(app: AppHandle, label: String) -> Result<ImageInfo> {
    require_prefix(
        &label,
        &[EDITOR_WINDOW_LABEL_PREFIX, PIN_WINDOW_LABEL_PREFIX],
    )?;

    screenshot::image_info(&app, &label)
}

#[tauri::command]
pub async fn get_screenshot_image(app: AppHandle, label: String) -> Result<Response> {
    require_prefix(
        &label,
        &[EDITOR_WINDOW_LABEL_PREFIX, PIN_WINDOW_LABEL_PREFIX],
    )?;

    Ok(Response::new(screenshot::image_pixels(&app, &label)?))
}

#[tauri::command]
pub async fn notify_screenshot_window_ready(app: AppHandle, label: String) -> Result<()> {
    require_prefix(
        &label,
        &[EDITOR_WINDOW_LABEL_PREFIX, PIN_WINDOW_LABEL_PREFIX],
    )?;

    screenshot::reveal_window(&app, &label)
}

#[tauri::command]
pub async fn close_screenshot_window(app: AppHandle, label: String) -> Result<()> {
    require_prefix(
        &label,
        &[EDITOR_WINDOW_LABEL_PREFIX, PIN_WINDOW_LABEL_PREFIX],
    )?;

    screenshot::close_window(&app, &label);

    Ok(())
}

#[tauri::command]
pub async fn export_screenshot(app: AppHandle, request: Request<'_>) -> Result<ExportResult> {
    let export = screenshot::parse_export_request(&request)?;
    require_prefix(
        &export.label,
        &[EDITOR_WINDOW_LABEL_PREFIX, PIN_WINDOW_LABEL_PREFIX],
    )?;

    screenshot::export(&app, export).await
}

#[tauri::command]
pub async fn start_screenshot_drag(app: AppHandle, label: String) -> Result<()> {
    require_prefix(&label, &[EDITOR_WINDOW_LABEL_PREFIX])?;

    screenshot::start_drag(&app, &label).await
}

#[tauri::command]
pub async fn copy_screenshot_color(color: String) -> Result<()> {
    screenshot::copy_color(&color).await
}

#[tauri::command]
pub async fn read_screenshot_clipboard_image() -> Result<Response> {
    Ok(Response::new(screenshot::read_clipboard_image().await?))
}

#[tauri::command]
pub async fn set_screenshot_pin_scale(
    app: AppHandle,
    label: String,
    scale: f64,
    anchor_x: f64,
    anchor_y: f64,
) -> Result<()> {
    require_prefix(&label, &[PIN_WINDOW_LABEL_PREFIX])?;

    screenshot::set_pin_scale(&app, &label, scale, anchor_x, anchor_y)
}

#[tauri::command]
pub async fn show_screenshot_pin_menu(app: AppHandle, label: String) -> Result<()> {
    require_prefix(&label, &[PIN_WINDOW_LABEL_PREFIX])?;

    screenshot::show_pin_menu(&app, &label)
}

fn require_prefix(label: &str, prefixes: &[&str]) -> Result<()> {
    if prefixes.iter().any(|prefix| label.starts_with(prefix)) {
        return Ok(());
    }

    Err(AppError::Other(anyhow::anyhow!(
        "screenshot window label is invalid"
    )))
}
