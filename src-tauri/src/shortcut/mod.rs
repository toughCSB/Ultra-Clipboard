//! Rust owns global shortcut registration. Frontend shortcut editing suspends
//! registration and applies the latest settings when editing finishes.

use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutEvent, ShortcutState};

use crate::core::{AppError, Result};
use crate::screenshot::{self, CaptureMode};
use crate::settings::{SettingsStore, Shortcuts};
use crate::window::{self, CLIPBOARD_WINDOW_LABEL, PREFERENCE_WINDOW_LABEL};

#[cfg(target_os = "windows")]
mod win_v;

pub const CONFLICT_EVENT: &str = "shortcut://conflict";
const RESUME_DEBOUNCE: Duration = Duration::from_millis(160);

#[derive(Debug, Clone, Serialize)]
pub struct ShortcutConflict {
    pub action: &'static str,
    pub binding: String,
    pub reason: String,
}

#[derive(Default)]
pub struct ShortcutManager {
    active: Mutex<Vec<(&'static str, Shortcut)>>,
    pause: Mutex<ShortcutPause>,
}

#[derive(Default)]
struct ShortcutPause {
    resume_epoch: u64,
    suspend_count: usize,
}

impl ShortcutPause {
    fn suspend(&mut self) -> bool {
        self.resume_epoch += 1;
        self.suspend_count += 1;

        self.suspend_count == 1
    }

    fn resume(&mut self) -> Option<bool> {
        if self.suspend_count == 0 {
            return None;
        }

        self.suspend_count -= 1;

        Some(self.suspend_count == 0)
    }

    fn next_resume_epoch(&mut self) -> u64 {
        self.resume_epoch += 1;
        self.resume_epoch
    }

    fn allows_resume(&self, epoch: u64) -> bool {
        self.resume_epoch == epoch && self.suspend_count == 0
    }

    fn suspended(&self) -> bool {
        self.suspend_count > 0
    }
}

pub fn init(app: &AppHandle, shortcuts: &Shortcuts) -> Result<()> {
    app.manage(ShortcutManager::default());
    apply(app, shortcuts)
}

pub fn suspend(app: &AppHandle) -> Result<()> {
    let manager = app.state::<ShortcutManager>();
    let should_unregister = {
        let mut pause = manager.pause.lock().expect("shortcut state poisoned");
        pause.suspend()
    };

    if should_unregister {
        unregister_active(app)?;
    }

    Ok(())
}

pub fn resume(app: &AppHandle) -> Result<()> {
    let manager = app.state::<ShortcutManager>();
    let should_schedule = {
        let mut pause = manager.pause.lock().expect("shortcut state poisoned");

        match pause.resume() {
            Some(should_schedule) => should_schedule,
            None => {
                log::warn!("resume global shortcuts called without active suspend");

                return Ok(());
            }
        }
    };

    if should_schedule {
        schedule_resume(app);
    }

    Ok(())
}

pub fn apply(app: &AppHandle, shortcuts: &Shortcuts) -> Result<()> {
    unregister_active(app)?;

    if is_suspended(app) {
        return Ok(());
    }

    let mut desired: Vec<(&'static str, &str)> = vec![
        ("open_clipboard", &shortcuts.open_clipboard),
        ("open_preference", &shortcuts.open_preference),
    ];
    if screenshot::is_supported() {
        desired.extend([
            ("capture_area", shortcuts.capture_area.as_str()),
            ("capture_fullscreen", shortcuts.capture_fullscreen.as_str()),
            ("capture_window", shortcuts.capture_window.as_str()),
            ("capture_repeat", shortcuts.capture_repeat.as_str()),
            ("capture_delayed", shortcuts.capture_delayed.as_str()),
        ]);
    }

    #[cfg(target_os = "windows")]
    win_v::set_enabled(app, shortcuts.win_v);

    let mut active = Vec::new();
    let mut claimed: Vec<String> = Vec::new();
    for (action, binding) in desired {
        if binding.trim().is_empty() {
            continue;
        }

        // Older settings can already use a new default; the earlier action keeps the binding.
        let normalized = normalize_binding(binding);
        if claimed.contains(&normalized) {
            log::warn!("skip shortcut {action}={binding}: already used by another action");
            let _ = app.emit(
                CONFLICT_EVENT,
                ShortcutConflict {
                    action,
                    binding: binding.into(),
                    reason: "already used by another Ultra Clipboard shortcut".into(),
                },
            );
            continue;
        }
        claimed.push(normalized);

        match register_one(app, action, binding) {
            Ok(shortcut) => active.push((action, shortcut)),
            Err(err) => {
                log::warn!("register shortcut {action}={binding} failed: {err}");
                let _ = app.emit(
                    CONFLICT_EVENT,
                    ShortcutConflict {
                        action,
                        binding: binding.into(),
                        reason: err.to_string(),
                    },
                );
            }
        }
    }

    *app.state::<ShortcutManager>()
        .active
        .lock()
        .expect("shortcut state poisoned") = active;
    Ok(())
}

fn is_suspended(app: &AppHandle) -> bool {
    app.state::<ShortcutManager>()
        .pause
        .lock()
        .expect("shortcut state poisoned")
        .suspended()
}

fn schedule_resume(app: &AppHandle) {
    let app = app.clone();
    let epoch = {
        let manager = app.state::<ShortcutManager>();
        let mut pause = manager.pause.lock().expect("shortcut state poisoned");
        pause.next_resume_epoch()
    };

    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(RESUME_DEBOUNCE).await;

        if !should_run_scheduled_resume(&app, epoch) {
            return;
        }

        let settings = app.state::<SettingsStore>().snapshot();
        if let Err(err) = apply(&app, &settings.shortcuts) {
            log::warn!("resume delayed global shortcuts failed: {err}");
        }
    });
}

