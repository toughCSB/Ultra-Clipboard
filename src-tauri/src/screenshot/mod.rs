//! Screenshot capture, frozen selection overlays, editor and pin windows, and
//! output actions. Rust owns capture, window geometry, clipboard, and files;
//! the frontend paints frames and edits pixels.

mod backend;
mod editor;
mod geometry;
mod ocr;
mod output;
mod overlay;
mod pin;
mod state;

use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use chrono::Local;
use serde::{Deserialize, Serialize};
use tauri::ipc::Request;
use tauri::{AppHandle, Manager, Window};

use crate::core::{AppError, Result};

pub use editor::ImageInfo;
pub use geometry::PixelRect;
pub use output::ExportResult;
pub use overlay::OverlayState;

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
            if let Err(err) = app.run_on_main_thread(move || match monitor_geometries(&main_app) {
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
        cancel_capture(app, None);
        return true;
    }

    if label.starts_with(PIN_WINDOW_LABEL_PREFIX) {
        close_window(app, label);
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

pub fn overlay_frame(app: &AppHandle, label: &str, session_id: u64) -> Result<Vec<u8>> {
    let frame = app
        .state::<ScreenshotState>()
        .with_session(|session| {
            if session.id != session_id {
                return None;
            }

            session.frame(label).and_then(|frame| frame.rgba.clone())
        })
        .flatten()
        .ok_or_else(|| AppError::Other(anyhow::anyhow!("capture session has ended")))?;

    Ok(frame.as_ref().clone())
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
            },
            frame.monitor,
            true,
        )
    })();

    if result.is_err() {
        overlay::hide_all(app);
        state.finish_capture();
    }

    result
}

pub fn cancel_capture(app: &AppHandle, session_id: Option<u64>) {
    let session = app.state::<ScreenshotState>().take_session(session_id);
    overlay::hide_all(app);

    if let Some(session) = session {
        backend::restore_foreground(session.previous_foreground);
    }
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
    let state = app.state::<ScreenshotState>();
    if let Some(drag_file) = state.remove_window(label) {
        if let Some(directory) = drag_file.path.parent() {
            if let Err(err) = std::fs::remove_dir_all(directory) {
                log::warn!("remove screenshot drag directory failed: {err}");
            }
        }
    }

    if state.complete_editor_handoff(label) {
        overlay::hide_all(app);
    }

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

pub async fn start_drag(app: &AppHandle, label: &str) -> Result<()> {
    output::start_drag(app, label).await
}

pub async fn copy_color(color: &str) -> Result<()> {
    output::copy_color(color).await
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
    if !state.try_begin_capture() {
        log::debug!("screenshot capture already in progress");
        return;
    }

    let monitors = match monitor_geometries(app) {
        Ok(monitors) if !monitors.is_empty() => monitors,
        Ok(_) => {
            log::error!("screenshot capture found no monitors");
            state.finish_capture();
            return;
        }
        Err(err) => {
            log::error!("screenshot capture could not read monitors: {err}");
            state.finish_capture();
            return;
        }
    };
    let cursor = app.cursor_position().ok();
    let previous_foreground = backend::foreground_window();
    let cursor_index = cursor
        .and_then(|cursor| {
            monitors
                .iter()
                .position(|monitor| monitor.bounds.contains_point(cursor.x, cursor.y))
        })
        .unwrap_or(0);
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

            let target = CaptureTarget {
                mode,
                monitors,
                cursor_index,
                direct,
                previous_foreground,
                captured_at: Local::now(),
            };
            let early_session = early.then(|| show_early_session(&capture_app, &target));
            let result = match early_session {
                Some(session_id) => capture_into_session(&capture_app, &target, session_id),
                None => capture(&capture_app, target),
            };
            if let Err(err) = result {
                log::error!("screenshot capture failed: {err}");
                fail_capture(&capture_app, early_session, &err);
            }
        });

    if let Err(err) = spawned {
        log::error!("spawn screenshot capture thread failed: {err}");
        state.finish_capture();
    }
}

/// Stores a session without pixels and shows its overlays over the live screen.
fn show_early_session(app: &AppHandle, target: &CaptureTarget) -> u64 {
    let state = app.state::<ScreenshotState>();
    let session_id = state.next_id();
    state.set_session(CaptureSession {
        id: session_id,
        mode: overlay_mode(target.mode),
        frames: monitor_frames(&target.monitors, None),
        windows: Vec::new(),
        cursor_label: overlay::label_for(target.cursor_index),
        previous_foreground: target.previous_foreground,
        captured_at: target.captured_at,
    });

    let main_app = app.clone();
    if let Err(err) = app.run_on_main_thread(move || present_early(&main_app, session_id)) {
        log::error!("dispatch early screenshot overlays failed: {err}");
    }

    session_id
}

