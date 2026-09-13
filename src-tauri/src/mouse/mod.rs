//! The non-focusable Windows clipboard window cannot use blur events for
//! auto-hide, so a low-level mouse hook detects clicks outside its bounds.

#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "windows")]
pub use windows::{disable_outside_click_hide, enable_outside_click_hide};
