//! Screenshot capture, frozen selection overlays, editor and pin windows, and
//! output actions. Rust owns capture, window geometry, clipboard, and files;
//! the frontend paints frames and edits pixels.

mod backend;
mod editor;
mod frame_transport;
mod geometry;
mod ocr;
mod output;
mod overlay;
mod pin;
mod state;
mod translation;

use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use chrono::Local;
use serde::{Deserialize, Serialize};
use tauri::ipc::Request;
use tauri::{AppHandle, Manager, Window};
use tokio::sync::oneshot;

use crate::core::{AppError, Result};

pub use editor::ImageInfo;
pub use geometry::PixelRect;
pub use output::ExportResult;
pub use overlay::OverlayState;
pub use translation::TranslationStatus;

use state::{CaptureSession, CapturedImage, MonitorFrame, MonitorGeometry, ScreenshotState};

pub const OVERLAY_WINDOW_LABEL_PREFIX: &str = "screenshot-overlay-";
pub const EDITOR_WINDOW_LABEL_PREFIX: &str = "screenshot-editor-";
pub const PIN_WINDOW_LABEL_PREFIX: &str = "screenshot-pin-";
/// Overlay window title; the macOS backend uses it to tell overlays apart from
/// this app's other windows when excluding them from a capture.
pub const OVERLAY_WINDOW_TITLE: &str = "Ultra Clipboard Capture";

/// Lets the tray menu finish closing before the screen is frozen.
const TRAY_CAPTURE_DELAY: Duration = Duration::from_millis(200);

/// Lets startup finish before the overlay WebViews are built in the background.
const OVERLAY_PREWARM_DELAY: Duration = Duration::from_secs(3);
const OVERLAY_PRESENT_TIMEOUT: Duration = Duration::from_secs(10);

/// Time to open a menu or hover state before a delayed capture freezes the screen.
const DELAYED_CAPTURE_DELAY: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CaptureMode {
    Area,
    Fullscreen,
    Window,
    /// Captures the last confirmed area again, or starts an area capture.
    Repeat,
    /// Starts an area capture after a short delay.
    Delayed,
}

pub fn init(app: &AppHandle) {
    app.manage(ScreenshotState::default());
    app.manage(translation::TranslationFlights::default());
    prewarm_overlays(app);
}

/// Builds one hidden overlay per monitor after startup, so the first capture
/// does not wait for WebViews to boot. Overlays are kept from then on.
fn prewarm_overlays(app: &AppHandle) {
    if !backend::is_supported() {
        return;
    }

    let app = app.clone();
    let spawned = thread::Builder::new()
        .name("screenshot-overlay-prewarm".to_owned())
        .spawn(move || {
            thread::sleep(OVERLAY_PREWARM_DELAY);

            let main_app = app.clone();
            if let Err(err) = app.run_on_main_thread(move || match backend::monitors(&main_app) {
                Ok(monitors) => overlay::prewarm(&main_app, monitors.len()),
                Err(err) => {
                    log::warn!("prewarm screenshot overlays could not read monitors: {err}")
                }
            }) {
                log::warn!("dispatch screenshot overlay prewarm failed: {err}");
            }
        });

    if let Err(err) = spawned {
        log::warn!("spawn screenshot overlay prewarm failed: {err}");
    }
}

pub fn is_supported() -> bool {
    backend::is_supported()
}

pub fn is_screenshot_window(label: &str) -> bool {
    [
        OVERLAY_WINDOW_LABEL_PREFIX,
        EDITOR_WINDOW_LABEL_PREFIX,
        PIN_WINDOW_LABEL_PREFIX,
    ]
    .iter()
    .any(|prefix| label.starts_with(prefix))
}

