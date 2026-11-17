//! Frozen selection overlays, one borderless window per monitor. Overlays only
//! select an area or window; editing always happens in the editor window.

use std::thread;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::window::Color;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use super::backend;
use super::geometry::PixelRect;
use super::state::{CaptureSession, MonitorGeometry, ScreenshotState};
use super::{CaptureMode, OVERLAY_WINDOW_LABEL_PREFIX, OVERLAY_WINDOW_TITLE};
use crate::core::events;
use crate::core::Result;
use crate::window::lifecycle;

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
    /// Whether the frozen pixels exist. An overlay shown before the capture
    /// starts without them and is told again once they arrive.
    pub ready: bool,
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
        ready: frame.rgba.is_some(),
        windows,
    })
}

/// When an overlay becomes visible.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Reveal {
    /// After the frontend draws the frozen frame (see [`reveal`]).
    AfterPaint,
    /// Right away and without focus, over the live screen, before the capture.
    Now,
}

/// Prepares one overlay per captured monitor and asks each frontend to paint.
pub fn present(app: &AppHandle, session_id: u64, when: Reveal) -> Result<()> {
    let started = Instant::now();
    let state = app.state::<ScreenshotState>();
    let Some(labels) = state
        .with_session(|session| {
            (session.id == session_id).then(|| {
                session
                    .frames
                    .iter()
                    .map(|frame| frame.label.clone())
                    .collect::<Vec<_>>()
            })
        })
        .flatten()
    else {
        return Ok(());
    };

    for label in &labels {
        let is_new = app.get_webview_window(label).is_none();
        let window = ensure_window(app, label)?;
        if !is_new {
            if let Err(err) = window.emit_to(
                label.as_str(),
                events::SCREENSHOT_OVERLAY_SESSION,
                OverlaySessionPayload {
                    label,
                    session_id: Some(session_id),
                },
            ) {
                log::warn!("emit screenshot overlay session to {label} failed: {err}");
            }
        }
        schedule_ready_timeout(app, label.clone(), session_id);

        if when == Reveal::Now {
            show(app, label, session_id, false)?;
        }
    }

    for (label, window) in app.webview_windows() {
        if label.starts_with(OVERLAY_WINDOW_LABEL_PREFIX) && !labels.contains(&label) {
            hide_window(app, &label, &window);
        }
    }

    log::info!(
        "screenshot overlay windows prepared: {} in {}ms",
        labels.len(),
        started.elapsed().as_millis()
    );

    Ok(())
}

/// Tells overlays shown before the capture that their pixels arrived, and
/// gives the overlay under the cursor focus now that the frame is taken.
pub fn refresh(app: &AppHandle, session_id: u64) {
    let state = app.state::<ScreenshotState>();
    let Some((labels, cursor_label)) = state
        .with_session(|session| {
            if session.id != session_id {
                return None;
            }

            let labels = session
                .frames
                .iter()
                .map(|frame| frame.label.clone())
                .collect::<Vec<_>>();

            Some((labels, session.cursor_label.clone()))
        })
        .flatten()
    else {
        return;
    };

    for label in &labels {
        let Some(window) = app.get_webview_window(label) else {
            continue;
        };
        if let Err(err) = window.emit_to(
            label.as_str(),
            events::SCREENSHOT_OVERLAY_SESSION,
            OverlaySessionPayload {
                label,
                session_id: Some(session_id),
            },
        ) {
            log::warn!("emit screenshot overlay frame to {label} failed: {err}");
        }
        if *label == cursor_label {
            if let Err(err) = window.set_focus() {
                log::warn!("focus screenshot overlay {label} failed: {err}");
            }
        }
    }
}

/// Builds hidden overlays ahead of the first capture so it does not wait for
/// a WebView to boot.
pub fn prewarm(app: &AppHandle, count: usize) {
    for index in 0..count {
        let started = Instant::now();
        match ensure_window(app, &label_for(index)) {
            Ok(_) => log::info!(
                "prewarm screenshot overlay {index} in {}ms",
                started.elapsed().as_millis()
            ),
            Err(err) => log::warn!("prewarm screenshot overlay {index} failed: {err}"),
        }
    }
}

/// Shows a painted overlay at its monitor bounds. The overlay under the cursor takes focus.
pub fn reveal(app: &AppHandle, label: &str, session_id: u64) -> Result<()> {
    show(app, label, session_id, true)?;
    if app.get_webview_window(label).is_some() {
        app.state::<ScreenshotState>()
            .mark_overlay_painted(session_id, label);
    }
    Ok(())
}

