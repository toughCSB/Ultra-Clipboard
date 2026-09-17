//! macOS capture is not implemented yet. Every entry point reports that explicitly
//! so shortcuts, tray items, and preferences can stay hidden on macOS.

use crate::core::Result;
use crate::screenshot::geometry::PixelRect;

pub fn is_supported() -> bool {
    false
}

pub fn capture_rect(_rect: PixelRect) -> Result<Vec<u8>> {
    Err(anyhow::anyhow!("screenshot capture is not available on macOS yet").into())
}

pub fn list_windows() -> Vec<PixelRect> {
    Vec::new()
}

pub fn foreground_window() -> Option<isize> {
    None
}

pub fn restore_foreground(_handle: Option<isize>) {}