/// Handles OS close requests for screenshot windows. Returns `true` when the
/// default close must be prevented because this module owns the close flow.
pub fn intercept_close_request(window: &Window) -> bool {
    let app = window.app_handle();
    let label = window.label();

    if label.starts_with(EDITOR_WINDOW_LABEL_PREFIX) {
        editor::request_close(app, label);
        return true;
    }

    if label.starts_with(OVERLAY_WINDOW_LABEL_PREFIX) {
        let Some(session_id) = app
            .state::<ScreenshotState>()
            .with_session(|session| session.id)
        else {
            let main_app = app.clone();
            if let Err(err) = app.run_on_main_thread(move || overlay::hide_all(&main_app)) {
                log::error!("hide inactive screenshot overlay failed: {err}");
            }
            return true;
        };
        let main_app = app.clone();
        if let Err(err) =
            app.run_on_main_thread(move || cancel_capture(&main_app, Some(session_id)))
        {
            log::error!("dispatch screenshot overlay close failed: {err}");
        }
        return true;
    }

    if label.starts_with(PIN_WINDOW_LABEL_PREFIX) {
        let main_app = app.clone();
        let label = label.to_owned();
        if let Err(err) = app.run_on_main_thread(move || close_window(&main_app, &label)) {
            log::error!("dispatch screenshot pin close failed: {err}");
        }
        return true;
    }

    false
}

pub fn start_capture(app: &AppHandle, mode: CaptureMode) {
    dispatch_capture(app, mode, capture_delay(mode, None));
}

pub fn start_capture_from_tray(app: &AppHandle, mode: CaptureMode) {
    dispatch_capture(app, mode, capture_delay(mode, Some(TRAY_CAPTURE_DELAY)));
}

/// Runs screenshot window effects in the same order as capture and timeout callbacks.
pub async fn on_main_thread<T: Send + 'static>(
    app: &AppHandle,
    effect: impl FnOnce(AppHandle) -> T + Send + 'static,
) -> Result<T> {
    let (sender, receiver) = oneshot::channel();
    let main_app = app.clone();
    app.run_on_main_thread(move || {
        let _ = sender.send(effect(main_app));
    })
    .map_err(|err| AppError::Other(anyhow::anyhow!(err)))?;

    receiver
        .await
        .map_err(|err| AppError::Other(anyhow::anyhow!(err)))
}

fn dispatch_capture(app: &AppHandle, mode: CaptureMode, delay: Option<Duration>) {
    let capture_app = app.clone();
    if let Err(err) = app.run_on_main_thread(move || {
        begin_capture(&capture_app, mode, delay);
    }) {
        log::error!("dispatch screenshot capture to main thread failed: {err}");
    }
}

fn capture_delay(mode: CaptureMode, base: Option<Duration>) -> Option<Duration> {
    if mode == CaptureMode::Delayed {
        return Some(base.unwrap_or_default().max(DELAYED_CAPTURE_DELAY));
    }

    base
}

pub fn overlay_state(app: &AppHandle, label: &str) -> Option<OverlayState> {
    app.state::<ScreenshotState>()
        .with_session(|session| overlay::state_for(session, label))
        .flatten()
}

pub async fn overlay_frame(app: &AppHandle, label: &str, session_id: u64) -> Result<Vec<u8>> {
    let (rgba, width, height) = app
        .state::<ScreenshotState>()
        .with_session(|session| {
            if session.id != session_id {
                return None;
            }

            session.frame(label).and_then(|frame| {
                frame.rgba.clone().map(|rgba| {
                    (
                        rgba,
                        frame.monitor.bounds.width,
                        frame.monitor.bounds.height,
                    )
                })
            })
        })
        .flatten()
        .ok_or_else(|| AppError::Other(anyhow::anyhow!("capture session has ended")))?;

    let png = tauri::async_runtime::spawn_blocking(move || {
        frame_transport::encode_overlay_png(rgba.as_slice(), width, height)
    })
    .await
    .map_err(|err| AppError::Other(anyhow::anyhow!("overlay PNG task failed: {err}")))??;
    let current = app
        .state::<ScreenshotState>()
        .with_session(|session| session.id == session_id)
        .unwrap_or(false);
    if !current {
        return Err(AppError::Other(anyhow::anyhow!(
            "capture session has ended"
        )));
    }

    Ok(png)
}

pub fn reveal_overlay(app: &AppHandle, label: &str, session_id: u64) -> Result<()> {
    overlay::reveal(app, label, session_id)
}

