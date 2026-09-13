use crate::db::models::Platform;

#[derive(Debug, Clone)]
pub struct FrontmostApp {
    pub id: String,

    pub name: String,
    pub platform: Platform,

    pub icon_png: Option<Vec<u8>>,
}

pub fn detect_frontmost() -> Option<FrontmostApp> {
    #[cfg(target_os = "macos")]
    {
        macos::detect()
    }
    #[cfg(target_os = "windows")]
    {
        windows::detect()
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        None
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::{FrontmostApp, Platform};
    use crate::clipboard::icon;

    use std::path::PathBuf;

    use objc2::msg_send;
    use objc2::rc::{autoreleasepool, Retained};
    use objc2_app_kit::{NSRunningApplication, NSWorkspace};
    use objc2_foundation::{NSString, NSURL};

    pub(super) fn detect() -> Option<FrontmostApp> {
        autoreleasepool(|_| {
            let workspace = NSWorkspace::sharedWorkspace();
            let app = workspace.frontmostApplication()?;

            let id = app.bundleIdentifier().map(|s| s.to_string())?;
            let name = app
                .localizedName()
                .map(|s| s.to_string())
                .unwrap_or_else(|| id.clone());

            let icon_png =
                unsafe { bundle_path(&app) }.and_then(|path| icon::icon_png(&path, None));

            Some(FrontmostApp {
                id,
                name,
                platform: Platform::Macos,
                icon_png,
            })
        })
    }

    unsafe fn bundle_path(app: &NSRunningApplication) -> Option<PathBuf> {
        let url: Option<Retained<NSURL>> = msg_send![app, bundleURL];
        let url = url?;
        let path: Option<Retained<NSString>> = msg_send![&*url, path];
        Some(PathBuf::from(path?.to_string()))
    }
}

#[cfg(target_os = "windows")]
mod windows {
    use super::{FrontmostApp, Platform};
    use crate::clipboard::icon;

    use std::path::Path;

    use winapi::shared::minwindef::{DWORD, FALSE};
    use winapi::shared::windef::HWND;
    use winapi::um::handleapi::CloseHandle;
    use winapi::um::processthreadsapi::OpenProcess;
    use winapi::um::winbase::QueryFullProcessImageNameW;
    use winapi::um::winnt::PROCESS_QUERY_LIMITED_INFORMATION;
    use winapi::um::winuser::{GetForegroundWindow, GetWindowThreadProcessId};

    pub(super) fn detect() -> Option<FrontmostApp> {
        let exe_path = unsafe { foreground_exe_path() }?;

        let name = Path::new(&exe_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(&exe_path)
            .to_owned();
        let icon_png = icon::icon_png(Path::new(&exe_path), None);

        Some(FrontmostApp {
            id: exe_path,
            name,
            platform: Platform::Windows,
            icon_png,
        })
    }

    unsafe fn foreground_exe_path() -> Option<String> {
        let hwnd: HWND = GetForegroundWindow();
        if hwnd.is_null() {
            return None;
        }
        let mut pid: DWORD = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == 0 {
            return None;
        }

        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, pid);
        if handle.is_null() {
            return None;
        }

        let mut buf = [0u16; 1024];
        let mut size: DWORD = buf.len() as DWORD;
        let ok = QueryFullProcessImageNameW(handle, 0, buf.as_mut_ptr(), &mut size);
        CloseHandle(handle);

        if ok == 0 || size == 0 {
            return None;
        }
        Some(String::from_utf16_lossy(&buf[..size as usize]))
    }
}
