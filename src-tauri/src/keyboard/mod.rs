pub const NAV_EVENT: &str = "keyboard://nav";

mod windows;
pub use windows::{disable_navigation_keys, enable_navigation_keys};