/// Crops the frozen frame to the confirmed selection and opens the editor.
pub fn commit_selection(
    app: &AppHandle,
    label: &str,
    session_id: u64,
    selection: PixelRect,
) -> Result<()> {
    let state = app.state::<ScreenshotState>();
    if state.with_session(|session| session.id == session_id && !session.is_ready()) == Some(true) {
        return Err(AppError::Other(anyhow::anyhow!(
            "capture pixels have not arrived yet"
        )));
    }
    let Some(session) = state.take_session(Some(session_id)) else {
        return Ok(());
    };

    let result = (|| {
        let frame = session
            .frame(label)
            .ok_or_else(|| AppError::Other(anyhow::anyhow!("capture monitor is unavailable")))?;
        let frame_rgba = frame
            .rgba
            .as_ref()
            .ok_or_else(|| AppError::Other(anyhow::anyhow!("capture pixels are missing")))?;
        let bounds = frame.monitor.bounds;
        let selection = selection
            .intersect(&PixelRect::new(0, 0, bounds.width, bounds.height))
            .ok_or_else(|| AppError::Other(anyhow::anyhow!("selection is empty")))?;
        let rgba = geometry::crop_rgba(frame_rgba, bounds.width, bounds.height, selection)
            .ok_or_else(|| AppError::Other(anyhow::anyhow!("selection is outside the monitor")))?;

        log::info!(
            "screenshot selection committed: {}x{} at {},{}",
            selection.width,
            selection.height,
            selection.x,
            selection.y
        );
        state.set_last_area(PixelRect::new(
            bounds.x.saturating_add(selection.x),
            bounds.y.saturating_add(selection.y),
            selection.width,
            selection.height,
        ));

        editor::open(
            app,
            CapturedImage {
                width: selection.width,
                height: selection.height,
                scale_factor: frame.monitor.scale_factor,
                origin_x: bounds.x.saturating_add(selection.x),
                origin_y: bounds.y.saturating_add(selection.y),
                rgba,
                captured_at: session.captured_at,
                source_title: session.source_title.clone(),
            },
            frame.monitor,
            Some(session_id),
        )
    })();

    if result.is_err() {
        overlay::hide_all(app);
        state.finish_capture(session_id);
    }

    result
}

pub fn cancel_capture(app: &AppHandle, session_id: Option<u64>) {
    let state = app.state::<ScreenshotState>();
    cancel_active_session(
        &state,
        session_id,
        || overlay::hide_all(app),
        |previous| {
            backend::restore_foreground(previous);
        },
    );
}

fn cancel_active_session(
    state: &ScreenshotState,
    session_id: Option<u64>,
    hide: impl FnOnce(),
    restore_foreground: impl FnOnce(Option<isize>),
) -> bool {
    let Some(session) = state.take_session(session_id) else {
        return false;
    };
    hide();
    restore_foreground(session.previous_foreground);
    state.finish_capture(session.id);
    true
}

pub fn image_info(app: &AppHandle, label: &str) -> Result<ImageInfo> {
    app.state::<ScreenshotState>()
        .image(label)
        .map(|image| ImageInfo::from_image(&image))
        .ok_or_else(|| AppError::Other(anyhow::anyhow!("screenshot window is closed")))
}

pub fn image_pixels(app: &AppHandle, label: &str) -> Result<Vec<u8>> {
    app.state::<ScreenshotState>()
        .image(label)
        .map(|image| image.rgba.clone())
        .ok_or_else(|| AppError::Other(anyhow::anyhow!("screenshot window is closed")))
}

pub fn reveal_window(app: &AppHandle, label: &str) -> Result<()> {
    editor::reveal(app, label)
}

