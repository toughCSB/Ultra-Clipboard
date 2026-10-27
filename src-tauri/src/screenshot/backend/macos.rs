//! macOS capture through ScreenCaptureKit. Public coordinates stay in Tauri's
//! virtual-desktop physical pixels; ScreenCaptureKit points are contained here.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use block2::RcBlock;
use core_graphics::access::ScreenCaptureAccess;
use core_graphics::display::CGDisplay;
use objc2::rc::{autoreleasepool, Retained};
use objc2::{AnyThread, MainThreadMarker};
use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication, NSScreen, NSWorkspace};
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_core_graphics::{CGDataProvider, CGImage};
use objc2_foundation::{NSArray, NSError};
use objc2_screen_capture_kit::{
    SCContentFilter, SCDisplay, SCRunningApplication, SCScreenshotManager, SCShareableContent,
    SCStreamConfiguration, SCWindow,
};

use crate::core::{AppError, Result};
use crate::screenshot::geometry::{bgra_to_rgba_opaque, PixelRect};
use crate::screenshot::state::MonitorGeometry;
use crate::screenshot::OVERLAY_WINDOW_TITLE;

const CAPTURE_TIMEOUT: Duration = Duration::from_secs(8);
const SCREEN_PIXEL_FORMAT_BGRA: u32 = u32::from_be_bytes(*b"BGRA");
const SCREEN_RECORDING_PERMISSION_ERROR: &str = "screen recording permission is required";

/// Overlays are transparent and every capture excludes them, so they can be
/// shown the moment a capture starts, like the system screenshot tool.
pub const OVERLAY_BEFORE_CAPTURE: bool = true;

pub fn is_supported() -> bool {
    true
}

pub fn is_permission_error(error: &AppError) -> bool {
    error.to_string() == SCREEN_RECORDING_PERMISSION_ERROR
}

pub fn monitor_geometries() -> Result<Vec<MonitorGeometry>> {
    let mtm = MainThreadMarker::new()
        .ok_or_else(|| anyhow::anyhow!("macOS monitor enumeration requires the main thread"))?;
    let screens = NSScreen::screens(mtm);
    let mut monitors = Vec::with_capacity(screens.len());

    for screen in screens.iter() {
        let display_id = screen.CGDirectDisplayID();
        if display_id == 0 {
            continue;
        }

        let display = CGDisplay::new(display_id);
        let scale_factor = screen.backingScaleFactor();
        let screen_frame = screen.frame();
        let display_bounds = display.bounds();
        let bounds = PixelRect::new(
            scale_i32(display_bounds.origin.x, scale_factor),
            scale_i32(display_bounds.origin.y, scale_factor),
            scale_u32(screen_frame.size.width, scale_factor)?,
            scale_u32(screen_frame.size.height, scale_factor)?,
        );
        let work_area =
            screen_work_area(&screen, display_bounds.origin.x, display_bounds.origin.y)?;

        log::info!(
            "macOS monitor {display_id}: bounds={:?}, work_area={:?}, scale={scale_factor}",
            bounds,
            work_area
        );

        monitors.push(MonitorGeometry {
            bounds,
            work_area,
            scale_factor,
        });
    }

    if monitors.is_empty() {
        return Err(anyhow::anyhow!("macOS reported no active displays").into());
    }

    Ok(monitors)
}

fn screen_work_area(screen: &NSScreen, display_x: f64, display_y: f64) -> Result<PixelRect> {
    let scale_factor = screen.backingScaleFactor();
    let screen_frame = screen.frame();
    let visible_frame = screen.visibleFrame();
    let work_x = display_x + visible_frame.origin.x - screen_frame.origin.x;
    let work_y = display_y + (screen_frame.origin.y + screen_frame.size.height)
        - (visible_frame.origin.y + visible_frame.size.height);

    Ok(PixelRect::new(
        scale_i32(work_x, scale_factor),
        scale_i32(work_y, scale_factor),
        scale_u32(visible_frame.size.width, scale_factor)?,
        scale_u32(visible_frame.size.height, scale_factor)?,
    ))
}

fn scale_i32(value: f64, scale_factor: f64) -> i32 {
    (value * scale_factor).round() as i32
}

fn scale_u32(value: f64, scale_factor: f64) -> Result<u32> {
    let scaled = value * scale_factor;
    if !scaled.is_finite() || scaled < 0.0 || scaled > f64::from(u32::MAX) {
        return Err(anyhow::anyhow!("invalid scaled macOS display dimension").into());
    }

    Ok(scaled.round() as u32)
}

