//! Screenshot editor windows. Each capture opens its own editor that owns the
//! captured pixels until the window closes.

use std::thread;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};

use super::state::{CapturedImage, MonitorGeometry, ScreenshotState};
use super::EDITOR_WINDOW_LABEL_PREFIX;
use crate::core::events;
use crate::core::Result;

const TOOLBAR_HEIGHT: f64 = 52.0;
const CANVAS_MARGIN: f64 = 96.0;
const MIN_WIDTH: f64 = 960.0;
const MIN_HEIGHT: f64 = 560.0;
const WORK_AREA_RATIO: f64 = 0.9;

/// Shows an editor even if its frontend never reports the first paint.
const READY_FALLBACK: Duration = Duration::from_millis(2500);

/// Carries the label because frontend listeners receive events for every target.
#[derive(Clone, Serialize)]
struct CloseRequestPayload<'a> {
    label: &'a str,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageInfo {
    pub width: u32,
    pub height: u32,
    pub scale_factor: f64,
    pub captured_at: String,
}

impl ImageInfo {
    pub fn from_image(image: &CapturedImage) -> Self {
        Self {
            width: image.width,
            height: image.height,
            scale_factor: image.scale_factor,
            captured_at: image.captured_at.to_rfc3339(),
        }
    }
}

/// Opens a hidden editor sized around the image on the capture monitor.
/// The window is shown once the frontend has painted the image.
pub fn open(
    app: &AppHandle,
    image: CapturedImage,
    monitor: MonitorGeometry,
    handoff_overlay: Option<u64>,
) -> Result<()> {
    let started = Instant::now();
    let state = app.state::<ScreenshotState>();
    let label = format!("{EDITOR_WINDOW_LABEL_PREFIX}{}", state.next_id());
    let (width, height) = window_size(&image, &monitor);
    state.insert_image(&label, image);
    if let Some(id) = handoff_overlay {
        state.begin_editor_handoff(id, &label);
    }

    let builder = WebviewWindowBuilder::new(
        app,
        &label,
        WebviewUrl::App("index.html/#/screenshot-editor".into()),
    )
    .title("Ultra Clipboard Screenshot")
    .inner_size(width, height)
    .min_inner_size(MIN_WIDTH, 420.0)
    .accept_first_mouse(true)
    .disable_drag_drop_handler()
    .on_navigation(navigation_guard(app, "/screenshot-editor"))
    .visible(false);

    // The toolbar doubles as the title bar, like the Shottr editor.
    #[cfg(target_os = "windows")]
    let builder = builder.decorations(false);

    #[cfg(target_os = "macos")]
    let builder = builder
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true);

    let window = match builder.build() {
        Ok(window) => window,
        Err(err) => {
            state.remove_window(&label);
            return Err(anyhow::anyhow!("build screenshot editor window: {err}").into());
        }
    };

    if let Err(err) = center_on_monitor(&window, &monitor) {
        log::warn!("center screenshot editor {label} failed: {err}");
    }
    log::info!(
        "screenshot editor {label} window prepared in {}ms",
        started.elapsed().as_millis()
    );
    schedule_ready_fallback(app, label);

    Ok(())
}

pub(super) fn navigation_guard(
    app: &AppHandle,
    route: &'static str,
) -> impl Fn(&tauri::Url) -> bool + Send + 'static {
    let dev_url = if cfg!(debug_assertions) {
        app.config().build.dev_url.clone()
    } else {
        None
    };

    move |url| {
        let allowed = is_screenshot_document(url, route, dev_url.as_ref());
        if !allowed {
            log::info!("ignored navigation away from screenshot document");
        }
        allowed
    }
}

fn is_screenshot_document(url: &tauri::Url, route: &str, dev_url: Option<&tauri::Url>) -> bool {
    let app_origin = (url.scheme() == "tauri" && url.host_str() == Some("localhost"))
        || (matches!(url.scheme(), "http" | "https") && url.host_str() == Some("tauri.localhost"));
    let dev_origin = dev_url.is_some_and(|dev| dev.origin() == url.origin());

    (app_origin || dev_origin)
        && url.path().trim_end_matches('/') == "/index.html"
        && url
            .fragment()
            .and_then(|fragment| fragment.split('?').next())
            == Some(route)
}

/// Shows a painted editor or pin window and brings it to the front.
pub fn reveal(app: &AppHandle, label: &str) -> Result<()> {
    let Some(window) = app.get_webview_window(label) else {
        finish_overlay_handoff(app, label);
        return Ok(());
    };
    if window.is_visible().unwrap_or(false) {
        finish_overlay_handoff(app, label);
        return Ok(());
    }

    if let Err(err) = window.show() {
        finish_overlay_handoff(app, label);
        return Err(anyhow::anyhow!(err).into());
    }
    finish_overlay_handoff(app, label);
    window.set_focus().map_err(|err| anyhow::anyhow!(err))?;

    Ok(())
}

fn finish_overlay_handoff(app: &AppHandle, label: &str) {
    let state = app.state::<ScreenshotState>();
    state.complete_editor_handoff(label, || {
        super::overlay::hide_all(app);
    });
}

