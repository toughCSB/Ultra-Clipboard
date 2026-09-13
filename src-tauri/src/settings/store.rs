//! Persists settings with a temporary file followed by an atomic rename.
//! Serde defaults preserve compatibility with older configuration files.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use anyhow::Context;
use tauri::AppHandle;

use crate::core::{AppError, Result};

use super::model::{
    Language, Settings, WINDOW_OPEN_GROUP_PREFIX, WINDOW_OPEN_SELECTION_ALL,
    WINDOW_OPEN_SELECTION_PRESERVE,
};

const FILENAME: &str = "settings.json";

pub struct SettingsStore {
    path: RwLock<PathBuf>,
    current: RwLock<Settings>,
}

impl SettingsStore {
    pub fn new(app: &AppHandle) -> Result<Self> {
        let dir = crate::core::paths::config_dir(app)?;
        fs::create_dir_all(&dir).with_context(|| format!("failed to create dir at {dir:?}"))?;

        let path = dir.join(FILENAME);

        let current = match load_from_disk(&path) {
            Some(settings) => settings,
            None => {
                let settings = default_settings_with_system_locale();
                if let Err(err) = write_atomic(&path, &settings) {
                    log::warn!("persist first-run settings failed: {err}");
                }
                settings
            }
        };
        log::info!("settings store ready at {path:?}");

        Ok(Self {
            path: RwLock::new(path),
            current: RwLock::new(current),
        })
    }

    pub fn snapshot(&self) -> Settings {
        self.current.read().expect("settings poisoned").clone()
    }

    pub fn reset(&self) -> Result<Settings> {
        let next = default_settings_with_system_locale();

        let path = self.path();
        write_atomic(&path, &next)?;
        *self.current.write().expect("settings poisoned") = next.clone();
        Ok(next)
    }

    pub fn update(&self, patch: serde_json::Value) -> Result<Settings> {
        if !patch.is_object() {
            return Err(AppError::Other(anyhow::anyhow!(
                "settings patch must be a JSON object"
            )));
        }

        let mut guard = self.current.write().expect("settings poisoned");

        let mut merged = serde_json::to_value(&*guard)
            .context("failed to serialize current settings for merge")?;
        deep_merge(&mut merged, patch);

        let next: Settings = serde_json::from_value(merged)
            .map_err(|err| AppError::Other(anyhow::anyhow!("invalid settings patch: {err}")))?;

        validate_settings(&next)?;

        let path = self.path();
        write_atomic(&path, &next)?;
        *guard = next.clone();
        Ok(next)
    }

    pub fn replace_from_file(&self, path: &Path) -> Result<Settings> {
        let content =
            fs::read_to_string(path).with_context(|| format!("failed to read {path:?}"))?;
        let next: Settings = serde_json::from_str(&content)
            .map_err(|err| AppError::Other(anyhow::anyhow!("invalid settings file: {err}")))?;

        validate_settings(&next)?;

        let path = self.path();
        write_atomic(&path, &next)?;
        *self.current.write().expect("settings poisoned") = next.clone();
        Ok(next)
    }

    pub fn rebase(&self, app: &AppHandle) -> Result<Settings> {
        let dir = crate::core::paths::config_dir(app)?;
        fs::create_dir_all(&dir).with_context(|| format!("failed to create dir at {dir:?}"))?;
        let path = dir.join(FILENAME);
        let current = match load_from_disk(&path) {
            Some(settings) => settings,
            None => {
                let settings = default_settings_with_system_locale();
                write_atomic(&path, &settings)?;
                settings
            }
        };

        *self.path.write().expect("settings path poisoned") = path;
        *self.current.write().expect("settings poisoned") = current.clone();
        Ok(current)
    }

    fn path(&self) -> PathBuf {
        self.path.read().expect("settings path poisoned").clone()
    }
}

fn default_settings_with_system_locale() -> Settings {
    let mut settings = Settings::default();
    if let Some(tag) = tauri_plugin_os::locale() {
        settings.appearance.language = Language::from_system_locale(&tag);
        log::info!(
            "default settings language from locale {tag}: {:?}",
            settings.appearance.language
        );
    }
    settings
}

fn load_from_disk(path: &Path) -> Option<Settings> {
    if !path.exists() {
        return None;
    }

    match fs::read_to_string(path).and_then(|content| {
        serde_json::from_str::<Settings>(&content)
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))
    }) {
        Ok(settings) => Some(settings),
        Err(err) => {
            log::warn!("settings file {path:?} unreadable, using defaults: {err}");
            Some(Settings::default())
        }
    }
}

fn write_atomic(path: &Path, settings: &Settings) -> Result<()> {
    let json = serde_json::to_string_pretty(settings).context("failed to serialize settings")?;

    let tmp = path.with_extension("json.tmp");
    {
        let mut file = fs::File::create(&tmp)
            .with_context(|| format!("failed to create tmp settings at {tmp:?}"))?;
        file.write_all(json.as_bytes())
            .with_context(|| format!("failed to write tmp settings at {tmp:?}"))?;
        file.sync_all().ok();
    }
    fs::rename(&tmp, path)
        .with_context(|| format!("failed to promote tmp settings to {path:?}"))?;
    Ok(())
}

fn validate_settings(settings: &Settings) -> Result<()> {
    validate_window_open_group(&settings.clipboard.window.select_group_on_open)?;

    let open_clipboard = normalize_shortcut_value(&settings.shortcuts.open_clipboard);
    let open_preference = normalize_shortcut_value(&settings.shortcuts.open_preference);

    if open_clipboard.is_empty() || open_preference.is_empty() {
        return Ok(());
    }

    if open_clipboard == open_preference {
        return Err(AppError::Other(anyhow::anyhow!(
            "global shortcuts must be unique"
        )));
    }

    Ok(())
}