/// Releases a screenshot window's pixels and temporary drag file, then destroys it.
pub fn close_window(app: &AppHandle, label: &str) {
    app.state::<translation::TranslationFlights>()
        .cancel_label(label);
    let state = app.state::<ScreenshotState>();
    if let Some(drag_file) = state.remove_window(label) {
        if let Some(directory) = drag_file.path.parent() {
            if let Err(err) = std::fs::remove_dir_all(directory) {
                log::warn!("remove screenshot drag directory failed: {err}");
            }
        }
    }

    state.complete_editor_handoff(label, || {
        overlay::hide_all(app);
    });

    if let Some(window) = app.get_webview_window(label) {
        if let Err(err) = window.destroy() {
            log::error!("destroy screenshot window {label} failed: {err}");
        }
    }
}

pub fn parse_export_request(request: &Request<'_>) -> Result<output::ExportRequest> {
    output::parse_request(request)
}

pub async fn export(app: &AppHandle, request: output::ExportRequest) -> Result<ExportResult> {
    let started = Instant::now();
    let action = request.action;
    let (width, height) = (request.width, request.height);
    let result = output::export(app, request).await;

    log::info!(
        "screenshot export {action:?} {width}x{height} finished in {}ms",
        started.elapsed().as_millis()
    );

    result
}

pub async fn translation_status() -> Result<TranslationStatus> {
    translation::status().await
}

pub async fn set_translation_key(key: String) -> Result<()> {
    translation::set_key(key).await
}

pub async fn remove_translation_key() -> Result<()> {
    translation::remove_key().await
}

pub fn cancel_translation(app: &AppHandle, label: &str, request_id: u64) {
    app.state::<translation::TranslationFlights>()
        .cancel(label, request_id);
}

pub async fn start_drag(app: &AppHandle, label: &str) -> Result<()> {
    output::start_drag(app, label).await
}

pub async fn copy_color(color: &str) -> Result<()> {
    output::copy_color(color).await
}

pub async fn copy_text(app: &AppHandle, label: &str, text: &str) -> Result<()> {
    if app.state::<ScreenshotState>().image(label).is_none() {
        return Err(AppError::Other(anyhow::anyhow!(
            "screenshot window is closed"
        )));
    }

    output::copy_text(text).await
}

pub async fn read_clipboard_image() -> Result<Vec<u8>> {
    output::read_clipboard_png().await
}

pub fn set_pin_scale(
    app: &AppHandle,
    label: &str,
    scale: f64,
    anchor_x: f64,
    anchor_y: f64,
) -> Result<()> {
    pin::set_scale(app, label, scale, anchor_x, anchor_y)
}

pub fn show_pin_menu(app: &AppHandle, label: &str) -> Result<()> {
    pin::show_menu(app, label)
}

/// Handles native menu events owned by screenshot windows.
pub fn handle_menu_event(app: &AppHandle, id: &str) -> bool {
    pin::handle_menu_event(app, id)
}

fn begin_capture(app: &AppHandle, mode: CaptureMode, delay: Option<Duration>) {
    if !backend::is_supported() {
        log::info!("screenshot capture is not supported on this platform");
        return;
    }

    let state = app.state::<ScreenshotState>();
    let Some(capture_id) = state.try_begin_capture() else {
        log::debug!("screenshot capture already in progress");
        return;
    };

    let monitors = match backend::monitors(app) {
        Ok(monitors) if !monitors.is_empty() => monitors,
        Ok(_) => {
            log::error!("screenshot capture found no monitors");
            state.finish_capture(capture_id);
            return;
        }
        Err(err) => {
            log::error!("screenshot capture could not read monitors: {err}");
            state.finish_capture(capture_id);
            return;
        }
    };
    let previous_foreground = backend::foreground_window();
    let source_title = backend::foreground_title(previous_foreground);
    let cursor_index = backend::cursor_monitor(app, &monitors).unwrap_or(0);
    let direct = direct_target(&state, mode, &monitors, cursor_index);
    // Selection overlays shown before the capture appear at once, like the
    // system screenshot tool; the pixels arrive while the user starts selecting.
    let early = direct.is_none() && backend::OVERLAY_BEFORE_CAPTURE;

    let capture_app = app.clone();
    let spawned = thread::Builder::new()
        .name("screenshot-capture".to_owned())
        .spawn(move || {
            if let Some(delay) = delay {
                thread::sleep(delay);
            }

            if let Err(err) = backend::ensure_capture_permission() {
                log::error!("screenshot permission is unavailable: {err}");
                fail_capture(&capture_app, capture_id, None, &err);
                return;
            }

            let target = CaptureTarget {
                mode,
                monitors,
                cursor_index,
                direct,
                previous_foreground,
                captured_at: Local::now(),
                source_title,
            };
            let early_session = if early {
                match show_early_session(&capture_app, &target, capture_id) {
                    Ok(session_id) => Some(session_id),
                    Err(err) => {
                        log::error!("present early screenshot overlays failed: {err}");
                        fail_capture(&capture_app, capture_id, Some(capture_id), &err);
                        return;
                    }
                }
            } else {
                None
            };
            let result = match early_session {
                Some(session_id) => capture_into_session(&capture_app, &target, session_id),
                None => capture(&capture_app, target, capture_id),
            };
            if let Err(err) = result {
                log::error!("screenshot capture failed: {err}");
                fail_capture(&capture_app, capture_id, early_session, &err);
            }
        });

    if let Err(err) = spawned {
        log::error!("spawn screenshot capture thread failed: {err}");
        state.finish_capture(capture_id);
    }
}