pub struct CaptureContext {
    displays: Vec<DisplayTarget>,
    windows: Vec<Retained<SCWindow>>,
    exclusion: Exclusion,
}

/// Keeps capture overlays out of the frame while this app's other windows,
/// such as pinned screenshots, stay capturable.
struct Exclusion {
    app: Option<Retained<SCRunningApplication>>,
    kept_windows: Vec<Retained<SCWindow>>,
}

impl CaptureContext {
    pub fn new(monitors: &[MonitorGeometry]) -> Result<Self> {
        let permission_started = Instant::now();
        let access = ScreenCaptureAccess;
        if !access.preflight() && !access.request() {
            return Err(anyhow::anyhow!(SCREEN_RECORDING_PERMISSION_ERROR).into());
        }
        log::info!(
            "macOS screenshot permission confirmed in {}ms",
            permission_started.elapsed().as_millis()
        );

        autoreleasepool(|_| {
            let snapshot_started = Instant::now();
            let snapshot = shareable_snapshot()?;
            log::info!(
                "macOS screenshot shareable content loaded in {}ms",
                snapshot_started.elapsed().as_millis()
            );

            let exclusion = own_exclusion(&snapshot);
            if exclusion.app.is_none() {
                log::warn!("macOS screenshot could not find its own app to exclude overlays");
            }

            Ok(Self {
                displays: match_displays(monitors, snapshot.displays)?,
                windows: snapshot.windows,
                exclusion,
            })
        })
    }

    pub fn capture_rect(&self, rect: PixelRect) -> Result<Vec<u8>> {
        let mut pixels = self.capture_rects(&[rect])?;

        Ok(pixels.remove(0))
    }

    /// Captures every area at once; ScreenCaptureKit serves the requests in parallel.
    pub fn capture_rects(&self, rects: &[PixelRect]) -> Result<Vec<Vec<u8>>> {
        let started = Instant::now();
        let pixels = autoreleasepool(|_| {
            let requests = rects
                .iter()
                .map(|rect| {
                    let target = self
                        .displays
                        .iter()
                        .find(|target| contains_rect(target.monitor.bounds, *rect))
                        .ok_or_else(|| {
                            anyhow::anyhow!("capture area does not fit an available display")
                        })?;
                    let source_rect =
                        display_source_rect(target.monitor, *rect).ok_or_else(|| {
                            anyhow::anyhow!("capture area is empty or outside its display")
                        })?;

                    Ok(start_capture(target, *rect, source_rect, &self.exclusion))
                })
                .collect::<Result<Vec<_>>>()?;

            requests
                .into_iter()
                .map(CaptureRequest::wait)
                .collect::<Result<Vec<_>>>()
        })?;
        log::info!(
            "macOS screenshot pixels for {} area(s) captured in {}ms",
            rects.len(),
            started.elapsed().as_millis()
        );

        Ok(pixels)
    }

    pub fn list_windows(&self) -> Vec<PixelRect> {
        let own_process = std::process::id() as i32;
        let mut rects = Vec::new();

        for window in &self.windows {
            // SAFETY: The retained ScreenCaptureKit snapshot owns every window.
            let (visible, layer, owner, frame) = unsafe {
                (
                    window.isOnScreen(),
                    window.windowLayer(),
                    window.owningApplication(),
                    window.frame(),
                )
            };
            if !visible || layer != 0 {
                continue;
            }
            if owner
                .as_ref()
                .is_some_and(|app| unsafe { app.processID() } == own_process)
            {
                continue;
            }

            for target in &self.displays {
                if let Some(rect) = window_rect_on_display(frame, target) {
                    rects.push(rect);
                }
            }
        }

        rects
    }
}

pub fn foreground_window() -> Option<isize> {
    autoreleasepool(|_| {
        NSWorkspace::sharedWorkspace()
            .frontmostApplication()
            .map(|app| app.processIdentifier() as isize)
    })
}

pub fn restore_foreground(handle: Option<isize>) {
    let Some(process_id) = handle.and_then(|handle| i32::try_from(handle).ok()) else {
        return;
    };

    autoreleasepool(|_| {
        let Some(app) = NSRunningApplication::runningApplicationWithProcessIdentifier(process_id)
        else {
            return;
        };

        if !app.activateWithOptions(NSApplicationActivationOptions::empty()) {
            log::debug!("restore frontmost application after screenshot was rejected by macOS");
        }
    });
}

