//! Windows capture through GDI. The process is per-monitor DPI aware, so every
//! coordinate here is a physical pixel in virtual desktop space.

use std::ffi::c_void;
use std::mem::size_of;
use std::time::Instant;

use windows::Win32::Foundation::{BOOL, HWND, LPARAM, RECT};
use windows::Win32::Graphics::Dwm::{
    DwmGetWindowAttribute, DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS,
};
use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits,
    ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, SRCCOPY,
};
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClassNameW, GetForegroundWindow, GetWindowLongPtrW, GetWindowRect,
    GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindow,
    IsWindowVisible, SetForegroundWindow, GWL_EXSTYLE, WS_EX_TOOLWINDOW,
};

use tauri::{AppHandle, PhysicalPosition, PhysicalSize, Position, Size, WebviewWindow};

use super::{anchored_frame, monitor_at_pixel, Frame};
use crate::core::Result;
use crate::screenshot::geometry::{bgra_to_rgba_opaque, PixelRect};
use crate::screenshot::state::MonitorGeometry;

/// Desktop shell windows cover every monitor and would hide real windows in window mode.
const SHELL_BACKGROUND_CLASSES: [&str; 2] = ["Progman", "WorkerW"];
const MAX_PARALLEL_MONITORS: usize = 4;

/// GDI copies the composed desktop, so an overlay shown before the capture
/// would be in the frame.
pub const OVERLAY_BEFORE_CAPTURE: bool = false;

pub fn is_supported() -> bool {
    true
}