/// Stores a session without pixels and shows its overlays over the live screen.
fn show_early_session(app: &AppHandle, target: &CaptureTarget, session_id: u64) -> Result<u64> {
    let state = app.state::<ScreenshotState>();
    state.set_session(CaptureSession {
        id: session_id,
        mode: overlay_mode(target.mode),
        frames: monitor_frames(&target.monitors, None),
        windows: Vec::new(),
        cursor_label: overlay::label_for(target.cursor_index),
        previous_foreground: target.previous_foreground,
        captured_at: target.captured_at,
        source_title: target.source_title.clone(),
    });

    let (sender, receiver) = mpsc::sync_channel(1);
    let main_app = app.clone();
    app.run_on_main_thread(move || {
        let _ = sender.send(present_early(&main_app, session_id));
    })
    .map_err(|err| AppError::Other(anyhow::anyhow!(err)))?;
    receiver
        .recv_timeout(OVERLAY_PRESENT_TIMEOUT)
        .map_err(|_| {
            AppError::Other(anyhow::anyhow!("screenshot overlay presentation timed out"))
        })??;

    if app
        .state::<ScreenshotState>()
        .with_session(|session| session.id == session_id)
        != Some(true)
    {
        return Err(AppError::Other(anyhow::anyhow!(
            "screenshot capture was cancelled"
        )));
    }

    Ok(session_id)
}

struct CaptureTarget {
    mode: CaptureMode,
    monitors: Vec<MonitorGeometry>,
    cursor_index: usize,
    /// Fullscreen and repeat capture skip the overlay and go to the editor.
    direct: Option<(MonitorGeometry, PixelRect)>,
    previous_foreground: Option<isize>,
    captured_at: chrono::DateTime<Local>,
    source_title: Option<String>,
}

fn direct_target(
    state: &ScreenshotState,
    mode: CaptureMode,
    monitors: &[MonitorGeometry],
    cursor_index: usize,
) -> Option<(MonitorGeometry, PixelRect)> {
    match mode {
        CaptureMode::Fullscreen => {
            let monitor = monitors[cursor_index];
            Some((monitor, monitor.bounds))
        }
        CaptureMode::Repeat => state
            .last_area()
            .and_then(|area| backend::monitor_for_rect(monitors, area)),
        _ => None,
    }
}

/// Window capture keeps its mode; repeat without a previous area and delayed
/// capture both pick an area on the frozen screen.
fn overlay_mode(mode: CaptureMode) -> CaptureMode {
    if mode == CaptureMode::Window {
        CaptureMode::Window
    } else {
        CaptureMode::Area
    }
}

fn monitor_frames(monitors: &[MonitorGeometry], pixels: Option<Vec<Vec<u8>>>) -> Vec<MonitorFrame> {
    let mut pixels = pixels.map(Vec::into_iter);

    monitors
        .iter()
        .enumerate()
        .map(|(index, monitor)| MonitorFrame {
            label: overlay::label_for(index),
            monitor: *monitor,
            rgba: pixels.as_mut().and_then(Iterator::next).map(Arc::new),
        })
        .collect()
}