struct ShareableSnapshot {
    displays: Vec<Retained<SCDisplay>>,
    windows: Vec<Retained<SCWindow>>,
    applications: Vec<Retained<SCRunningApplication>>,
}

// ScreenCaptureKit documents these objects as immutable shareable-content
// metadata. The callback retains them before this worker receives the snapshot.
unsafe impl Send for ShareableSnapshot {}

fn shareable_snapshot() -> Result<ShareableSnapshot> {
    let (sender, receiver) = mpsc::sync_channel(1);
    let completion = RcBlock::new(
        move |content: *mut SCShareableContent, error: *mut NSError| {
            let result = if let Some(error) = unsafe { error.as_ref() } {
                Err(format!("shareable content is unavailable: {error}"))
            } else if !content.is_null() {
                // SAFETY: ScreenCaptureKit owns the pointer for the callback. Retain it
                // while copying its retained display and window arrays.
                let content = unsafe { Retained::retain(content) }
                    .expect("non-null ScreenCaptureKit content must be retainable");
                Ok(ShareableSnapshot {
                    displays: unsafe { content.displays() }.to_vec(),
                    windows: unsafe { content.windows() }.to_vec(),
                    applications: unsafe { content.applications() }.to_vec(),
                })
            } else {
                Err("shareable content returned no value".to_owned())
            };
            let _ = sender.send(result);
        },
    );

    // SAFETY: The block owns its sender and remains alive until the bounded wait ends.
    unsafe {
        SCShareableContent::getShareableContentExcludingDesktopWindows_onScreenWindowsOnly_completionHandler(
            true,
            true,
            &completion,
        );
    }

    receiver
        .recv_timeout(CAPTURE_TIMEOUT)
        .map_err(|_| anyhow::anyhow!("shareable content request timed out"))?
        .map_err(|message| anyhow::anyhow!(message).into())
}

fn own_exclusion(snapshot: &ShareableSnapshot) -> Exclusion {
    let own_process = std::process::id() as i32;
    // SAFETY: The retained snapshot owns every application and window.
    let app = snapshot
        .applications
        .iter()
        .find(|app| unsafe { app.processID() } == own_process)
        .cloned();
    let kept_windows = snapshot
        .windows
        .iter()
        .filter(|window| {
            let owned = unsafe { window.owningApplication() }
                .is_some_and(|owner| unsafe { owner.processID() } == own_process);
            let is_overlay = unsafe { window.title() }
                .is_some_and(|title| title.to_string() == OVERLAY_WINDOW_TITLE);

            owned && !is_overlay
        })
        .cloned()
        .collect();

    Exclusion { app, kept_windows }
}

struct DisplayTarget {
    display: Retained<SCDisplay>,
    monitor: MonitorGeometry,
    point_frame: CGRect,
}

fn match_displays(
    monitors: &[MonitorGeometry],
    displays: Vec<Retained<SCDisplay>>,
) -> Result<Vec<DisplayTarget>> {
    let mut remaining = displays;
    let mut targets = Vec::with_capacity(monitors.len());

    for monitor in monitors {
        let expected_point_x = f64::from(monitor.bounds.x) / monitor.scale_factor;
        let expected_point_y = f64::from(monitor.bounds.y) / monitor.scale_factor;
        let best = remaining
            .iter()
            .enumerate()
            .filter_map(|(index, display)| {
                // SAFETY: Display metadata is immutable for this capture snapshot.
                let frame = unsafe { display.frame() };
                let pixel_width =
                    (unsafe { display.width() } as f64 * monitor.scale_factor).round();
                let pixel_height =
                    (unsafe { display.height() } as f64 * monitor.scale_factor).round();
                if (pixel_width - f64::from(monitor.bounds.width)).abs() > 1.0
                    || (pixel_height - f64::from(monitor.bounds.height)).abs() > 1.0
                {
                    return None;
                }

                let score = (frame.origin.x - expected_point_x).abs()
                    + (frame.origin.y - expected_point_y).abs();
                Some((index, frame, score))
            })
            .min_by(|left, right| left.2.total_cmp(&right.2));
        let Some((index, point_frame, _)) = best else {
            return Err(anyhow::anyhow!(
                "ScreenCaptureKit display does not match Tauri monitor {}x{} at {},{}",
                monitor.bounds.width,
                monitor.bounds.height,
                monitor.bounds.x,
                monitor.bounds.y
            )
            .into());
        };

        targets.push(DisplayTarget {
            display: remaining.remove(index),
            monitor: *monitor,
            point_frame,
        });
    }

    Ok(targets)
}

