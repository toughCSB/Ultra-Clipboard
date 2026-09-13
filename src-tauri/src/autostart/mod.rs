//! Autostart uses `auto-launch` directly to avoid known upstream issues in
//! `tauri-plugin-autostart`. Registered launches always include `--auto-launch`.

use std::env;

use tauri::{AppHandle, Manager};

use crate::core::{AppError, Result};

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "macos")]
use macos::PlatformAutostart;
#[cfg(target_os = "windows")]
use windows::PlatformAutostart;

pub(super) const AUTO_LAUNCH_ARG: &str = "--auto-launch";

pub struct AutostartManager {
    platform: PlatformAutostart,
}

pub fn init(app: &AppHandle) -> Result<()> {
    let exe = env::current_exe().map_err(|err| {
        log::error!("autostart init: current_exe failed: {err}");
        AppError::Other(anyhow::anyhow!("{err}"))
    })?;
    let exe_path = exe.to_string_lossy().to_string();

    let app_name = app.package_info().name.clone();

    let platform = PlatformAutostart::new(&app_name, &exe_path)?;

    app.manage(AutostartManager { platform });
    Ok(())
}

pub fn is_enabled(app: &AppHandle) -> Result<bool> {
    let manager = app.state::<AutostartManager>();
    manager.platform.is_enabled()
}

pub fn set_enabled(app: &AppHandle, enabled: bool) -> Result<()> {
    let manager = app.state::<AutostartManager>();
    manager.platform.set_enabled(enabled)
}

/// Align the OS autostart entry with the persisted setting during startup.
pub fn sync_enabled(app: &AppHandle, enabled: bool) -> Result<()> {
    set_enabled(app, enabled)
}

pub fn is_autostart_launch(args: &[String]) -> bool {
    args.iter().any(|arg| arg == AUTO_LAUNCH_ARG)
}

#[cfg(test)]
mod tests {
    use super::is_autostart_launch;

    #[test]
    fn detects_autostart_launch_argument() {
        let args = vec!["UltraClipboard.exe".to_owned(), "--auto-launch".to_owned()];

        assert!(is_autostart_launch(&args));
    }

    #[test]
    fn rejects_regular_launch_arguments() {
        let args = vec!["UltraClipboard.exe".to_owned()];

        assert!(!is_autostart_launch(&args));
    }
}
