//! - macOS: Command+V through CGEvent
//!
//! Clipboard writeback is handled by `clipboard::write`; this module only
//! injects the paste keystroke and relies on `WritebackGuard` for loop suppression.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "macos")]
pub use macos::simulate_paste;
#[cfg(target_os = "windows")]
pub use windows::simulate_paste;

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn simulate_paste() -> crate::core::error::Result<()> {
    Err(anyhow::anyhow!("simulate_paste not implemented on this platform").into())
}