/// One in-flight ScreenCaptureKit request. The filter, configuration, and
/// completion block stay alive until the result arrives.
struct CaptureRequest {
    receiver: mpsc::Receiver<std::result::Result<Vec<u8>, String>>,
    _filter: Retained<SCContentFilter>,
    _configuration: Retained<SCStreamConfiguration>,
    _completion: RcBlock<dyn Fn(*mut CGImage, *mut NSError)>,
}

impl CaptureRequest {
    fn wait(self) -> Result<Vec<u8>> {
        self.receiver
            .recv_timeout(CAPTURE_TIMEOUT)
            .map_err(|_| anyhow::anyhow!("screen image request timed out"))?
            .map_err(|message| anyhow::anyhow!(message).into())
    }
}

fn start_capture(
    target: &DisplayTarget,
    rect: PixelRect,
    source_rect: CGRect,
    exclusion: &Exclusion,
) -> CaptureRequest {
    // SAFETY: The display, application, and window arrays are retained for the filter lifetime.
    let filter = unsafe {
        match &exclusion.app {
            Some(app) => SCContentFilter::initWithDisplay_excludingApplications_exceptingWindows(
                SCContentFilter::alloc(),
                &target.display,
                &NSArray::from_retained_slice(std::slice::from_ref(app)),
                &NSArray::from_retained_slice(&exclusion.kept_windows),
            ),
            None => SCContentFilter::initWithDisplay_excludingWindows(
                SCContentFilter::alloc(),
                &target.display,
                &NSArray::<SCWindow>::from_slice(&[]),
            ),
        }
    };
    // SAFETY: new returns a fully initialized configuration.
    let configuration = unsafe { SCStreamConfiguration::new() };
    // SAFETY: All values are finite, positive and valid for this retained config.
    unsafe {
        configuration.setWidth(rect.width as usize);
        configuration.setHeight(rect.height as usize);
        configuration.setPixelFormat(SCREEN_PIXEL_FORMAT_BGRA);
        configuration.setShowsCursor(false);
        configuration.setScalesToFit(true);
        configuration.setSourceRect(source_rect);
    }

    let (sender, receiver) = mpsc::sync_channel(1);
    let completion = RcBlock::new(move |image: *mut CGImage, error: *mut NSError| {
        let result = if let Some(error) = unsafe { error.as_ref() } {
            Err(format!("screen image is unavailable: {error}"))
        } else if let Some(image) = unsafe { image.as_ref() } {
            copy_bgra_image(image, rect.width, rect.height).map_err(|error| error.to_string())
        } else {
            Err("screen image returned no pixels".to_owned())
        };
        let _ = sender.send(result);
    });

    // SAFETY: The request keeps the filter, configuration and block alive until it is waited on.
    unsafe {
        SCScreenshotManager::captureImageWithFilter_configuration_completionHandler(
            &filter,
            &configuration,
            Some(&completion),
        );
    }

    CaptureRequest {
        receiver,
        _filter: filter,
        _configuration: configuration,
        _completion: completion,
    }
}

fn copy_bgra_image(image: &CGImage, width: u32, height: u32) -> anyhow::Result<Vec<u8>> {
    let image_width = CGImage::width(Some(image));
    let image_height = CGImage::height(Some(image));
    if image_width != width as usize || image_height != height as usize {
        anyhow::bail!(
            "screen image size is {image_width}x{image_height}, expected {width}x{height}"
        );
    }
    if CGImage::bits_per_component(Some(image)) != 8 || CGImage::bits_per_pixel(Some(image)) != 32 {
        anyhow::bail!("screen image is not 8-bit BGRA");
    }

    let row_bytes = CGImage::bytes_per_row(Some(image));
    let copy_bytes = width as usize * 4;
    if row_bytes < copy_bytes {
        anyhow::bail!("screen image row is shorter than its width");
    }
    let provider = CGImage::data_provider(Some(image))
        .ok_or_else(|| anyhow::anyhow!("screen image has no data provider"))?;
    let data = CGDataProvider::data(Some(&provider))
        .ok_or_else(|| anyhow::anyhow!("screen image data is unavailable"))?;
    let bytes = unsafe { data.as_bytes_unchecked() };
    let required = row_bytes
        .checked_mul(height as usize)
        .ok_or_else(|| anyhow::anyhow!("screen image is too large"))?;
    if bytes.len() < required {
        anyhow::bail!("screen image data is shorter than its rows");
    }

    let mut pixels = Vec::with_capacity(width as usize * height as usize * 4);
    for row in bytes.chunks(row_bytes).take(height as usize) {
        pixels.extend_from_slice(&row[..copy_bytes]);
    }
    bgra_to_rgba_opaque(&mut pixels);

    Ok(pixels)
}

