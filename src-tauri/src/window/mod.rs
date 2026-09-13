pub mod lifecycle;
pub(super) mod position;
pub mod preview;
mod state;

#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "macos")]
pub use macos::handle_reopen;
pub use state::WindowStateStore;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, Mutex};

use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder, Window};

use crate::core::Result;
use crate::settings::{SettingsStore, WindowPosition};

pub const CLIPBOARD_WINDOW_LABEL: &str = "clipboard";
pub const PREFERENCE_WINDOW_LABEL: &str = "preference";
pub const CLIPBOARD_PREVIEW_WINDOW_LABEL: &str = "clipboard-preview";
pub const ONBOARDING_WINDOW_LABEL: &str = "onboarding";
pub const UPDATE_WINDOW_LABEL: &str = "update";

const PREFERENCE_HIGHLIGHT_EVENT: &str = "preference://highlight-setting";

static PENDING_PREFERENCE_HIGHLIGHT: LazyLock<Mutex<Option<String>>> =
    LazyLock::new(|| Mutex::new(None));

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct PreferenceHighlightPayload {
    setting_id: String,
}

static CLIPBOARD_WINDOW_PINNED: AtomicBool = AtomicBool::new(false);

static CLIPBOARD_WINDOW_AUTO_HIDE_SUSPENDED: AtomicBool = AtomicBool::new(false);

pub fn is_clipboard_window_pinned() -> bool {
    CLIPBOARD_WINDOW_PINNED.load(Ordering::Relaxed)
}

pub fn should_auto_hide_clipboard_window() -> bool {
    !CLIPBOARD_WINDOW_PINNED.load(Ordering::Relaxed)
        && !CLIPBOARD_WINDOW_AUTO_HIDE_SUSPENDED.load(Ordering::Relaxed)
}

pub fn set_clipboard_window_pinned(pinned: bool) {
    CLIPBOARD_WINDOW_PINNED.store(pinned, Ordering::Relaxed);
}

pub fn set_clipboard_window_auto_hide_suspended(suspended: bool) {
    CLIPBOARD_WINDOW_AUTO_HIDE_SUSPENDED.store(suspended, Ordering::Relaxed);
}

pub fn set_clipboard_window_editing(app_handle: &AppHandle, editing: bool) -> Result<()> {
    #[cfg(target_os = "windows")]
    return windows::set_clipboard_window_editing(app_handle, editing);

    #[cfg(target_os = "macos")]
    {
        let _ = app_handle;
        let _ = editing;

        Ok(())
    }
}

const WINDOW_VISIBILITY_EVENT: &str = "window://visibility";

#[derive(Clone, serde::Serialize)]
struct WindowVisibilityPayload<'a> {
    label: &'a str,
    visible: bool,
}

pub(super) fn emit_visibility(app_handle: &AppHandle, label: &str, visible: bool) {
    if let Err(err) = app_handle.emit(
        WINDOW_VISIBILITY_EVENT,
        WindowVisibilityPayload { label, visible },
    ) {
        log::error!("emit window visibility failed: {err:?}");
    }
}

pub(super) fn get_window(app_handle: &AppHandle, label: &str) -> Result<WebviewWindow> {
    app_handle
        .get_webview_window(label)
        .ok_or_else(|| anyhow::anyhow!("window not found: {label}").into())
}

pub fn show_window(app_handle: &AppHandle, label: &str) -> Result<()> {
    if app_handle.get_webview_window(label).is_none() {
        if let Some(build) = lifecycle::rebuild_fn(label) {
            build(app_handle)?;
        }
    }

    if label == CLIPBOARD_WINDOW_LABEL {
        if let Err(err) = apply_clipboard_window_layout(app_handle) {
            log::warn!("apply clipboard window layout failed: {err}");
        }
    } else if label == ONBOARDING_WINDOW_LABEL {
        if let Err(err) = position_window(app_handle, label, WindowPosition::Center) {
            log::warn!("center onboarding window failed: {err}");
        }
    } else {
        let visible = get_window(app_handle, label)?.is_visible().unwrap_or(false);

        if !visible {
            if let Err(err) = state::restore_window_state(app_handle, label) {
                log::warn!("restore window state failed for {label}: {err}");
            }
        }
    }

    #[cfg(target_os = "macos")]
    let result = macos::show_window(app_handle, label);
    #[cfg(target_os = "windows")]
    let result = windows::show_window(app_handle, label);
    if result.is_ok() && !delays_clipboard_visibility_event(label) {
        if label == CLIPBOARD_WINDOW_LABEL {
            preview::resume_after_clipboard_show();
        }
        emit_visibility(app_handle, label, true);
        lifecycle::on_shown(app_handle, label);
    }
    result
}