fn show(app: &AppHandle, label: &str, session_id: u64, allow_focus: bool) -> Result<()> {
    let state = app.state::<ScreenshotState>();
    let Some(Some((monitor, focus))) = state.with_session(|session| {
        if session.id != session_id {
            return None;
        }

        session
            .frame(label)
            .map(|frame| (frame.monitor, session.cursor_label == label))
    }) else {
        return Ok(());
    };

    let Some(window) = app.get_webview_window(label) else {
        return Ok(());
    };
    if window.is_visible().unwrap_or(false) {
        return Ok(());
    }

    apply_bounds(&window, &monitor)?;
    window
        .set_always_on_top(true)
        .map_err(|err| anyhow::anyhow!(err))?;
    window.show().map_err(|err| anyhow::anyhow!(err))?;
    if focus && allow_focus {
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
    if window.is_visible().unwrap_or(false) {
        if let Err(err) = window.hide() {
            log::warn!("hide screenshot overlay {label} failed: {err}");
            return;
        }

        lifecycle::on_hidden(app, label, "screenshot-overlay-hide");
    }

    if let Err(err) = window.emit_to(
        label,
        events::SCREENSHOT_OVERLAY_SESSION,
        OverlaySessionPayload {
            label,
            session_id: None,
        },
    ) {
        log::warn!("emit screenshot overlay reset to {label} failed: {err}");
    }
}

fn ensure_window(app: &AppHandle, label: &str) -> Result<WebviewWindow> {
    if let Some(window) = app.get_webview_window(label) {
        return Ok(window);
    }

    let builder = WebviewWindowBuilder::new(
        app,
        label,
        WebviewUrl::App("index.html/#/screenshot-overlay".into()),
    )
    .title(OVERLAY_WINDOW_TITLE)
    .decorations(false)
    .resizable(false)
    .maximizable(false)
    .minimizable(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .shadow(false)
    .accept_first_mouse(true)
    .disable_drag_drop_handler()
    .on_navigation(super::editor::navigation_guard(app, "/screenshot-overlay"))
    .focused(false)
    .visible(false);

    #[cfg(target_os = "macos")]
    let builder = builder
        .transparent(true)
        .background_color(Color(0, 0, 0, 0))
        // Hidden overlays must keep their WebView loaded between captures.
        .background_throttling(tauri::utils::config::BackgroundThrottlingPolicy::Disabled);
    #[cfg(target_os = "windows")]
    let builder = builder.background_color(Color(0, 0, 0, 255));

    builder
        .build()
        .map_err(|err| anyhow::anyhow!("build screenshot overlay window: {err}").into())
}

fn apply_bounds(window: &WebviewWindow, monitor: &MonitorGeometry) -> Result<()> {
    let (position, size) = backend::window_frame(monitor, monitor.bounds);

    window
        .set_position(position)
        .map_err(|err| anyhow::anyhow!(err))?;
    window.set_size(size).map_err(|err| anyhow::anyhow!(err))?;

    Ok(())
}

fn schedule_ready_timeout(app: &AppHandle, label: String, session_id: u64) {
    let app = app.clone();

    thread::spawn(move || {
        thread::sleep(READY_TIMEOUT);

        let main_app = app.clone();
        let main_label = label.clone();
        if let Err(err) = app.run_on_main_thread(move || {
            let state = main_app.state::<ScreenshotState>();
            cancel_unpainted_overlay(&state, session_id, &main_label, || {
                log::warn!("screenshot overlay {main_label} was not painted in time");
                super::cancel_capture(&main_app, Some(session_id));
            });
        }) {
            log::warn!("screenshot overlay timeout dispatch failed for {label}: {err}");
        }
    });
}

fn cancel_unpainted_overlay(
    state: &ScreenshotState,
    session_id: u64,
    label: &str,
    cancel: impl FnOnce(),
) -> bool {
    if !state.overlay_paint_pending(session_id, label) {
        return false;
    }
    cancel();
    true
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use chrono::Local;

    use super::*;
    use crate::screenshot::state::{MonitorFrame, MonitorGeometry};

    fn session(id: u64) -> CaptureSession {
        let bounds = PixelRect::new(0, 0, 2, 2);
        CaptureSession {
            id,
            mode: CaptureMode::Area,
            frames: vec![MonitorFrame {
                label: label_for(0),
                monitor: MonitorGeometry {
                    bounds,
                    work_area: bounds,
                    scale_factor: 1.0,
                },
                rgba: None,
            }],
            windows: Vec::new(),
            cursor_label: label_for(0),
            previous_foreground: None,
            captured_at: Local::now(),
            source_title: None,
        }
    }

    #[test]
    fn native_visibility_without_frontend_paint_still_times_out() {
        let state = ScreenshotState::default();
        let id = state.try_begin_capture().unwrap();
        assert!(state.set_session(session(id)));
        let cancelled = Cell::new(0);

        assert!(cancel_unpainted_overlay(&state, id, &label_for(0), || {
            cancelled.set(cancelled.get() + 1);
        }));
        assert_eq!(cancelled.get(), 1);
    }

    #[test]
    fn wrong_id_or_label_cannot_acknowledge_or_cancel() {
        let state = ScreenshotState::default();
        let id = state.try_begin_capture().unwrap();
        assert!(state.set_session(session(id)));
        assert!(!state.mark_overlay_painted(id + 1, &label_for(0)));
        assert!(!state.mark_overlay_painted(id, &label_for(1)));
        assert!(!cancel_unpainted_overlay(
            &state,
            id + 1,
            &label_for(0),
            || panic!("stale cancel")
        ));
        assert!(!cancel_unpainted_overlay(
            &state,
            id,
            &label_for(1),
            || panic!("wrong label")
        ));
        assert!(state.overlay_paint_pending(id, &label_for(0)));
    }

    #[test]
    fn matching_paint_prevents_timeout_and_old_timer_cannot_cancel_new_session() {
        let state = ScreenshotState::default();
        let old = state.try_begin_capture().unwrap();
        assert!(state.set_session(session(old)));
        assert!(state.mark_overlay_painted(old, &label_for(0)));
        assert!(!cancel_unpainted_overlay(
            &state,
            old,
            &label_for(0),
            || panic!("painted")
        ));
        assert!(state.finish_capture(old));

        let current = state.try_begin_capture().unwrap();
        assert!(state.set_session(session(current)));
        assert!(!cancel_unpainted_overlay(
            &state,
            old,
            &label_for(0),
            || panic!("old timer")
        ));
        assert!(state.overlay_paint_pending(current, &label_for(0)));
    }
}