fn contains_rect(bounds: PixelRect, rect: PixelRect) -> bool {
    rect.width > 0
        && rect.height > 0
        && rect.x >= bounds.x
        && rect.y >= bounds.y
        && rect.right() <= bounds.right()
        && rect.bottom() <= bounds.bottom()
}

fn display_source_rect(monitor: MonitorGeometry, rect: PixelRect) -> Option<CGRect> {
    if !contains_rect(monitor.bounds, rect) || monitor.scale_factor <= 0.0 {
        return None;
    }
    let scale = monitor.scale_factor;

    Some(CGRect::new(
        CGPoint::new(
            f64::from(rect.x - monitor.bounds.x) / scale,
            f64::from(rect.y - monitor.bounds.y) / scale,
        ),
        CGSize::new(
            f64::from(rect.width) / scale,
            f64::from(rect.height) / scale,
        ),
    ))
}

fn window_rect_on_display(frame: CGRect, target: &DisplayTarget) -> Option<PixelRect> {
    window_rect_in_monitor(frame, target.point_frame, target.monitor)
}

fn window_rect_in_monitor(
    frame: CGRect,
    display: CGRect,
    monitor: MonitorGeometry,
) -> Option<PixelRect> {
    let left = frame.origin.x.max(display.origin.x);
    let top = frame.origin.y.max(display.origin.y);
    let right = (frame.origin.x + frame.size.width).min(display.origin.x + display.size.width);
    let bottom = (frame.origin.y + frame.size.height).min(display.origin.y + display.size.height);
    if right - left <= 1.0 || bottom - top <= 1.0 {
        return None;
    }

    let scale = monitor.scale_factor;
    let x = f64::from(monitor.bounds.x) + (left - display.origin.x) * scale;
    let y = f64::from(monitor.bounds.y) + (top - display.origin.y) * scale;
    let width = (right - left) * scale;
    let height = (bottom - top) * scale;

    Some(PixelRect::new(
        x.round() as i32,
        y.round() as i32,
        width.round().max(1.0) as u32,
        height.round().max(1.0) as u32,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn monitor(x: i32, y: i32, width: u32, height: u32, scale: f64) -> MonitorGeometry {
        MonitorGeometry {
            bounds: PixelRect::new(x, y, width, height),
            work_area: PixelRect::new(x, y, width, height),
            scale_factor: scale,
        }
    }

    #[test]
    fn converts_retina_pixels_to_display_points() {
        let source = display_source_rect(
            monitor(-2000, 0, 2000, 1200, 2.0),
            PixelRect::new(-1900, 100, 800, 400),
        )
        .unwrap();

        assert_eq!(
            source,
            CGRect::new(CGPoint::new(50.0, 50.0), CGSize::new(400.0, 200.0))
        );
    }

    #[test]
    fn rejects_rect_crossing_a_monitor_edge() {
        let source = display_source_rect(
            monitor(0, 0, 1920, 1080, 1.0),
            PixelRect::new(1800, 100, 200, 100),
        );

        assert!(source.is_none());
    }

    #[test]
    fn clips_a_window_to_a_scaled_monitor_with_negative_origin() {
        let rect = window_rect_in_monitor(
            CGRect::new(CGPoint::new(-1100.0, 50.0), CGSize::new(300.0, 200.0)),
            CGRect::new(CGPoint::new(-1000.0, 0.0), CGSize::new(1000.0, 600.0)),
            monitor(-2000, 0, 2000, 1200, 2.0),
        )
        .unwrap();

        assert_eq!(rect, PixelRect::new(-2000, 100, 400, 400));
    }
}