fn present_early(app: &AppHandle, session_id: u64) -> Result<()> {
    let started = Instant::now();
    overlay::present(app, session_id, overlay::Reveal::Now)?;

    log::info!(
        "screenshot overlays shown before capture in {}ms",
        started.elapsed().as_millis()
    );
    Ok(())
}

/// Ends a failed capture and, for a missing permission, opens its preference.
fn fail_capture(app: &AppHandle, capture_id: u64, early_session: Option<u64>, err: &AppError) {
    let permission_missing = backend::is_permission_error(err);
    let main_app = app.clone();

    if let Err(dispatch_err) = app.run_on_main_thread(move || {
        let state = main_app.state::<ScreenshotState>();
        let cancelled = early_session.is_some_and(|session_id| {
            cancel_active_session(
                &state,
                Some(session_id),
                || overlay::hide_all(&main_app),
                |previous| {
                    backend::restore_foreground(previous);
                },
            )
        });
        let was_current = if early_session.is_some() {
            cancelled
        } else {
            state.finish_capture(capture_id)
        };

        if permission_missing && was_current {
            if let Err(open_err) = crate::window::open_preference_with_highlight(
                &main_app,
                "permissions.screenRecording".to_owned(),
            ) {
                log::error!("open screen recording permission preference failed: {open_err}");
            }
        }
    }) {
        log::error!("dispatch failed screenshot cleanup failed: {dispatch_err}");
        let state = app.state::<ScreenshotState>();
        if let Some(session_id) = early_session {
            if state.take_session(Some(session_id)).is_some() {
                state.finish_capture(capture_id);
            }
        } else {
            state.finish_capture(capture_id);
        }
    }
}

/// Fills a session whose overlays are already on screen.
fn capture_into_session(app: &AppHandle, target: &CaptureTarget, session_id: u64) -> Result<()> {
    let started = Instant::now();
    let backend = backend::CaptureContext::new(&target.monitors)?;
    let rects = target
        .monitors
        .iter()
        .map(|monitor| monitor.bounds)
        .collect::<Vec<_>>();
    let pixels = backend.capture_rects(&rects)?;

    let state = app.state::<ScreenshotState>();
    if !state.fill_session(
        session_id,
        pixels,
        backend.list_windows(),
        backend.origin_title(target.previous_foreground),
    ) {
        log::debug!("screenshot session {session_id} ended before its pixels arrived");
        return Ok(());
    }
    log::info!(
        "screenshot {:?} froze {} monitor(s) behind visible overlays in {}ms",
        target.mode,
        target.monitors.len(),
        started.elapsed().as_millis()
    );

    let main_app = app.clone();
    app.run_on_main_thread(move || overlay::refresh(&main_app, session_id))
        .map_err(|err| AppError::Other(anyhow::anyhow!(err)))
}