fn delays_clipboard_visibility_event(label: &str) -> bool {
    cfg!(target_os = "macos") && label == CLIPBOARD_WINDOW_LABEL
}

pub fn hide_window(app_handle: &AppHandle, label: &str) -> Result<()> {
    if let Err(err) = state::save_window_state(app_handle, label) {
        log::warn!("save window state on hide failed for {label}: {err}");
    }

    if label == CLIPBOARD_WINDOW_LABEL {
        preview::suppress_for_clipboard_hide(app_handle);
    }

    #[cfg(target_os = "macos")]
    let result = macos::hide_window(app_handle, label);
    #[cfg(target_os = "windows")]
    let result = windows::hide_window(app_handle, label);
    if result.is_ok() {
        emit_visibility(app_handle, label, false);
        lifecycle::on_hidden(app_handle, label, "hide");
    }
    result
}

pub fn toggle_window(app_handle: &AppHandle, label: &str) -> Result<()> {
    let visible = app_handle
        .get_webview_window(label)
        .and_then(|window| window.is_visible().ok())
        .unwrap_or(false);
    if visible {
        hide_window(app_handle, label)
    } else {
        show_window(app_handle, label)
    }
}

pub fn show_taskbar_icon(app_handle: &AppHandle, visible: bool) -> Result<()> {
    #[cfg(target_os = "macos")]
    return macos::show_taskbar_icon(app_handle, visible);
    #[cfg(target_os = "windows")]
    return windows::show_taskbar_icon(app_handle, visible);
}

pub fn position_window(app_handle: &AppHandle, label: &str, pos: WindowPosition) -> Result<()> {
    let window = get_window(app_handle, label)?;
    position::position_window(&window, pos)
}

fn apply_clipboard_window_layout(app_handle: &AppHandle) -> Result<()> {
    let Some(store) = app_handle.try_state::<SettingsStore>() else {
        return Ok(());
    };
    let snap = store.snapshot();
    let position = snap.clipboard.window.position;

    let _ = state::restore_window_state(app_handle, CLIPBOARD_WINDOW_LABEL)?;

    if matches!(position, WindowPosition::Remember) {
        return Ok(());
    }

    let window = get_window(app_handle, CLIPBOARD_WINDOW_LABEL)?;
    position::position_window(&window, position)
}

pub fn save_all_window_states(app_handle: &AppHandle) {
    for label in app_handle.webview_windows().into_keys() {
        if let Err(err) = state::save_window_state(app_handle, &label) {
            log::warn!("save window state on exit failed for {label}: {err}");
        }
    }
}

pub fn intercept_close_request(window: &Window) -> bool {
    if window.label() == ONBOARDING_WINDOW_LABEL {
        return true;
    }

    if let Err(err) = state::save_window_state(window.app_handle(), window.label()) {
        log::warn!(
            "save window state on close failed for {}: {err}",
            window.label()
        );
    }

    if let Err(err) = window.hide() {
        log::error!("hide window on close failed: {err:?}");
    } else {
        emit_visibility(window.app_handle(), window.label(), false);
        lifecycle::on_hidden(window.app_handle(), window.label(), "close");
    }
    true
}