struct CaptureTarget {
    mode: CaptureMode,
    monitors: Vec<MonitorGeometry>,
    cursor_index: usize,
    /// Fullscreen and repeat capture skip the overlay and go to the editor.
    direct: Option<(MonitorGeometry, PixelRect)>,
    previous_foreground: Option<isize>,
    captured_at: chrono::DateTime<Local>,
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
            .and_then(|area| repeat_target(monitors, area)),
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

fn present_early(app: &AppHandle, session_id: u64) {
    let started = Instant::now();
    if let Err(err) = overlay::present(app, session_id, overlay::Reveal::Now) {
        log::error!("present early screenshot overlays failed: {err}");
        cancel_capture(app, Some(session_id));
        return;
    }

    log::info!(
        "screenshot overlays shown before capture in {}ms",
        started.elapsed().as_millis()
    );
}

/// Ends a failed capture and, for a missing permission, opens its preference.
fn fail_capture(app: &AppHandle, early_session: Option<u64>, err: &AppError) {
    let permission_missing = backend::is_permission_error(err);
    let main_app = app.clone();

    if let Err(dispatch_err) = app.run_on_main_thread(move || {
        if let Some(session_id) = early_session {
            cancel_capture(&main_app, Some(session_id));
        }
        main_app.state::<ScreenshotState>().finish_capture();

        if permission_missing {
            if let Err(open_err) = crate::window::open_preference_with_highlight(
                &main_app,
                "permissions.screenRecording".to_owned(),
            ) {
                log::error!("open screen recording permission preference failed: {open_err}");
            }
        }
    }) {
        log::error!("dispatch failed screenshot cleanup failed: {dispatch_err}");
        app.state::<ScreenshotState>().finish_capture();
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
    if !state.fill_session(session_id, pixels, backend.list_windows()) {
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

fn capture(app: &AppHandle, target: CaptureTarget) -> Result<()> {
    let started = Instant::now();
    let CaptureTarget {
        mode,
        monitors,
        cursor_index,
        direct,
        previous_foreground,
        captured_at,
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
        };
        let main_app = app.clone();

        return app
            .run_on_main_thread(move || {
                main_app.state::<ScreenshotState>().finish_capture();
                if let Err(err) = editor::open(&main_app, image, monitor, false) {
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
    let session_id = state.next_id();
    let mode = overlay_mode(mode);
    state.set_session(CaptureSession {
        id: session_id,
        mode,
        frames: monitor_frames(&monitors, Some(pixels)),
        windows: backend.list_windows(),
        cursor_label: overlay::label_for(cursor_index),
        previous_foreground,
        captured_at,
    });
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

/// Finds the monitor that shows most of a previously captured area and the
/// part of the area still on it. Returns `None` when the area left every monitor.
fn repeat_target(
    monitors: &[MonitorGeometry],
    area: PixelRect,
) -> Option<(MonitorGeometry, PixelRect)> {
    monitors
        .iter()
        .filter_map(|monitor| {
            monitor
                .bounds
                .intersect(&area)
                .map(|visible| (*monitor, visible))
        })
        .max_by_key(|(_, visible)| u64::from(visible.width) * u64::from(visible.height))
}

#[cfg(target_os = "macos")]
fn monitor_geometries(_app: &AppHandle) -> Result<Vec<MonitorGeometry>> {
    backend::monitor_geometries()
}

#[cfg(target_os = "windows")]
fn monitor_geometries(app: &AppHandle) -> Result<Vec<MonitorGeometry>> {
    let monitors = app
        .available_monitors()
        .map_err(|err| anyhow::anyhow!(err))?;

    Ok(monitors
        .iter()
        .map(|monitor| {
            let position = monitor.position();
            let size = monitor.size();
            let work_area = monitor.work_area();

            MonitorGeometry {
                bounds: PixelRect::new(position.x, position.y, size.width, size.height),
                work_area: PixelRect::new(
                    work_area.position.x,
                    work_area.position.y,
                    work_area.size.width,
                    work_area.size.height,
                ),
                scale_factor: monitor.scale_factor(),
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn monitor(x: i32, y: i32, width: u32, height: u32) -> MonitorGeometry {
        MonitorGeometry {
            bounds: PixelRect::new(x, y, width, height),
            work_area: PixelRect::new(x, y, width, height),
            scale_factor: 1.25,
        }
    }

    #[test]
    fn repeat_uses_monitor_showing_most_of_the_area() {
        let monitors = [monitor(0, 0, 2160, 3840), monitor(2160, 0, 1920, 1080)];
        let (target, area) = repeat_target(&monitors, PixelRect::new(2000, 100, 400, 200)).unwrap();

        assert_eq!(target.bounds, monitors[1].bounds);
        assert_eq!(area, PixelRect::new(2160, 100, 240, 200));
    }

    #[test]
    fn repeat_falls_back_when_area_left_every_monitor() {
        let monitors = [monitor(0, 0, 1920, 1080)];

        assert!(repeat_target(&monitors, PixelRect::new(5000, 5000, 10, 10)).is_none());
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
