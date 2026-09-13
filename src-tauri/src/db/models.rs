//! Database models and query types exchanged with the clipboard layer.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[sqlx(rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum ClipboardKind {
    Text,
    Image,
    Files,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[sqlx(rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum ClipboardSubKind {
    Rtf,
    Html,
    Url,
    Email,
    Color,
    Path,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[sqlx(rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Macos,
    Windows,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardItem {
    pub id: String,
    pub kind: ClipboardKind,
    pub sub_kind: Option<ClipboardSubKind>,
    pub group_id: Option<String>,

    pub source_app_id: Option<String>,
    pub content: String,

    pub content_hash: String,
    pub search_text: Option<String>,

    pub summary: Option<String>,

    pub file_types: Option<String>,
    pub size: Option<i64>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub use_count: i64,
    pub is_favorite: bool,
    pub is_pinned: bool,

    pub is_sensitive: bool,
    pub platform: Platform,
    pub note: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    #[sqlx(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_app_name: Option<String>,

    #[sqlx(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_app_icon_file: Option<String>,

    #[sqlx(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_app_icon_path: Option<String>,

    #[sqlx(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_thumbnail_path: Option<String>,

    #[sqlx(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_entries: Option<Vec<FileEntry>>,

    #[sqlx(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub files_preview_kind: Option<FilesPreviewKind>,

    #[sqlx(skip)]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub available_actions: Vec<ClipboardAction>,

    #[sqlx(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color_preview: Option<String>,

    #[sqlx(skip)]
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub display_created_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ClipboardAction {
    Paste,

    PasteAsPlainText,

    PasteAsPath,

    Copy,

    SaveImage,

    OpenLink,

    SendEmail,

    RevealInFinder,

    RevealInExplorer,

    ToggleFavorite,

    TogglePinned,

    EditNote,

    Delete,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    pub is_image: bool,

    pub exists: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_path: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FilesPreviewKind {
    ImagePreview,

    List,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardApp {
    pub id: String,
    pub name: String,

    pub icon_file: Option<String>,
    pub platform: Platform,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardGroup {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub is_hidden: bool,
    pub sort_order: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum ClipboardItemSort {
    #[serde(rename = "createdAtDesc")]
    CreatedAt,
    #[default]
    #[serde(rename = "updatedAtDesc")]
    UpdatedAt,
    #[serde(rename = "useCountDesc")]
    UseCount,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ClipboardItemQuery {
    pub kind: Option<ClipboardKind>,
    pub group_id: Option<String>,
    pub favorite: Option<bool>,
    pub pinned: Option<bool>,

    pub group: Option<ClipboardGroupFilter>,
    pub keyword: Option<String>,
    pub sort: ClipboardItemSort,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ClipboardGroupFilter {
    All,
    Text,
    Image,
    Files,
    Favorite,
}

impl Default for ClipboardItemQuery {
    fn default() -> Self {
        Self {
            kind: None,
            group_id: None,
            favorite: None,
            pinned: None,
            group: None,
            keyword: None,
            sort: ClipboardItemSort::UpdatedAt,
            limit: 20,
            offset: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardItemPage {
    pub list: Vec<ClipboardItem>,
    pub total: i64,
    pub has_more: bool,
}
