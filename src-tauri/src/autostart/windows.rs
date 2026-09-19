use auto_launch::{AutoLaunch, AutoLaunchBuilder, WindowsEnableMode};
use windows_registry::CURRENT_USER;

use crate::core::{windows_args, AppError, Result};

use super::AUTO_LAUNCH_ARG;

const WINDOWS_RUN_KEY: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run";
const WINDOWS_STARTUP_APPROVED_RUN_KEY: &str =
    r"SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";
const WIN32_ERROR_FILE_NOT_FOUND: u32 = 2;

pub(super) struct PlatformAutostart {
    inner: AutoLaunch,
}

impl PlatformAutostart {
    pub(super) fn new(app_name: &str, exe_path: &str) -> Result<Self> {
        let quoted_exe_path = windows_args::quote_arg(exe_path);
        let mut builder = AutoLaunchBuilder::new();
        builder
            .set_app_name(app_name)
            .set_app_path(&quoted_exe_path)
            .set_args(&[AUTO_LAUNCH_ARG])
            .set_windows_enable_mode(WindowsEnableMode::CurrentUser);

        let inner = builder.build().map_err(|err| {
            log::error!("autostart init: build AutoLaunch failed: {err}");
            AppError::Other(anyhow::anyhow!("{err}"))
        })?;

        Ok(Self { inner })
    }

    pub(super) fn is_enabled(&self) -> Result<bool> {
        let app_name = self.inner.get_app_name();
        let registered = match CURRENT_USER
            .open(WINDOWS_RUN_KEY)
            .and_then(|key| key.get_string(app_name))
        {
            Ok(_) => true,
            Err(err) if registry_error_is_file_not_found(&err) => false,
            Err(err) => return Err(registry_app_error("read current-user autostart entry", err)),
        };
        if !registered {
            return Ok(false);
        }

        match CURRENT_USER
            .open(WINDOWS_STARTUP_APPROVED_RUN_KEY)
            .and_then(|key| key.get_value(app_name))
        {
            Ok(value) => {
                Ok(value.len() < 8 || value[value.len() - 8..].iter().all(|byte| *byte == 0))
            }
            Err(err) if registry_error_is_file_not_found(&err) => Ok(true),
            Err(err) => Err(registry_app_error(
                "read current-user startup approval",
                err,
            )),
        }
    }

    pub(super) fn set_enabled(&self, enabled: bool) -> Result<()> {
        if enabled {
            return self.inner.enable().map_err(|err| {
                log::error!("autostart enable failed: {err}");
                AppError::Other(anyhow::anyhow!("{err}"))
            });
        }

        cleanup_current_user_run_entry(self.inner.get_app_name())
    }
}

fn cleanup_current_user_run_entry(app_name: &str) -> Result<()> {
    cleanup_run_entry(app_name)
        .map_err(|err| registry_app_error("remove current-user autostart entry", err))?;
    cleanup_startup_approved_entry(app_name);
    Ok(())
}

fn cleanup_run_entry(app_name: &str) -> std::result::Result<(), windows_result::Error> {
    match CURRENT_USER
        .options()
        .write()
        .open(WINDOWS_RUN_KEY)
        .and_then(|key| key.remove_value(app_name))
    {
        Ok(()) => Ok(()),
        Err(err) if registry_error_is_file_not_found(&err) => Ok(()),
        Err(err) => Err(err),
    }
}

fn cleanup_startup_approved_entry(app_name: &str) {
    match CURRENT_USER
        .options()
        .write()
        .open(WINDOWS_STARTUP_APPROVED_RUN_KEY)
        .and_then(|key| key.remove_value(app_name))
    {
        Ok(()) => {}
        Err(err) if registry_error_is_file_not_found(&err) => {}
        Err(err) => {
            log::debug!("autostart: cleanup StartupApproved value failed: {err}");
        }
    }
}

fn registry_error_is_file_not_found(err: &windows_result::Error) -> bool {
    err.code() == windows_result::HRESULT::from_win32(WIN32_ERROR_FILE_NOT_FOUND)
}

fn registry_app_error(action: &str, err: windows_result::Error) -> AppError {
    log::error!("autostart: {action} failed: {err}");
    AppError::Other(anyhow::anyhow!("{err}"))
}