/// Reads the monitors in virtual desktop pixels. Any thread may call this.
pub fn monitors(app: &AppHandle) -> Result<Vec<MonitorGeometry>> {
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

/// Windows needs no capture permission.
pub fn ensure_capture_permission() -> Result<()> {
    Ok(())
}

/// The monitor under the mouse pointer.
pub fn cursor_monitor(app: &AppHandle, monitors: &[MonitorGeometry]) -> Option<usize> {
    let cursor = app.cursor_position().ok()?;

    monitor_at_pixel(monitors, cursor.x, cursor.y)
}

/// Where a window covering `rect` goes; capture pixels are screen pixels here.
pub fn window_frame(_monitor: &MonitorGeometry, rect: PixelRect) -> (Position, Size) {
    (
        PhysicalPosition::new(rect.x, rect.y).into(),
        PhysicalSize::new(rect.width, rect.height).into(),
    )
}

/// Resizes a window to `width` x `height` pixels while the point at
/// `anchor_x`/`anchor_y` (0..1 of the current window) stays fixed on screen.
pub fn resize_around_anchor(
    window: &WebviewWindow,
    width: u32,
    height: u32,
    anchor_x: f64,
    anchor_y: f64,
) -> Result<()> {
    let position = window
        .outer_position()
        .map_err(|err| anyhow::anyhow!(err))?;
    let size = window.inner_size().map_err(|err| anyhow::anyhow!(err))?;
    let next = anchored_physical(
        PixelRect::new(position.x, position.y, size.width, size.height),
        width,
        height,
        anchor_x,
        anchor_y,
    );

    window
        .set_size(PhysicalSize::new(next.width, next.height))
        .map_err(|err| anyhow::anyhow!(err))?;
    window
        .set_position(PhysicalPosition::new(next.x, next.y))
        .map_err(|err| anyhow::anyhow!(err))?;

    Ok(())
}

fn anchored_physical(
    current: PixelRect,
    width: u32,
    height: u32,
    anchor_x: f64,
    anchor_y: f64,
) -> PixelRect {
    let next = anchored_frame(
        Frame {
            x: f64::from(current.x),
            y: f64::from(current.y),
            width: f64::from(current.width),
            height: f64::from(current.height),
        },
        f64::from(width),
        f64::from(height),
        anchor_x,
        anchor_y,
    );

    PixelRect::new(next.x.round() as i32, next.y.round() as i32, width, height)
}

pub struct CaptureContext;

impl CaptureContext {
    pub fn new(_monitors: &[MonitorGeometry]) -> Result<Self> {
        Ok(Self)
    }

    pub fn capture_rect(&self, rect: PixelRect) -> Result<Vec<u8>> {
        capture_rect(rect)
    }

    pub fn capture_rects(&self, rects: &[PixelRect]) -> Result<Vec<Vec<u8>>> {
        if rects.len() <= 1 {
            return rects.iter().map(|rect| capture_rect(*rect)).collect();
        }

        let started = Instant::now();
        let mut pixels = Vec::with_capacity(rects.len());
        for group in rects.chunks(MAX_PARALLEL_MONITORS) {
            // Each task acquires and releases its own GDI contexts on the same thread.
            let captured = std::thread::scope(|scope| {
                let tasks = group
                    .iter()
                    .map(|&rect| scope.spawn(move || capture_rect(rect)))
                    .collect::<Vec<_>>();
                let outcomes = tasks
                    .into_iter()
                    .map(|task| {
                        task.join().unwrap_or_else(|_| {
                            Err(anyhow::anyhow!("screenshot monitor capture task panicked").into())
                        })
                    })
                    .collect::<Vec<_>>();
                outcomes.into_iter().collect::<Result<Vec<_>>>()
            })?;
            pixels.extend(captured);
        }
        log::info!(
            "Windows screenshot pixels for {} monitor(s) captured in {}ms",
            rects.len(),
            started.elapsed().as_millis()
        );
        Ok(pixels)
    }

    pub fn list_windows(&self) -> Vec<PixelRect> {
        list_windows()
    }

    pub fn origin_title(&self, _handle: Option<isize>) -> Option<String> {
        None
    }
}

pub fn is_permission_error(_error: &crate::core::AppError) -> bool {
    false
}

/// Copies the composed desktop inside `rect` as opaque RGBA.
fn capture_rect(rect: PixelRect) -> Result<Vec<u8>> {
    let byte_len = rect
        .byte_len()
        .filter(|len| *len > 0)
        .ok_or_else(|| anyhow::anyhow!("capture area is empty"))?;
    let width =
        i32::try_from(rect.width).map_err(|_| anyhow::anyhow!("capture area is too wide"))?;
    let height =
        i32::try_from(rect.height).map_err(|_| anyhow::anyhow!("capture area is too tall"))?;
    let mut pixels = vec![0u8; byte_len];

    // SAFETY: every GDI handle created here is released before returning, and the
    // pixel buffer holds exactly width * height * 4 bytes for the 32-bit DIB.
    let copied_rows = unsafe {
        let screen_dc = GetDC(HWND(0));
        if screen_dc.0 == 0 {
            return Err(anyhow::anyhow!("screen device context is unavailable").into());
        }

        let memory_dc = CreateCompatibleDC(screen_dc);
        let bitmap = CreateCompatibleBitmap(screen_dc, width, height);
        let previous = SelectObject(memory_dc, bitmap);
        let blit = BitBlt(
            memory_dc, 0, 0, width, height, screen_dc, rect.x, rect.y, SRCCOPY,
        );
        // GetDIBits requires the bitmap to be deselected from every device context.
        SelectObject(memory_dc, previous);

        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let rows = if blit.is_ok() {
            GetDIBits(
                memory_dc,
                bitmap,
                0,
                rect.height,
                Some(pixels.as_mut_ptr().cast::<c_void>()),
                &mut info,
                DIB_RGB_COLORS,
            )
        } else {
            0
        };

        let _ = DeleteObject(bitmap);
        let _ = DeleteDC(memory_dc);
        ReleaseDC(HWND(0), screen_dc);

        blit.map_err(|err| anyhow::anyhow!("screen copy failed: {err}"))?;
        rows
    };

    if copied_rows != height {
        return Err(anyhow::anyhow!("screen pixels could not be read").into());
    }

    bgra_to_rgba_opaque(&mut pixels);
    Ok(pixels)
}

/// Lists visible top-level windows of other processes, front to back.
fn list_windows() -> Vec<PixelRect> {
    let mut context = WindowEnumContext {
        // SAFETY: GetCurrentProcessId has no preconditions.
        own_process_id: unsafe { GetCurrentProcessId() },
        rects: Vec::new(),
    };

    // SAFETY: the callback only runs during this call, while `context` is alive.
    let result = unsafe {
        EnumWindows(
            Some(collect_window),
            LPARAM(&mut context as *mut WindowEnumContext as isize),
        )
    };
    if let Err(err) = result {
        log::warn!("enumerate windows for screenshot failed: {err}");
    }

    context.rects
}

pub fn foreground_window() -> Option<isize> {
    // SAFETY: GetForegroundWindow has no preconditions.
    let hwnd = unsafe { GetForegroundWindow() };

    (hwnd.0 != 0).then_some(hwnd.0)
}

pub fn foreground_title(handle: Option<isize>) -> Option<String> {
    let hwnd = HWND(handle?);
    // SAFETY: The handle came from GetForegroundWindow; an invalidated handle returns no text.
    unsafe {
        let len = GetWindowTextLengthW(hwnd);
        if len <= 0 {
            return None;
        }
        let mut text = vec![0u16; len as usize + 1];
        let copied = GetWindowTextW(hwnd, &mut text);
        (copied > 0).then(|| String::from_utf16_lossy(&text[..copied as usize]))
    }
}

pub fn restore_foreground(handle: Option<isize>) {
    let Some(handle) = handle else {
        return;
    };
    let hwnd = HWND(handle);

    // SAFETY: IsWindow validates the stale handle before it is used.
    unsafe {
        if IsWindow(hwnd).as_bool() && !SetForegroundWindow(hwnd).as_bool() {
            log::debug!("restore foreground after screenshot cancel was rejected by Windows");
        }
    }
}

struct WindowEnumContext {
    own_process_id: u32,
    rects: Vec<PixelRect>,
}

unsafe extern "system" fn collect_window(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let context = &mut *(lparam.0 as *mut WindowEnumContext);
    if let Some(rect) = capturable_window_rect(hwnd, context.own_process_id) {
        context.rects.push(rect);
    }

    BOOL(1)
}

unsafe fn capturable_window_rect(hwnd: HWND, own_process_id: u32) -> Option<PixelRect> {
    if !IsWindowVisible(hwnd).as_bool() || IsIconic(hwnd).as_bool() {
        return None;
    }

    let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
    if ex_style & WS_EX_TOOLWINDOW.0 != 0 {
        return None;
    }

    let mut process_id = 0u32;
    GetWindowThreadProcessId(hwnd, Some(&mut process_id));
    if process_id == own_process_id {
        return None;
    }

    let mut cloaked = 0u32;
    let cloaked_result = DwmGetWindowAttribute(
        hwnd,
        DWMWA_CLOAKED,
        (&mut cloaked as *mut u32).cast::<c_void>(),
        size_of::<u32>() as u32,
    );
    if cloaked_result.is_ok() && cloaked != 0 {
        return None;
    }

    if is_shell_background(hwnd) {
        return None;
    }

    let mut bounds = RECT::default();
    let frame_result = DwmGetWindowAttribute(
        hwnd,
        DWMWA_EXTENDED_FRAME_BOUNDS,
        (&mut bounds as *mut RECT).cast::<c_void>(),
        size_of::<RECT>() as u32,
    );
    if frame_result.is_err() {
        GetWindowRect(hwnd, &mut bounds).ok()?;
    }

    let width = bounds.right - bounds.left;
    let height = bounds.bottom - bounds.top;
    if width <= 1 || height <= 1 {
        return None;
    }

    Some(PixelRect::new(
        bounds.left,
        bounds.top,
        width as u32,
        height as u32,
    ))
}

unsafe fn is_shell_background(hwnd: HWND) -> bool {
    let mut buffer = [0u16; 64];
    let len = GetClassNameW(hwnd, &mut buffer);
    if len <= 0 {
        return false;
    }

    let class_name = String::from_utf16_lossy(&buffer[..len as usize]);

    SHELL_BACKGROUND_CLASSES.contains(&class_name.as_str())
}

#[cfg(test)]
mod tests {
    use super::super::centered_position;
    use super::*;

    fn monitor(x: i32, scale: f64) -> MonitorGeometry {
        let bounds = PixelRect::new(x, 0, 1920, 1080);

        MonitorGeometry {
            bounds,
            work_area: bounds,
            scale_factor: scale,
        }
    }

    #[test]
    fn window_frame_is_in_screen_pixels_on_any_monitor() {
        let (position, size) =
            window_frame(&monitor(1920, 2.0), PixelRect::new(2000, 50, 300, 200));

        assert_eq!(
            position,
            Position::Physical(PhysicalPosition::new(2000, 50))
        );
        assert_eq!(size, Size::Physical(PhysicalSize::new(300, 200)));
    }

    #[test]
    fn centered_window_sits_in_the_work_area() {
        let monitors = [monitor(0, 1.0), monitor(1920, 1.5)];
        let position = centered_position(&monitors[1], (960.0, 560.0));

        assert_eq!(
            position,
            Position::Physical(PhysicalPosition::new(
                1920 + (1920 - 1440) / 2,
                (1080 - 840) / 2
            ))
        );
    }

    #[test]
    fn anchored_resize_keeps_the_anchor_in_screen_pixels() {
        let next = anchored_physical(PixelRect::new(100, 100, 400, 200), 800, 400, 0.5, 0.5);

        assert_eq!(next, PixelRect::new(-100, 0, 800, 400));
    }
}