pub fn build_preference_window(app_handle: &AppHandle) -> Result<()> {
    if app_handle
        .get_webview_window(PREFERENCE_WINDOW_LABEL)
        .is_some()
    {
        return Ok(());
    }

    let builder = WebviewWindowBuilder::new(
        app_handle,
        PREFERENCE_WINDOW_LABEL,
        WebviewUrl::App("index.html/#/preference".into()),
    )
    .title("Ultra Clipboard Preference")
    .inner_size(960.0, 600.0)
    .min_inner_size(960.0, 600.0)
    .center()
    .maximizable(false)
    .skip_taskbar(true)
    .accept_first_mouse(true)
    .disable_drag_drop_handler()
    .visible(false);

    #[cfg(target_os = "macos")]
    let builder = builder
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true);

    builder
        .build()
        .map_err(|err| anyhow::anyhow!("build preference window: {err}"))?;

    Ok(())
}

pub fn build_update_window(app_handle: &AppHandle) -> Result<()> {
    if app_handle.get_webview_window(UPDATE_WINDOW_LABEL).is_some() {
        return Ok(());
    }

    let builder = WebviewWindowBuilder::new(
        app_handle,
        UPDATE_WINDOW_LABEL,
        WebviewUrl::App("index.html/#/update".into()),
    )
    .title("Ultra Clipboard Update")
    .inner_size(520.0, 230.0)
    .min_inner_size(520.0, 230.0)
    .center()
    .maximizable(false)
    .resizable(false)
    .skip_taskbar(true)
    .accept_first_mouse(true)
    .disable_drag_drop_handler()
    .decorations(true)
    .transparent(false)
    .visible(false);

    #[cfg(target_os = "macos")]
    let builder = builder
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true);

    builder
        .build()
        .map_err(|err| anyhow::anyhow!("build update window: {err}"))?;

    Ok(())
}

pub fn build_onboarding_window(app_handle: &AppHandle) -> Result<()> {
    if app_handle
        .get_webview_window(ONBOARDING_WINDOW_LABEL)
        .is_some()
    {
        return Ok(());
    }

    WebviewWindowBuilder::new(
        app_handle,
        ONBOARDING_WINDOW_LABEL,
        WebviewUrl::App("index.html/#/onboarding".into()),
    )
    .title("Ultra Clipboard Onboarding")
    .inner_size(900.0, 600.0)
    .center()
    .resizable(false)
    .maximizable(false)
    .decorations(false)
    .transparent(true)
    .accept_first_mouse(true)
    .disable_drag_drop_handler()
    .visible(false)
    .build()
    .map_err(|err| anyhow::anyhow!("build onboarding window: {err}"))?;

    Ok(())
}

pub fn open_onboarding(app_handle: &AppHandle) -> Result<()> {
    if app_handle
        .get_webview_window(ONBOARDING_WINDOW_LABEL)
        .is_none()
    {
        build_onboarding_window(app_handle)?;
    }

    show_window(app_handle, ONBOARDING_WINDOW_LABEL)
}

pub fn open_preference_with_highlight(app_handle: &AppHandle, setting_id: String) -> Result<()> {
    let exists = app_handle
        .get_webview_window(PREFERENCE_WINDOW_LABEL)
        .is_some();

    if !exists {
        set_pending_preference_highlight(setting_id.clone());
    }

    show_window(app_handle, PREFERENCE_WINDOW_LABEL)?;

    if exists {
        app_handle
            .emit_to(
                PREFERENCE_WINDOW_LABEL,
                PREFERENCE_HIGHLIGHT_EVENT,
                PreferenceHighlightPayload { setting_id },
            )
            .map_err(|err| anyhow::anyhow!("emit preference highlight: {err}"))?;
    }

    Ok(())
}

fn set_pending_preference_highlight(setting_id: String) {
    let mut guard = PENDING_PREFERENCE_HIGHLIGHT
        .lock()
        .unwrap_or_else(|poisoned| {
            log::error!("pending preference highlight mutex poisoned on set, recovering");
            poisoned.into_inner()
        });
    *guard = Some(setting_id);
}

pub fn take_pending_preference_highlight() -> Option<String> {
    let mut guard = PENDING_PREFERENCE_HIGHLIGHT
        .lock()
        .unwrap_or_else(|poisoned| {
            log::error!("pending preference highlight mutex poisoned on take, recovering");
            poisoned.into_inner()
        });
    guard.take()
}
