//! Platform capture backends. Both adapters expose the same functions and
//! types, and own everything about displays: listing them, the one under the
//! pointer, and where a window covering capture pixels goes. Callers outside
//! this module never convert between capture pixels and window units.

use tauri::{Position, WebviewWindow};

use super::geometry::PixelRect;
use super::state::MonitorGeometry;
use crate::core::Result;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "macos")]
pub use macos::*;
#[cfg(target_os = "windows")]
pub use windows::*;

/// A window rectangle in floating-point window units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// The monitor whose capture-pixel bounds hold the point.
pub fn monitor_at_pixel(monitors: &[MonitorGeometry], x: f64, y: f64) -> Option<usize> {
    monitors
        .iter()
        .position(|monitor| monitor.bounds.contains_point(x, y))
}

/// The monitor showing most of `area`, with the part of `area` it shows.
pub fn monitor_for_rect(
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

/// The work area of `monitor` in logical units.
pub fn work_area_logical_size(monitor: &MonitorGeometry) -> (f64, f64) {
    let scale = monitor.scale_factor.max(1.0);

    (
        f64::from(monitor.work_area.width) / scale,
        f64::from(monitor.work_area.height) / scale,
    )
}

/// Where a window of `logical` size sits when centred in the work area of `monitor`.
pub fn centered_position(monitor: &MonitorGeometry, logical: (f64, f64)) -> Position {
    let width = (logical.0 * monitor.scale_factor).round() as u32;
    let height = (logical.1 * monitor.scale_factor).round() as u32;
    let area = monitor.work_area;
    let x = i64::from(area.x) + (i64::from(area.width) - i64::from(width)).max(0) / 2;
    let y = i64::from(area.y) + (i64::from(area.height) - i64::from(height)).max(0) / 2;

    window_frame(monitor, PixelRect::new(x as i32, y as i32, width, height)).0
}

/// Centres `window` in the work area of `monitor`.
pub fn center_window(window: &WebviewWindow, monitor: &MonitorGeometry) -> Result<()> {
    // The window may sit on a display with another scale, so measure it in that
    // display's units.
    let scale = window.scale_factor().map_err(|err| anyhow::anyhow!(err))?;
    let size = window
        .outer_size()
        .map_err(|err| anyhow::anyhow!(err))?
        .to_logical::<f64>(scale);

    window
        .set_position(centered_position(monitor, (size.width, size.height)))
        .map_err(|err| anyhow::anyhow!(err))?;

    Ok(())
}

/// A frame of `width` x `height` that keeps the point at `anchor_x`/`anchor_y`
/// (0..1 of `current`) where it is.
pub fn anchored_frame(
    current: Frame,
    width: f64,
    height: f64,
    anchor_x: f64,
    anchor_y: f64,
) -> Frame {
    let pivot_x = current.x + current.width * anchor_x;
    let pivot_y = current.y + current.height * anchor_y;

    Frame {
        x: pivot_x - width * anchor_x,
        y: pivot_y - height * anchor_y,
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn monitor(x: i32, y: i32, width: u32, height: u32, scale: f64) -> MonitorGeometry {
        MonitorGeometry {
            bounds: PixelRect::new(x, y, width, height),
            work_area: PixelRect::new(x, y + 40, width, height - 40),
            scale_factor: scale,
        }
    }

    fn mixed() -> [MonitorGeometry; 2] {
        [
            monitor(0, 0, 1920, 1080, 1.0),
            monitor(1920, 0, 3840, 2160, 2.0),
        ]
    }

    #[test]
    fn finds_the_monitor_holding_a_pixel() {
        let monitors = mixed();

        assert_eq!(monitor_at_pixel(&monitors, 100.0, 100.0), Some(0));
        assert_eq!(monitor_at_pixel(&monitors, 1920.0, 0.0), Some(1));
        assert_eq!(monitor_at_pixel(&monitors, 9000.0, 0.0), None);
    }

    #[test]
    fn rect_is_assigned_to_the_monitor_showing_most_of_it() {
        let monitors = mixed();
        let (target, shown) =
            monitor_for_rect(&monitors, PixelRect::new(1800, 100, 400, 200)).unwrap();

        assert_eq!(target.bounds, monitors[1].bounds);
        assert_eq!(shown, PixelRect::new(1920, 100, 280, 200));
    }

    #[test]
    fn rect_outside_every_monitor_has_no_monitor() {
        assert!(monitor_for_rect(&mixed(), PixelRect::new(9000, 9000, 10, 10)).is_none());
    }

    #[test]
    fn work_area_is_measured_in_logical_units() {
        assert_eq!(work_area_logical_size(&mixed()[1]), (1920.0, 1060.0));
        assert_eq!(
            work_area_logical_size(&monitor(0, 0, 100, 100, 0.5)),
            (100.0, 60.0)
        );
    }

    #[test]
    fn scaling_around_the_center_keeps_it_fixed() {
        let current = Frame {
            x: 100.0,
            y: 100.0,
            width: 400.0,
            height: 200.0,
        };
        let next = anchored_frame(current, 800.0, 400.0, 0.5, 0.5);

        assert_eq!(
            next,
            Frame {
                x: -100.0,
                y: 0.0,
                width: 800.0,
                height: 400.0
            }
        );
    }

    #[test]
    fn scaling_around_a_corner_keeps_the_corner_fixed() {
        let current = Frame {
            x: 10.0,
            y: 20.0,
            width: 100.0,
            height: 50.0,
        };
        let next = anchored_frame(current, 50.0, 25.0, 1.0, 1.0);

        assert_eq!((next.x + next.width, next.y + next.height), (110.0, 70.0));
    }
}
