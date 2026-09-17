//! Settings data model. `#[serde(default)]` keeps newly added fields
//! compatible with existing configuration files.

use serde::{Deserialize, Serialize};

use crate::db::models::ClipboardItemSort;

pub const WINDOW_OPEN_SELECTION_PRESERVE: &str = "preserve";
pub const WINDOW_OPEN_SELECTION_ALL: &str = "all";
pub const WINDOW_OPEN_GROUP_PREFIX: &str = "group:";

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub general: General,
    pub appearance: Appearance,
    pub shortcuts: Shortcuts,
    pub clipboard: Clipboard,
    pub onboarding: Onboarding,
    pub update: Update,
    pub webdav: WebDav,
    pub sync: SyncSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct SyncSettings {
    pub enabled: bool,
    pub bind_address: String,
    pub listen_port: u16,
    pub device_id: String,
    pub peers: Vec<SyncPeerSettings>,
}

impl Default for SyncSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            bind_address: String::new(),
            listen_port: 45_321,
            device_id: uuid::Uuid::new_v4().to_string(),
            peers: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct SyncPeerSettings {
    pub id: String,
    pub address: String,
    pub port: u16,
    pub secret_reference: String,
}

impl Default for SyncPeerSettings {
    fn default() -> Self {
        Self {
            id: String::new(),
            address: String::new(),
            port: 45_321,
            secret_reference: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct General {
    pub auto_start: bool,
    /// Windows: persist the user's intent to run Ultra Clipboard with administrator privileges.
    pub run_as_admin: bool,

    /// macOS menu bar or Windows system tray icon.
    pub tray_icon: bool,

    /// macOS Dock or Windows taskbar icon.
    pub dock_icon: bool,
}

impl Default for General {
    fn default() -> Self {
        Self {
            auto_start: false,
            run_as_admin: false,
            tray_icon: true,
            dock_icon: false,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct Onboarding {
    /// First-run progress; business data remains persisted by its own settings.
    pub completed: bool,
    pub last_step: u32,
    pub legacy_import: OnboardingLegacyImport,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct OnboardingLegacyImport {
    /// Lightweight status for legacy-data import; the onboarding flow owns the actual import.
    pub checked: bool,
    pub imported: bool,
    pub import_types: Vec<OnboardingLegacyImportType>,
    pub imported_at: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum OnboardingLegacyImportType {
    Normal,
    Favorite,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct Appearance {
    pub theme: Theme,
    pub language: Language,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            theme: Theme::Auto,
            language: Language::KoKR,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    Auto,
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum Language {
    #[default]
    #[serde(rename = "ko-KR", alias = "zh-CN")]
    KoKR,
    #[serde(rename = "en-US")]
    EnUS,
}

impl Language {
    /// Maps the system locale to a supported language.
    pub fn from_system_locale(tag: &str) -> Self {
        let lower = tag.to_ascii_lowercase();
        if lower.starts_with("ko") || lower.starts_with("zh") {
            Self::KoKR
        } else {
            Self::EnUS
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct Shortcuts {
    /// Global shortcut that opens the clipboard window.
    pub open_clipboard: String,

    /// Global shortcut that opens preferences.
    pub open_preference: String,

    /// Windows-only Win+V replacement for the system clipboard history panel.
    pub win_v: bool,

    /// Global shortcut that freezes the screen for an area screenshot.
    pub capture_area: String,

    /// Global shortcut that captures the monitor under the cursor.
    pub capture_fullscreen: String,

    /// Global shortcut that freezes the screen for a window screenshot.
    pub capture_window: String,

    /// Global shortcut that captures the last confirmed area again.
    pub capture_repeat: String,

    /// Global shortcut that starts an area screenshot after a short delay.
    pub capture_delayed: String,
}

impl Default for Shortcuts {
    fn default() -> Self {
        Self {
            open_clipboard: "Alt+C".into(),
            open_preference: "Alt+X".into(),
            win_v: false,
            capture_area: "Alt+Shift+2".into(),
            capture_fullscreen: "Alt+Shift+1".into(),
            capture_window: String::new(),
            capture_repeat: String::new(),
            capture_delayed: String::new(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct Clipboard {
    pub capture: Capture,
    pub content: Content,
    pub display: Display,
    pub sensitive: Sensitive,
    pub history: History,
    pub search: Search,
    pub window: Window,
    pub preview: Preview,
    pub feedback: Feedback,
    pub filters: Filters,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct Capture {
    pub text: bool,
    pub html: bool,
    pub rtf: bool,
    pub image: bool,
    pub files: bool,

    /// Maximum collected text size in MB. `0` means unlimited.
    pub max_text_mb: u32,

    /// Maximum collected image size in MB. `0` means unlimited.
    pub max_image_mb: u32,

    /// Priority when the clipboard provides multiple representations.
    pub order: Vec<CaptureKind>,
}

impl Default for Capture {
    fn default() -> Self {
        Self {
            text: true,
            html: true,
            rtf: true,
            image: true,
            files: true,
            max_text_mb: 4,
            max_image_mb: 20,
            order: CaptureKind::default_order(),
        }
    }
}

impl Capture {
    /// Returns the text byte limit; `None` means unlimited.
    pub fn max_text_bytes(&self) -> Option<u64> {
        mb_to_bytes(self.max_text_mb)
    }

    pub fn max_image_bytes(&self) -> Option<u64> {
        mb_to_bytes(self.max_image_mb)
    }

    /// Returns a deduplicated order with missing kinds appended.
    pub fn ordered_kinds(&self) -> Vec<CaptureKind> {
        let mut order = Vec::new();
        for kind in self
            .order
            .iter()
            .copied()
            .chain(CaptureKind::default_order())
        {
            if !order.contains(&kind) {
                order.push(kind);
            }
        }

        order
    }

    pub fn is_enabled(&self, kind: CaptureKind) -> bool {
        match kind {
            CaptureKind::Text => self.text,
            CaptureKind::Html => self.html,
            CaptureKind::Rtf => self.rtf,
            CaptureKind::Image => self.image,
            CaptureKind::Files => self.files,
        }
    }
}

fn mb_to_bytes(mb: u32) -> Option<u64> {
    if mb == 0 {
        return None;
    }

    Some(u64::from(mb) * 1024 * 1024)
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CaptureKind {
    Files,
    Image,
    Html,
    Rtf,
    Text,
}

impl CaptureKind {
    /// Preserves the default priority: files, image, HTML, RTF, then plain text.
    pub fn default_order() -> Vec<Self> {
        vec![Self::Files, Self::Image, Self::Html, Self::Rtf, Self::Text]
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct Sensitive {
    /// Whether high-confidence keys and tokens are stored in history.
    pub collect_secrets: bool,

    /// Whether stored sensitive content is redacted in lists and previews.
    pub redact_secrets: bool,
}

impl Default for Sensitive {
    fn default() -> Self {
        Self {
            collect_secrets: true,
            redact_secrets: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct Filters {
    /// Clipboard content from matching source application ids is not stored.
    pub excluded_app_ids: Vec<String>,
}

impl Default for Filters {
    fn default() -> Self {
        Self {
            excluded_app_ids: default_excluded_app_ids(),
        }
    }
}

fn default_excluded_app_ids() -> Vec<String> {
    #[cfg(target_os = "macos")]
    {
        vec![
            "com.apple.keychainaccess".to_owned(),
            "com.apple.Passwords".to_owned(),
        ]
    }
    #[cfg(target_os = "windows")]
    {
        Vec::new()
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Vec::new()
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    #[test]
    fn macos_defaults_keep_sensitive_system_apps_excluded() {
        let ids = default_excluded_app_ids();

        assert!(ids.contains(&"com.apple.keychainaccess".to_owned()));
        assert!(ids.contains(&"com.apple.Passwords".to_owned()));
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct Content {
    /// Action performed when a list item is clicked.
    pub auto_paste: AutoPaste,

    pub middle_click: MiddleClickAction,

    pub copy_plain: bool,

    pub copy_then_hide_window: bool,

    pub paste_plain: bool,

    pub paste_files_as_path: bool,

    pub show_original_preview: bool,

    pub delete_confirm: bool,

    pub delete_favorite_items: bool,

    pub delete_favorite_confirm: bool,

    pub delete_pinned_items: bool,

    pub delete_pinned_confirm: bool,

    pub delete_favorite_items_only_in_favorite_group: bool,
    pub auto_favorite: bool,

    /// Whether copying or pasting from history refreshes use count and `updated_at`.
    pub update_on_reuse: bool,

    pub sort: ClipboardItemSort,

    pub item_actions: Vec<ItemAction>,

    pub item_action_order: Vec<ItemAction>,
}

impl Default for Content {
    fn default() -> Self {
        Self {
            auto_paste: AutoPaste::DoubleClickPaste,
            middle_click: MiddleClickAction::Disabled,
            copy_plain: false,
            copy_then_hide_window: false,
            paste_plain: false,
            paste_files_as_path: false,
            show_original_preview: true,
            delete_confirm: true,
            delete_favorite_items: false,
            delete_favorite_confirm: true,
            delete_pinned_items: false,
            delete_pinned_confirm: true,
            delete_favorite_items_only_in_favorite_group: true,
            auto_favorite: false,
            update_on_reuse: false,
            sort: ClipboardItemSort::UpdatedAt,
            item_actions: vec![
                ItemAction::Copy,
                ItemAction::Star,
                ItemAction::PinItem,
                ItemAction::Delete,
            ],
            item_action_order: vec![
                ItemAction::Paste,
                ItemAction::PastePlain,
                ItemAction::PastePath,
                ItemAction::Copy,
                ItemAction::CopyPlain,
                ItemAction::OpenLink,
                ItemAction::SendEmail,
                ItemAction::Reveal,
                ItemAction::Note,
                ItemAction::Star,
                ItemAction::PinItem,
                ItemAction::Delete,
            ],
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct Display {
    /// Maximum lines shown for text summaries.
    pub text_max_lines: u8,

    /// Image thumbnail height in px.
    pub image_max_height: u16,

    /// Maximum number of file entries returned and displayed.
    pub file_max_count: u8,
}

impl Default for Display {
    fn default() -> Self {
        Self {
            text_max_lines: 3,
            image_max_height: 64,
            file_max_count: 3,
        }
    }
}

impl Display {
    /// Clamps the file-entry limit to the range supported by the UI.
    pub fn file_entry_limit(self) -> usize {
        usize::from(self.file_max_count.clamp(1, 5))
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AutoPaste {
    /// Select only; do not perform an automatic action.
    Disabled,
    SingleClickPaste,
    #[default]
    DoubleClickPaste,
    SingleClickCopy,
    DoubleClickCopy,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MiddleClickAction {
    /// Select only; do not perform an automatic action.
    #[default]
    Disabled,
    SingleClickPaste,
    SingleClickPastePlain,
    SingleClickCopy,
    SingleClickCopyPlain,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ItemAction {
    Paste,
    PastePlain,
    PastePath,
    Copy,
    CopyPlain,
    OpenLink,
    SendEmail,
    Reveal,
    Note,
    Star,
    PinItem,
    Delete,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct Preview {
    pub hover_enabled: bool,
    pub hover_delay_ms: PreviewHoverDelayMs,
    pub space_enabled: bool,
}

impl Default for Preview {
    fn default() -> Self {
        Self {
            hover_enabled: false,
            hover_delay_ms: PreviewHoverDelayMs::Ms500,
            space_enabled: true,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PreviewHoverDelayMs {
    Ms300,
    #[default]
    Ms500,
    Ms1000,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct History {
    pub retention: Retention,

    /// Maximum retained item count. `0` means unlimited.
    pub max_count: u32,

    /// Automatic cleanup interval in hours. `0` disables periodic cleanup.
    pub cleanup_interval_hours: u32,
}

impl Default for History {
    fn default() -> Self {
        Self {
            retention: Retention {
                value: 1,
                unit: RetentionUnit::Months,
            },
            max_count: 0,
            cleanup_interval_hours: 24,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct Retention {
    /// Retention value; ignored when `unit` is `Forever`.
    pub value: u32,
    pub unit: RetentionUnit,
}

impl Default for Retention {
    fn default() -> Self {
        Self {
            value: 0,
            unit: RetentionUnit::Forever,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RetentionUnit {
    Hours,
    Days,
    Weeks,
    Months,
    #[default]
    Forever,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct Search {
    /// Focus the search field whenever the clipboard window is shown.
    pub default_focus: bool,

    /// Clear the search keyword when the clipboard window is hidden.
    pub clear_on_hide: bool,
}

impl Default for Search {
    fn default() -> Self {
        Self {
            default_focus: false,
            clear_on_hide: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct Window {
    pub position: WindowPosition,

    /// Return the history list to the top when opening the clipboard window.
    pub scroll_to_top_on_open: bool,

    pub select_range_on_open: WindowOpenRangeSelection,

    pub select_category_on_open: WindowOpenCategorySelection,

    pub select_group_on_open: String,

    /// Dormant clipboard window and idle destruction for other WebViews when hidden.
    pub lightweight_mode: bool,

    /// Idle seconds before a non-clipboard WebView is destroyed.
    pub idle_destroy_seconds: u32,
}

impl Default for Window {
    fn default() -> Self {
        Self {
            position: WindowPosition::FollowCursor,
            scroll_to_top_on_open: true,
            select_range_on_open: WindowOpenRangeSelection::Preserve,
            select_category_on_open: WindowOpenCategorySelection::Preserve,
            select_group_on_open: WINDOW_OPEN_SELECTION_PRESERVE.to_owned(),
            lightweight_mode: true,
            idle_destroy_seconds: 60,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum WindowOpenRangeSelection {
    #[default]
    Preserve,
    All,
    Favorite,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum WindowOpenCategorySelection {
    #[default]
    Preserve,
    All,
    Text,
    Image,
    Files,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum WindowPosition {
    Remember,
    #[default]
    FollowCursor,
    Center,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct Feedback {
    pub copy_sound: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct WebDav {
    pub enabled: bool,
    pub url: String,
    pub username: String,
    pub password: String,
    pub file_name: String,
}

impl Default for WebDav {
    fn default() -> Self {
        Self {
            enabled: false,
            url: String::new(),
            username: String::new(),
            password: String::new(),
            file_name: "clipboard.ecopastebak".to_owned(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct Update {
    pub auto_check: bool,
    pub frequency: UpdateFrequency,
    pub include_beta: bool,
    pub include_nightly: bool,
    pub last_checked_at: Option<String>,
    pub skipped_version: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum UpdateFrequency {
    #[default]
    Daily,
    Weekly,
    Monthly,
}