fn should_run_scheduled_resume(app: &AppHandle, epoch: u64) -> bool {
    let manager = app.state::<ShortcutManager>();
    let pause = manager.pause.lock().expect("shortcut state poisoned");

    pause.allows_resume(epoch)
}

fn unregister_active(app: &AppHandle) -> Result<()> {
    let plugin = app.global_shortcut();
    let manager = app.state::<ShortcutManager>();

    let previous = {
        let mut guard = manager.active.lock().expect("shortcut state poisoned");
        std::mem::take(&mut *guard)
    };
    for (_, shortcut) in &previous {
        if let Err(err) = plugin.unregister(*shortcut) {
            log::warn!("unregister previous shortcut failed: {err:?}");
        }
    }

    Ok(())
}

fn register_one(app: &AppHandle, action: &'static str, binding: &str) -> Result<Shortcut> {
    let plugin = app.global_shortcut();
    let shortcut: Shortcut = binding
        .parse()
        .map_err(|err| AppError::Other(anyhow::anyhow!("parse shortcut {binding}: {err}")))?;

    if plugin.is_registered(shortcut) {
        plugin
            .unregister(shortcut)
            .map_err(|err| AppError::Other(anyhow::anyhow!(err)))?;
    }

    plugin
        .on_shortcut(shortcut, move |app, _scut, event| {
            handle_event(app, action, event);
        })
        .map_err(|err| AppError::Other(anyhow::anyhow!(err)))?;
    Ok(shortcut)
}

fn normalize_binding(binding: &str) -> String {
    binding
        .split('+')
        .map(|key| key.trim().to_ascii_lowercase())
        .filter(|key| !key.is_empty())
        .collect::<Vec<_>>()
        .join("+")
}

fn handle_event(app: &AppHandle, action: &'static str, event: ShortcutEvent) {
    if !matches!(event.state(), ShortcutState::Pressed) {
        return;
    }

    let capture_mode = match action {
        "capture_area" => Some(CaptureMode::Area),
        "capture_fullscreen" => Some(CaptureMode::Fullscreen),
        "capture_window" => Some(CaptureMode::Window),
        "capture_repeat" => Some(CaptureMode::Repeat),
        "capture_delayed" => Some(CaptureMode::Delayed),
        _ => None,
    };
    if let Some(mode) = capture_mode {
        screenshot::start_capture(app, mode);
        return;
    }

    let label = match action {
        "open_clipboard" => CLIPBOARD_WINDOW_LABEL,
        "open_preference" => PREFERENCE_WINDOW_LABEL,
        _ => return,
    };
    if let Err(err) = window::toggle_window(app, label) {
        log::warn!("toggle window via shortcut {action} failed: {err}");
    }
}
