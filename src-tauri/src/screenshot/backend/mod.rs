//! Platform capture backends. Both expose the same free functions so the
//! overlay, editor, and output layers stay platform-independent.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "macos")]
pub use macos::*;
#[cfg(target_os = "windows")]
pub use windows::*;
