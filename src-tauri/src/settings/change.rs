//! Single entry point for runtime settings writes and the OS state that follows them.

use std::path::Path;

use tauri::{AppHandle, Emitter, Manager};

use super::{Settings, SettingsStore};
use crate::core::events;
use crate::core::Result;
use crate::{admin, autostart, shortcut, tray, window};

pub enum SettingsWrite<'a> {
    Patch(serde_json::Value),
    Reset,
    Restore(Box<Settings>),
    /// Backup overwrite: the database was replaced, so sync is always reconfigured.
    ReplaceFromFile(&'a Path),
    /// Storage relocation: every store moved, so sync is always reconfigured.
    Rebase,
}

impl SettingsWrite<'_> {
    fn swaps_storage(&self) -> bool {
        matches!(self, Self::ReplaceFromFile(_) | Self::Rebase)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnEffectFailure {
    /// Direct user changes: a failed reversible effect restores the previous settings.
    RollBack,
    /// Writes that follow an irreversible step such as a data import or move.
    LogOnly,
}

/// Commits a direct user patch, rolling back when a reversible effect fails.
pub async fn commit_patch(app: &AppHandle, patch: serde_json::Value) -> Result<Settings> {
    commit(app, SettingsWrite::Patch(patch), OnEffectFailure::RollBack).await
}

/// Persists a settings write, applies the OS state it changes, then emits
/// `settings://updated` once.
pub async fn commit(
    app: &AppHandle,
    write: SettingsWrite<'_>,
    on_failure: OnEffectFailure,
) -> Result<Settings> {
    let store = app.state::<SettingsStore>();
    let previous = store.snapshot();
    let swaps_storage = write.swaps_storage();
    let next = match write {
        SettingsWrite::Patch(patch) => store.update(patch)?,
        SettingsWrite::Reset => store.reset()?,
        SettingsWrite::Restore(settings) => store.restore(*settings)?,
        SettingsWrite::ReplaceFromFile(path) => store.replace_from_file(path)?,
        SettingsWrite::Rebase => store.rebase(app)?,
    };

    let effects = plan(&previous, &next, swaps_storage);

    for (index, effect) in effects.iter().enumerate() {
        let Err(err) = apply_effect(app, *effect, &next).await else {
            continue;
        };

        if on_failure == OnEffectFailure::RollBack && effect.is_reversible() {
            roll_back(app, previous, &effects[..=index]).await;
            return Err(err);
        }

        log::warn!("apply settings effect {effect:?} failed: {err}");
    }

    emit_settings_updated(app, &next);

    Ok(next)
}

async fn roll_back(app: &AppHandle, previous: Settings, applied: &[SettingsEffect]) {
    if let Err(err) = app.state::<SettingsStore>().restore(previous.clone()) {
        log::warn!("restore previous settings after a failed effect failed: {err}");
        return;
    }

    for effect in applied {
        if let Err(err) = apply_effect(app, *effect, &previous).await {
            log::warn!("restore settings effect {effect:?} failed: {err}");
        }
    }
}

async fn apply_effect(app: &AppHandle, effect: SettingsEffect, settings: &Settings) -> Result<()> {
    match effect {
        SettingsEffect::Sync => crate::sync::reconfigure(app).await,
        SettingsEffect::Autostart => autostart::set_enabled(app, settings.general.auto_start),
        SettingsEffect::DockIcon => window::show_taskbar_icon(app, settings.general.dock_icon),
        SettingsEffect::Shortcuts => shortcut::apply(app, &settings.shortcuts),
        SettingsEffect::Tray => tray::apply(app, settings),
        SettingsEffect::AdminTask => {
            admin::sync_scheduled_task(settings.general.run_as_admin);
            Ok(())
        }
    }
}

fn emit_settings_updated(app: &AppHandle, settings: &Settings) {
    if let Err(err) = app.emit(events::SETTINGS_UPDATED, settings) {
        log::warn!("emit settings updated event failed: {err}");
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SettingsEffect {
    Sync,
    Autostart,
    DockIcon,
    Shortcuts,
    Tray,
    AdminTask,
}

impl SettingsEffect {
    /// Effects whose failure is visible to the user roll the settings write back.
    fn is_reversible(self) -> bool {
        matches!(self, Self::Sync | Self::Autostart | Self::DockIcon)
    }
}

/// Lists the effects a change needs, reversible ones first so a failure
/// leaves as little applied OS state as possible.
fn plan(previous: &Settings, next: &Settings, swaps_storage: bool) -> Vec<SettingsEffect> {
    let checks = [
        (
            SettingsEffect::Sync,
            swaps_storage
                || previous.sync != next.sync
                || previous.clipboard.sensitive != next.clipboard.sensitive,
        ),
        (
            SettingsEffect::Autostart,
            previous.general.auto_start != next.general.auto_start,
        ),
        (
            SettingsEffect::DockIcon,
            previous.general.dock_icon != next.general.dock_icon,
        ),
        (
            SettingsEffect::Shortcuts,
            previous.shortcuts != next.shortcuts,
        ),
        (
            SettingsEffect::Tray,
            previous.general.tray_icon != next.general.tray_icon
                || previous.appearance.language != next.appearance.language,
        ),
        (
            SettingsEffect::AdminTask,
            previous.general.run_as_admin != next.general.run_as_admin,
        ),
    ];

    checks
        .into_iter()
        .filter_map(|(effect, changed)| changed.then_some(effect))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Language;

    #[test]
    fn unchanged_settings_need_no_effects() {
        let settings = Settings::default();

        assert_eq!(plan(&settings, &settings.clone(), false), Vec::new());
    }

    #[test]
    fn storage_swap_reconfigures_sync_once_even_when_unchanged() {
        let previous = Settings::default();
        let mut next = previous.clone();
        next.clipboard.sensitive.redact_secrets = !previous.clipboard.sensitive.redact_secrets;

        assert_eq!(
            plan(&previous, &previous.clone(), true),
            vec![SettingsEffect::Sync]
        );
        assert_eq!(plan(&previous, &next, true), vec![SettingsEffect::Sync]);
    }

    #[test]
    fn changed_shortcut_reapplies_shortcuts() {
        let previous = Settings::default();
        let mut next = previous.clone();
        next.shortcuts.open_clipboard = "Alt+Shift+V".into();

        assert_eq!(
            plan(&previous, &next, false),
            vec![SettingsEffect::Shortcuts]
        );
    }

    #[test]
    fn changed_language_rebuilds_tray() {
        let previous = Settings::default();
        let mut next = previous.clone();
        next.appearance.language = Language::EnUS;

        assert_eq!(plan(&previous, &next, false), vec![SettingsEffect::Tray]);
    }

    #[test]
    fn changed_sensitive_rules_reconfigure_sync() {
        let previous = Settings::default();
        let mut next = previous.clone();
        next.clipboard.sensitive.redact_secrets = !previous.clipboard.sensitive.redact_secrets;

        assert_eq!(plan(&previous, &next, false), vec![SettingsEffect::Sync]);
    }

    #[test]
    fn metadata_only_writes_need_no_effects() {
        let previous = Settings::default();
        let mut next = previous.clone();
        next.onboarding.completed = true;
        next.update.last_checked_at = Some("2026-10-26T00:00:00Z".into());

        assert_eq!(plan(&previous, &next, false), Vec::new());
    }

    #[test]
    fn reversible_effects_come_before_logged_ones() {
        let previous = Settings::default();
        let mut next = previous.clone();
        next.general.run_as_admin = true;
        next.general.tray_icon = false;
        next.shortcuts.win_v = !previous.shortcuts.win_v;
        next.general.dock_icon = true;
        next.general.auto_start = true;
        next.sync.enabled = !previous.sync.enabled;

        let effects = plan(&previous, &next, false);

        assert_eq!(
            effects,
            vec![
                SettingsEffect::Sync,
                SettingsEffect::Autostart,
                SettingsEffect::DockIcon,
                SettingsEffect::Shortcuts,
                SettingsEffect::Tray,
                SettingsEffect::AdminTask,
            ]
        );
        assert_eq!(
            effects
                .iter()
                .map(|e| e.is_reversible())
                .collect::<Vec<_>>(),
            vec![true, true, true, false, false, false]
        );
    }
}