fn validate_window_open_group(value: &str) -> Result<()> {
    if value == WINDOW_OPEN_SELECTION_PRESERVE || value == WINDOW_OPEN_SELECTION_ALL {
        return Ok(());
    }

    let Some(group_id) = value.strip_prefix(WINDOW_OPEN_GROUP_PREFIX) else {
        return Err(AppError::Other(anyhow::anyhow!(
            "open group selection is invalid"
        )));
    };

    if group_id.trim().is_empty() {
        return Err(AppError::Other(anyhow::anyhow!(
            "open group selection is invalid"
        )));
    }

    Ok(())
}

fn normalize_shortcut_value(value: &str) -> String {
    value
        .split('+')
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>()
        .join("+")
}

fn deep_merge(base: &mut serde_json::Value, patch: serde_json::Value) {
    match (base, patch) {
        (serde_json::Value::Object(base_map), serde_json::Value::Object(patch_map)) => {
            for (k, v) in patch_map {
                match base_map.get_mut(&k) {
                    Some(existing) if existing.is_object() && v.is_object() => {
                        deep_merge(existing, v);
                    }
                    _ => {
                        base_map.insert(k, v);
                    }
                }
            }
        }
        (slot, patch) => {
            *slot = patch;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deep_merge_overrides_leaves_and_recurses_objects() {
        let mut base = serde_json::json!({
            "general": {"autoStart": false, "trayIcon": true},
            "clipboard": {"history": {"maxCount": 0}},
        });
        let patch = serde_json::json!({
            "general": {"autoStart": true},
            "clipboard": {"history": {"maxCount": 500}},
        });
        deep_merge(&mut base, patch);
        assert_eq!(
            base,
            serde_json::json!({
                "general": {"autoStart": true, "trayIcon": true},
                "clipboard": {"history": {"maxCount": 500}},
            })
        );
    }

    #[test]
    fn deep_merge_replaces_arrays_wholesale() {
        let mut base = serde_json::json!({"itemActions": ["copy", "star", "delete"]});
        let patch = serde_json::json!({"itemActions": ["copy", "pastePlain"]});
        deep_merge(&mut base, patch);
        assert_eq!(
            base,
            serde_json::json!({"itemActions": ["copy", "pastePlain"]})
        );
    }

    #[test]
    fn validate_settings_rejects_duplicate_global_shortcuts() {
        let mut settings = Settings::default();
        settings.shortcuts.open_preference = settings.shortcuts.open_clipboard.clone();

        assert!(validate_settings(&settings).is_err());
    }

    #[test]
    fn validate_settings_allows_empty_global_shortcuts() {
        let mut settings = Settings::default();
        settings.shortcuts.open_preference = String::new();

        assert!(validate_settings(&settings).is_ok());
    }

    #[test]
    fn missing_fields_fall_back_to_defaults() {
        let partial = r#"{"general": {"autoStart": true}}"#;
        let parsed: Settings = serde_json::from_str(partial).unwrap();
        assert!(parsed.general.auto_start);
        assert!(!parsed.general.run_as_admin);
        assert!(parsed.general.tray_icon, "default kept");
        assert_eq!(parsed.shortcuts.open_clipboard, "Alt+C");
        assert_eq!(
            parsed.update.frequency,
            crate::settings::UpdateFrequency::Daily
        );
        assert_eq!(
            parsed.clipboard.content.sort,
            crate::db::models::ClipboardItemSort::UpdatedAt
        );
        assert!(!parsed.clipboard.content.copy_then_hide_window);
        assert!(
            parsed
                .clipboard
                .content
                .delete_favorite_items_only_in_favorite_group
        );
        assert!(!parsed.clipboard.content.delete_favorite_items);
        assert!(parsed.clipboard.content.delete_favorite_confirm);
        assert!(!parsed.clipboard.content.delete_pinned_items);
        assert!(parsed.clipboard.content.delete_pinned_confirm);
        assert!(!parsed.clipboard.content.update_on_reuse);
        assert_eq!(parsed.clipboard.history.cleanup_interval_hours, 24);
        assert_eq!(parsed.clipboard.history.retention.value, 1);
        assert_eq!(
            parsed.clipboard.history.retention.unit,
            crate::settings::RetentionUnit::Months
        );
        assert!(parsed.clipboard.window.scroll_to_top_on_open);
        assert_eq!(
            parsed.clipboard.window.select_range_on_open,
            crate::settings::WindowOpenRangeSelection::Preserve
        );
        assert_eq!(
            parsed.clipboard.window.select_category_on_open,
            crate::settings::WindowOpenCategorySelection::Preserve
        );
        assert_eq!(
            parsed.clipboard.window.select_group_on_open,
            crate::settings::WINDOW_OPEN_SELECTION_PRESERVE
        );
    }

    #[test]
    fn legacy_chinese_language_deserializes_and_serializes_as_korean() {
        let parsed: Settings =
            serde_json::from_str(r#"{"appearance":{"language":"zh-CN"}}"#).unwrap();

        assert_eq!(parsed.appearance.language, Language::KoKR);

        let serialized = serde_json::to_string(&parsed).unwrap();
        assert!(serialized.contains(r#""language":"ko-KR""#));
        assert!(!serialized.contains("zh-CN"));
    }

    #[test]
    fn validate_settings_rejects_invalid_open_group_selection() {
        let mut settings = Settings::default();
        settings.clipboard.window.select_group_on_open = "invalid".to_owned();

        assert!(validate_settings(&settings).is_err());
    }
}