/// Asks the editor frontend to run its close flow, which confirms unsaved edits.
pub fn request_close(app: &AppHandle, label: &str) {
    let non_editor_document = app
        .get_webview_window(label)
        .and_then(|window| window.url().ok())
        .is_some_and(|url| {
            url.scheme() == "file"
                || url
                    .fragment()
                    .and_then(|fragment| fragment.split('?').next())
                    != Some("/screenshot-editor")
        });
    if non_editor_document {
        let main_app = app.clone();
        let label = label.to_owned();
        if let Err(err) = app.run_on_main_thread(move || super::close_window(&main_app, &label)) {
            log::error!("close non-editor screenshot window failed: {err}");
        }
        return;
    }

    if let Err(err) = app.emit_to(
        label,
        events::SCREENSHOT_EDITOR_CLOSE_REQUESTED,
        CloseRequestPayload { label },
    ) {
        log::warn!("emit screenshot editor close request to {label} failed: {err}");
    }
}

fn window_size(image: &CapturedImage, monitor: &MonitorGeometry) -> (f64, f64) {
    let scale = monitor.scale_factor.max(1.0);
    let max_width = f64::from(monitor.work_area.width) / scale * WORK_AREA_RATIO;
    let max_height = f64::from(monitor.work_area.height) / scale * WORK_AREA_RATIO;
    let image_width = f64::from(image.width) / image.scale_factor.max(1.0);
    let image_height = f64::from(image.height) / image.scale_factor.max(1.0);

    let width = (image_width + CANVAS_MARGIN).clamp(MIN_WIDTH.min(max_width), max_width);
    let height = (image_height + TOOLBAR_HEIGHT + CANVAS_MARGIN)
        .clamp(MIN_HEIGHT.min(max_height), max_height);

    (width.round(), height.round())
}

fn center_on_monitor(window: &WebviewWindow, monitor: &MonitorGeometry) -> Result<()> {
    let size = window.outer_size().map_err(|err| anyhow::anyhow!(err))?;
    let area = monitor.work_area;
    let x = i64::from(area.x) + (i64::from(area.width) - i64::from(size.width)).max(0) / 2;
    let y = i64::from(area.y) + (i64::from(area.height) - i64::from(size.height)).max(0) / 2;

    window
        .set_position(PhysicalPosition::new(x as i32, y as i32))
        .map_err(|err| anyhow::anyhow!(err))?;

    Ok(())
}

fn schedule_ready_fallback(app: &AppHandle, label: String) {
    let app = app.clone();

    thread::spawn(move || {
        thread::sleep(READY_FALLBACK);

        let main_app = app.clone();
        let main_label = label.clone();
        if let Err(err) = app.run_on_main_thread(move || {
            if let Err(err) = reveal(&main_app, &main_label) {
                log::warn!("reveal screenshot window {main_label} after fallback failed: {err}");
            }
        }) {
            log::warn!("screenshot window fallback dispatch failed for {label}: {err}");
        }
    });
}

#[cfg(test)]
mod tests {
    use chrono::Local;

    use super::*;
    use crate::screenshot::geometry::PixelRect;

    #[test]
    fn screenshot_navigation_stays_on_its_app_document() {
        let route = "/screenshot-editor";
        for value in [
            "tauri://localhost/index.html/#/screenshot-editor",
            "http://tauri.localhost/index.html/#/screenshot-editor",
            "https://tauri.localhost/index.html/#/screenshot-editor",
        ] {
            assert!(is_screenshot_document(&value.parse().unwrap(), route, None));
        }
        for value in [
            "file:///tmp/screenshot.png",
            "file:///tmp/index.html/#/screenshot-editor",
            "https://example.com/index.html/#/screenshot-editor",
            "tauri://localhost/index.html/#/screenshot-pin",
            "data:text/html,screenshot",
        ] {
            assert!(!is_screenshot_document(
                &value.parse().unwrap(),
                route,
                None
            ));
        }
        let dev_url = "http://localhost:1420".parse().unwrap();
        assert!(is_screenshot_document(
            &"http://localhost:1420/index.html/#/screenshot-editor"
                .parse()
                .unwrap(),
            route,
            Some(&dev_url),
        ));
        assert!(!is_screenshot_document(
            &"http://localhost:1421/index.html/#/screenshot-editor"
                .parse()
                .unwrap(),
            route,
            Some(&dev_url),
        ));
    }

    fn image(width: u32, height: u32, scale_factor: f64) -> CapturedImage {
        CapturedImage {
            width,
            height,
            scale_factor,
            origin_x: 0,
            origin_y: 0,
            rgba: Vec::new(),
            captured_at: Local::now(),
            source_title: None,
        }
    }

    fn portrait_monitor() -> MonitorGeometry {
        MonitorGeometry {
            bounds: PixelRect::new(0, 0, 2160, 3840),
            work_area: PixelRect::new(0, 47, 2160, 3733),
            scale_factor: 1.25,
        }
    }

    #[test]
    fn small_capture_uses_minimum_editor_size() {
        let (width, height) = window_size(&image(200, 100, 1.25), &portrait_monitor());

        assert_eq!((width, height), (960.0, 560.0));
    }

    #[test]
    fn fullscreen_capture_is_limited_to_work_area() {
        let (width, height) = window_size(&image(2160, 3840, 1.25), &portrait_monitor());

        assert_eq!(width, (2160.0 / 1.25 * 0.9_f64).round());
        assert_eq!(height, (3733.0 / 1.25 * 0.9_f64).round());
    }
}