fn capture(app: &AppHandle, target: CaptureTarget, capture_id: u64) -> Result<()> {
    let started = Instant::now();
    let CaptureTarget {
        mode,
        monitors,
        cursor_index,
        direct,
        previous_foreground,
        captured_at,
        source_title,
    } = target;
    let backend = backend::CaptureContext::new(&monitors)?;

    if let Some((monitor, area)) = direct {
        let rgba = backend.capture_rect(area)?;
        log::info!(
            "screenshot {mode:?} captured {}x{} in {}ms",
            area.width,
            area.height,
            started.elapsed().as_millis()
        );

        let image = CapturedImage {
            width: area.width,
            height: area.height,
            scale_factor: monitor.scale_factor,
            origin_x: area.x,
            origin_y: area.y,
            rgba,
            captured_at,
            source_title: backend.origin_title(previous_foreground).or(source_title),
        };
        let main_app = app.clone();

        return app
            .run_on_main_thread(move || {
                if !main_app
                    .state::<ScreenshotState>()
                    .finish_capture(capture_id)
                {
                    return;
                }
                if let Err(err) = editor::open(&main_app, image, monitor, None) {
                    log::error!("open screenshot editor failed: {err}");
                }
            })
            .map_err(|err| AppError::Other(anyhow::anyhow!(err)));
    }

    let rects = monitors
        .iter()
        .map(|monitor| monitor.bounds)
        .collect::<Vec<_>>();
    let pixels = backend.capture_rects(&rects)?;

    let state = app.state::<ScreenshotState>();
    let session_id = capture_id;
    let mode = overlay_mode(mode);
    if !state.set_session(CaptureSession {
        id: session_id,
        mode,
        frames: monitor_frames(&monitors, Some(pixels)),
        windows: backend.list_windows(),
        cursor_label: overlay::label_for(cursor_index),
        previous_foreground,
        captured_at,
        source_title: backend.origin_title(previous_foreground).or(source_title),
    }) {
        return Ok(());
    }
    log::info!(
        "screenshot {mode:?} froze {} monitor(s) in {}ms",
        monitors.len(),
        started.elapsed().as_millis()
    );

    let main_app = app.clone();
    app.run_on_main_thread(move || {
        if let Err(err) = overlay::present(&main_app, session_id, overlay::Reveal::AfterPaint) {
            log::error!("present screenshot overlays failed: {err}");
            cancel_capture(&main_app, Some(session_id));
        }
    })
    .map_err(|err| AppError::Other(anyhow::anyhow!(err)))
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    fn monitor(x: i32, y: i32, width: u32, height: u32) -> MonitorGeometry {
        MonitorGeometry {
            bounds: PixelRect::new(x, y, width, height),
            work_area: PixelRect::new(x, y, width, height),
            scale_factor: 1.25,
        }
    }

    #[test]
    fn stale_cancel_does_not_hide_or_restore_current_capture() {
        let state = ScreenshotState::default();
        let old_id = state.try_begin_capture().unwrap();
        let old_monitor = monitor(0, 0, 2, 1);
        assert!(state.set_session(CaptureSession {
            id: old_id,
            mode: CaptureMode::Area,
            frames: monitor_frames(&[old_monitor], None),
            windows: Vec::new(),
            cursor_label: overlay::label_for(0),
            previous_foreground: Some(42),
            captured_at: Local::now(),
            source_title: None,
        }));
        assert!(cancel_active_session(&state, Some(old_id), || {}, |_| {}));

        let new_id = state.try_begin_capture().unwrap();
        assert!(state.set_session(CaptureSession {
            id: new_id,
            mode: CaptureMode::Area,
            frames: monitor_frames(&[old_monitor], None),
            windows: Vec::new(),
            cursor_label: overlay::label_for(0),
            previous_foreground: Some(84),
            captured_at: Local::now(),
            source_title: None,
        }));
        let hidden = Cell::new(0);
        let restored = Cell::new(None);

        assert!(!cancel_active_session(
            &state,
            Some(old_id),
            || hidden.set(1),
            |previous| {
                restored.set(previous);
            }
        ));
        assert!(!state.finish_capture(old_id));
        assert_eq!(hidden.get(), 0);
        assert_eq!(restored.get(), None);
        assert_eq!(state.with_session(|session| session.id), Some(new_id));

        assert!(cancel_active_session(
            &state,
            Some(new_id),
            || hidden.set(1),
            |previous| {
                restored.set(previous);
            }
        ));
        assert_eq!(hidden.get(), 1);
        assert_eq!(restored.get(), Some(84));
    }

    #[test]
    fn delayed_capture_waits_at_least_three_seconds() {
        assert_eq!(
            capture_delay(CaptureMode::Delayed, Some(TRAY_CAPTURE_DELAY)),
            Some(DELAYED_CAPTURE_DELAY)
        );
        assert_eq!(
            capture_delay(CaptureMode::Area, Some(TRAY_CAPTURE_DELAY)),
            Some(TRAY_CAPTURE_DELAY)
        );
        assert_eq!(capture_delay(CaptureMode::Repeat, None), None);
    }
}
