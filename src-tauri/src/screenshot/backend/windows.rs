//! Windows capture through GDI. The process is per-monitor DPI aware, so every
//! coordinate here is a physical pixel in virtual desktop space.

use std::ffi::c_void;
use std::mem::size_of;

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
    GetWindowThreadProcessId, IsIconic, IsWindow, IsWindowVisible, SetForegroundWindow,
    GWL_EXSTYLE, WS_EX_TOOLWINDOW,
};

use crate::core::Result;
use crate::screenshot::geometry::{bgra_to_rgba_opaque, PixelRect};
use crate::screenshot::state::MonitorGeometry;

/// Desktop shell windows cover every monitor and would hide real windows in window mode.
const SHELL_BACKGROUND_CLASSES: [&str; 2] = ["Progman", "WorkerW"];

pub fn is_supported() -> bool {
    true
}

pub struct CaptureContext;

impl CaptureContext {
    pub fn new(_monitors: &[MonitorGeometry]) -> Result<Self> {
        Ok(Self)
    }

    pub fn capture_rect(&self, rect: PixelRect) -> Result<Vec<u8>> {
        capture_rect(rect)
    }

    pub fn list_windows(&self) -> Vec<PixelRect> {
        list_windows()
    }
}

pub fn is_permission_error(_error: &crate::core::AppError) -> bool {
    false
}

/// Copies the composed desktop inside `rect` as opaque RGBA.
pub fn capture_rect(rect: PixelRect) -> Result<Vec<u8>> {
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
pub fn list_windows() -> Vec<PixelRect> {
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
