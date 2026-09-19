//! Frozen selection overlays, one borderless window per monitor. Overlays only
//! select an area or window; editing always happens in the editor window.

use std::thread;
use std::time::Duration;

use serde::Serialize;
use tauri::window::Color;
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};

use super::geometry::PixelRect;
use super::state::{CaptureSession, ScreenshotState};
use super::{CaptureMode, OVERLAY_WINDOW_LABEL_PREFIX};
use crate::core::Result;
use crate::window::lifecycle;

pub const OVERLAY_SESSION_EVENT: &str = "screenshot://overlay-session";

/// Ends a capture whose frontend never reports the first paint.
const READY_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct OverlaySessionPayload<'a> {
    label: &'a str,
    session_id: Option<u64>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayState {
    pub session_id: u64,
    pub mode: CaptureMode,
    pub width: u32,
    pub height: u32,
    pub scale_factor: f64,
    /// Capturable windows clipped to this monitor, in monitor-local pixels, front to back.
    pub windows: Vec<PixelRect>,
}

pub fn label_for(index: usize) -> String {
    format!("{OVERLAY_WINDOW_LABEL_PREFIX}{index}")
}

pub fn state_for(session: &CaptureSession, label: &str) -> Option<OverlayState> {
    let frame = session.frame(label)?;
    let bounds = frame.monitor.bounds;
    let windows = session
        .windows
        .iter()
        .filter_map(|window| window.intersect(&bounds))
        .map(|window| window.relative_to(bounds.x, bounds.y))
        .collect();

    Some(OverlayState {
        session_id: session.id,
        mode: session.mode,
        width: bounds.width,
        height: bounds.height,
        scale_factor: frame.monitor.scale_factor,
        windows,
    })
}

/// Prepares one hidden overlay per captured monitor and asks each frontend to paint.
/// Windows become visible in [`reveal`] after the frozen frame is drawn.
pub fn present(app: &AppHandle, session_id: u64) -> Result<()> {
    let state = app.state::<ScreenshotState>();
    let Some(labels) = state.with_session(|session| {
        session
            .frames
            .iter()
            .map(|frame| frame.label.clone())
            .collect::<Vec<_>>()
    }) else {
        return Ok(());
    };

    for label in &labels {
        let is_new = app.get_webview_window(label).is_none();
        let window = ensure_window(app, label)?;
        if !is_new {
            if let Err(err) = window.emit_to(
                label.as_str(),
                OVERLAY_SESSION_EVENT,
                OverlaySessionPayload {
                    label,
                    session_id: Some(session_id),
                },
            ) {
                log::warn!("emit screenshot overlay session to {label} failed: {err}");
            }
        }
        schedule_ready_timeout(app, label.clone(), session_id);
    }

    for (label, window) in app.webview_windows() {
        if label.starts_with(OVERLAY_WINDOW_LABEL_PREFIX) && !labels.contains(&label) {
            hide_window(app, &label, &window);
        }
    }

    Ok(())
}

/// Shows a painted overlay at its monitor bounds. The overlay under the cursor takes focus.
pub fn reveal(app: &AppHandle, label: &str, session_id: u64) -> Result<()> {
    let state = app.state::<ScreenshotState>();
    let Some(Some((bounds, focus))) = state.with_session(|session| {
        if session.id != session_id {
            return None;
        }

        session
            .frame(label)
            .map(|frame| (frame.monitor.bounds, session.cursor_label == label))
    }) else {
        return Ok(());
    };

    let Some(window) = app.get_webview_window(label) else {
        return Ok(());
    };
    if window.is_visible().unwrap_or(false) {
        return Ok(());
    }

    apply_bounds(&window, bounds)?;
    window
        .set_always_on_top(true)
        .map_err(|err| anyhow::anyhow!(err))?;
    window.show().map_err(|err| anyhow::anyhow!(err))?;
    if focus {
        window.set_focus().map_err(|err| anyhow::anyhow!(err))?;
    }

    lifecycle::on_shown(app, label);
    Ok(())
}

pub fn hide_all(app: &AppHandle) {
    for (label, window) in app.webview_windows() {
        if label.starts_with(OVERLAY_WINDOW_LABEL_PREFIX) {
            hide_window(app, &label, &window);
        }
    }
}

fn hide_window(app: &AppHandle, label: &str, window: &WebviewWindow) {
    if let Err(err) = window.emit_to(
        label,
        OVERLAY_SESSION_EVENT,
        OverlaySessionPayload {
            label,
            session_id: None,
        },
    ) {
        log::warn!("emit screenshot overlay reset to {label} failed: {err}");
    }

    if !window.is_visible().unwrap_or(false) {
        return;
    }

    if let Err(err) = window.hide() {
        log::warn!("hide screenshot overlay {label} failed: {err}");
        return;
    }

    lifecycle::on_hidden(app, label, "screenshot-overlay-hide");
}

fn ensure_window(app: &AppHandle, label: &str) -> Result<WebviewWindow> {
    if let Some(window) = app.get_webview_window(label) {
        return Ok(window);
    }

    WebviewWindowBuilder::new(
        app,
        label,
        WebviewUrl::App("index.html/#/screenshot-overlay".into()),
    )
    .title("Ultra Clipboard Capture")
    .decorations(false)
    .resizable(false)
    .maximizable(false)
    .minimizable(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .shadow(false)
    .background_color(Color(0, 0, 0, 255))
    .accept_first_mouse(true)
    .disable_drag_drop_handler()
    .focused(false)
    .visible(false)
    .build()
    .map_err(|err| anyhow::anyhow!("build screenshot overlay window: {err}").into())
}

fn apply_bounds(window: &WebviewWindow, bounds: PixelRect) -> Result<()> {
    window
        .set_position(PhysicalPosition::new(bounds.x, bounds.y))
        .map_err(|err| anyhow::anyhow!(err))?;
    window
        .set_size(PhysicalSize::new(bounds.width, bounds.height))
        .map_err(|err| anyhow::anyhow!(err))?;

    Ok(())
}

fn schedule_ready_timeout(app: &AppHandle, label: String, session_id: u64) {
    let app = app.clone();

    thread::spawn(move || {
        thread::sleep(READY_TIMEOUT);

        let main_app = app.clone();
        let main_label = label.clone();
        if let Err(err) = app.run_on_main_thread(move || {
            let is_current = main_app
                .state::<ScreenshotState>()
                .with_session(|session| session.id == session_id)
                .unwrap_or(false);
            let is_visible = main_app
                .get_webview_window(&main_label)
                .and_then(|window| window.is_visible().ok())
                .unwrap_or(false);
            if is_current && !is_visible {
                log::warn!("screenshot overlay {main_label} was not painted in time");
                super::cancel_capture(&main_app, Some(session_id));
            }
        }) {
            log::warn!("screenshot overlay timeout dispatch failed for {label}: {err}");
        }
    });
}
