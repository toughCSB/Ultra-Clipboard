//! Pinned screenshots: borderless always-on-top windows that start where the
//! pixels were captured and scale around the cursor.

use std::thread;
use std::time::Duration;

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::window::Color;
use tauri::{AppHandle, Manager, Position, Size, WebviewUrl, WebviewWindowBuilder};

use super::backend;
use super::geometry::PixelRect;
use super::state::MonitorGeometry;
use super::state::{CapturedImage, ScreenshotState};
use super::{editor, output, PIN_WINDOW_LABEL_PREFIX};
use crate::core::{AppError, Result};
use crate::i18n::screenshot_menu::{self, Key};

const MENU_COPY_PREFIX: &str = "screenshot-pin::copy::";
const MENU_SAVE_PREFIX: &str = "screenshot-pin::save::";
const MENU_CLOSE_PREFIX: &str = "screenshot-pin::close::";

const MIN_SCALE: f64 = 0.1;
const MAX_SCALE: f64 = 4.0;
const MIN_SIDE: u32 = 16;
const MONITOR_RATIO: f64 = 0.9;
const READY_FALLBACK: Duration = Duration::from_millis(2000);

pub fn open(app: &AppHandle, image: CapturedImage) -> Result<()> {
    let state = app.state::<ScreenshotState>();
    let label = format!("{PIN_WINDOW_LABEL_PREFIX}{}", state.next_id());
    let (position, size) = initial_frame(app, &image);
    state.insert_image(&label, image);

    let window = WebviewWindowBuilder::new(
        app,
        &label,
        WebviewUrl::App("index.html/#/screenshot-pin".into()),
    )
    .title("Ultra Clipboard Pin")
    .decorations(false)
    .resizable(false)
    .maximizable(false)
    .minimizable(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .shadow(true)
    .background_color(Color(0, 0, 0, 255))
    .accept_first_mouse(true)
    .disable_drag_drop_handler()
    .on_navigation(editor::navigation_guard(app, "/screenshot-pin"))
    .visible(false)
    .build();

    let window = match window {
        Ok(window) => window,
        Err(err) => {
            state.remove_window(&label);
            return Err(anyhow::anyhow!("build screenshot pin window: {err}").into());
        }
    };

    window.set_size(size).map_err(|err| anyhow::anyhow!(err))?;
    window
        .set_position(position)
        .map_err(|err| anyhow::anyhow!(err))?;

    schedule_ready_fallback(app, label);
    Ok(())
}

/// Resizes a pin to `scale` of its image while keeping the point under
/// `anchor_x`/`anchor_y` (0..1 of the current window) fixed on screen.
pub fn set_scale(
    app: &AppHandle,
    label: &str,
    scale: f64,
    anchor_x: f64,
    anchor_y: f64,
) -> Result<()> {
    if !scale.is_finite() || !anchor_x.is_finite() || !anchor_y.is_finite() {
        return Err(AppError::Other(anyhow::anyhow!("pin scale is invalid")));
    }

    let image = app
        .state::<ScreenshotState>()
        .image(label)
        .ok_or_else(|| AppError::Other(anyhow::anyhow!("pin window is closed")))?;
    let window = app
        .get_webview_window(label)
        .ok_or_else(|| AppError::Other(anyhow::anyhow!("pin window is closed")))?;
    let (width, height) = target_size(&image, scale);

    backend::resize_around_anchor(
        &window,
        width,
        height,
        anchor_x.clamp(0.0, 1.0),
        anchor_y.clamp(0.0, 1.0),
    )
}

pub fn show_menu(app: &AppHandle, label: &str) -> Result<()> {
    let window = app
        .get_webview_window(label)
        .ok_or_else(|| AppError::Other(anyhow::anyhow!("pin window is closed")))?;
    let lang = crate::i18n::current_language(app);

    let copy = MenuItem::with_id(
        app,
        format!("{MENU_COPY_PREFIX}{label}"),
        screenshot_menu::label(lang, Key::Copy),
        true,
        None::<&str>,
    )
    .map_err(|err| anyhow::anyhow!(err))?;
    let save = MenuItem::with_id(
        app,
        format!("{MENU_SAVE_PREFIX}{label}"),
        screenshot_menu::label(lang, Key::Save),
        true,
        None::<&str>,
    )
    .map_err(|err| anyhow::anyhow!(err))?;
    let separator = PredefinedMenuItem::separator(app).map_err(|err| anyhow::anyhow!(err))?;
    let close = MenuItem::with_id(
        app,
        format!("{MENU_CLOSE_PREFIX}{label}"),
        screenshot_menu::label(lang, Key::Close),
        true,
        None::<&str>,
    )
    .map_err(|err| anyhow::anyhow!(err))?;
    let menu = Menu::with_items(app, &[&copy, &save, &separator, &close])
        .map_err(|err| anyhow::anyhow!(err))?;

    window
        .popup_menu(&menu)
        .map_err(|err| anyhow::anyhow!(err))?;

    Ok(())
}

/// Handles pin context menu clicks. Returns `false` for ids owned by other menus.
pub fn handle_menu_event(app: &AppHandle, id: &str) -> bool {
    if let Some(label) = id.strip_prefix(MENU_COPY_PREFIX) {
        let app = app.clone();
        let label = label.to_owned();
        tauri::async_runtime::spawn(async move {
            if let Err(err) = output::copy_window_image(&app, &label).await {
                log::error!("copy pinned screenshot failed: {err}");
            }
        });

        return true;
    }

    if let Some(label) = id.strip_prefix(MENU_SAVE_PREFIX) {
        let app = app.clone();
        let label = label.to_owned();
        tauri::async_runtime::spawn(async move {
            if let Err(err) = output::save_window_image(&app, &label).await {
                log::error!("save pinned screenshot failed: {err}");
            }
        });

        return true;
    }

    if let Some(label) = id.strip_prefix(MENU_CLOSE_PREFIX) {
        let main_app = app.clone();
        let label = label.to_owned();
        if let Err(err) = app.run_on_main_thread(move || super::close_window(&main_app, &label)) {
            log::error!("dispatch screenshot pin menu close failed: {err}");
        }
        return true;
    }

    false
}

/// Fits the pin inside the monitor the image came from, or the first one.
fn initial_frame(app: &AppHandle, image: &CapturedImage) -> (Position, Size) {
    let monitors = backend::monitors(app).unwrap_or_else(|err| {
        log::warn!("screenshot pin could not read monitors: {err}");
        Vec::new()
    });

    pin_frame(&monitors, image)
}

fn pin_frame(monitors: &[MonitorGeometry], image: &CapturedImage) -> (Position, Size) {
    let monitor = backend::monitor_at_pixel(
        monitors,
        f64::from(image.origin_x),
        f64::from(image.origin_y),
    )
    .and_then(|index| monitors.get(index))
    .or_else(|| monitors.first());
    let Some(monitor) = monitor else {
        return backend::window_frame(&capture_monitor(image), capture_area(image));
    };

    backend::window_frame(monitor, fit_inside(image, monitor.bounds))
}

fn capture_area(image: &CapturedImage) -> PixelRect {
    PixelRect::new(image.origin_x, image.origin_y, image.width, image.height)
}

/// Stands in for the monitor when none could be read: the capture itself.
fn capture_monitor(image: &CapturedImage) -> MonitorGeometry {
    MonitorGeometry {
        bounds: capture_area(image),
        work_area: capture_area(image),
        scale_factor: image.scale_factor,
    }
}

fn fit_inside(image: &CapturedImage, area: PixelRect) -> PixelRect {
    let max_width = f64::from(area.width) * MONITOR_RATIO;
    let max_height = f64::from(area.height) * MONITOR_RATIO;
    let ratio = (max_width / f64::from(image.width))
        .min(max_height / f64::from(image.height))
        .min(1.0);
    let width = ((f64::from(image.width) * ratio).round() as u32).max(MIN_SIDE);
    let height = ((f64::from(image.height) * ratio).round() as u32).max(MIN_SIDE);
    let max_x = (area.right() - i64::from(width)).max(i64::from(area.x));
    let max_y = (area.bottom() - i64::from(height)).max(i64::from(area.y));
    let x = i64::from(image.origin_x).clamp(i64::from(area.x), max_x);
    let y = i64::from(image.origin_y).clamp(i64::from(area.y), max_y);

    PixelRect::new(x as i32, y as i32, width, height)
}

/// The pin size in pixels for `scale` of its image.
fn target_size(image: &CapturedImage, scale: f64) -> (u32, u32) {
    let scale = scale.clamp(MIN_SCALE, MAX_SCALE);

    (
        ((f64::from(image.width) * scale).round() as u32).max(MIN_SIDE),
        ((f64::from(image.height) * scale).round() as u32).max(MIN_SIDE),
    )
}

fn schedule_ready_fallback(app: &AppHandle, label: String) {
    let app = app.clone();

    thread::spawn(move || {
        thread::sleep(READY_FALLBACK);

        let main_app = app.clone();
        let main_label = label.clone();
        if let Err(err) = app.run_on_main_thread(move || {
            if let Err(err) = super::reveal_painted_window(&main_app, &main_label) {
                log::warn!("reveal screenshot pin {main_label} after fallback failed: {err}");
            }
        }) {
            log::warn!("screenshot pin fallback dispatch failed for {label}: {err}");
        }
    });
}

#[cfg(test)]
mod tests {
    use chrono::Local;

    use super::*;

    fn image(origin_x: i32, origin_y: i32, width: u32, height: u32) -> CapturedImage {
        CapturedImage {
            width,
            height,
            scale_factor: 1.25,
            origin_x,
            origin_y,
            rgba: Vec::new(),
            captured_at: Local::now(),
            source_title: None,
        }
    }

    #[test]
    fn pin_starts_at_capture_origin_when_it_fits() {
        let bounds = fit_inside(&image(400, 900, 320, 180), PixelRect::new(0, 0, 2160, 3840));

        assert_eq!(bounds, PixelRect::new(400, 900, 320, 180));
    }

    #[test]
    fn oversized_pin_shrinks_and_stays_on_monitor() {
        let bounds = fit_inside(&image(0, 0, 2160, 3840), PixelRect::new(0, 0, 2160, 3840));

        assert_eq!(bounds.width, 1944);
        assert_eq!(bounds.height, 3456);
        assert!(bounds.right() <= 2160 && bounds.bottom() <= 3840);
    }

    #[test]
    fn scaling_is_clamped() {
        let image = image(0, 0, 400, 200);

        assert_eq!(target_size(&image, 100.0).0, 1600);
        assert_eq!(target_size(&image, 0.001).1, 20);
    }

    fn monitor(x: i32, width: u32, height: u32, scale: f64) -> MonitorGeometry {
        let bounds = PixelRect::new(x, 0, width, height);

        MonitorGeometry {
            bounds,
            work_area: bounds,
            scale_factor: scale,
        }
    }

    #[test]
    fn pin_without_monitors_still_goes_through_the_backend() {
        let image = image(400, 900, 320, 180);

        assert_eq!(
            pin_frame(&[], &image),
            backend::window_frame(&capture_monitor(&image), capture_area(&image))
        );
    }

    #[test]
    fn pin_uses_the_monitor_holding_the_capture_origin() {
        let monitors = [monitor(0, 1920, 1080, 1.0), monitor(1920, 3840, 2160, 2.0)];
        let image = image(2000, 100, 320, 180);

        assert_eq!(
            pin_frame(&monitors, &image),
            backend::window_frame(&monitors[1], PixelRect::new(2000, 100, 320, 180))
        );
    }

    #[test]
    fn pin_off_every_monitor_falls_back_to_the_first() {
        let monitors = [monitor(0, 1920, 1080, 1.0), monitor(1920, 3840, 2160, 2.0)];
        let image = image(9000, 9000, 320, 180);

        assert_eq!(
            pin_frame(&monitors, &image),
            backend::window_frame(&monitors[0], fit_inside(&image, monitors[0].bounds))
        );
    }
}
