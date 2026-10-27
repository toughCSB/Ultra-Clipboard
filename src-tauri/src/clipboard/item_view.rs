//! What the clipboard window shows for a stored item: list decoration, the
//! preview payload, and the source-app view.

use std::path::Path;

use chrono::{DateTime, Datelike, Local, Utc};
use serde::Serialize;
use sqlx::SqlitePool;

use crate::clipboard::{
    item_policy, sanitize_css_color, validate_image_file_name, AppIconStore, FileIconStore,
    ImageStore,
};
use crate::core::{AppError, Result};
use crate::db::models::{
    ClipboardApp, ClipboardItem, ClipboardKind, ClipboardSubKind, FileEntry, FilesPreviewKind,
    Platform,
};
use crate::settings::Clipboard;

const PREVIEW_FILE_ENTRY_LIMIT: usize = 64;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardAppView {
    pub id: String,
    pub name: String,
    pub icon_file: Option<String>,
    pub icon_path: Option<String>,
    pub platform: Platform,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardPreviewPayload {
    pub id: String,
    pub kind: ClipboardKind,
    pub sub_kind: Option<ClipboardSubKind>,
    pub updated_at: DateTime<Utc>,

    pub text: Option<String>,
    pub image_path: Option<String>,
    pub image_width: Option<i64>,
    pub image_height: Option<i64>,
    pub size: Option<i64>,
    pub is_sensitive: bool,
    pub image_exists: bool,
    pub files: Vec<ClipboardPreviewFileEntry>,
    pub total_files: usize,
}

#[derive(Debug, Serialize)]
pub struct ClipboardPreviewFileEntry {
    #[serde(flatten)]
    pub entry: FileEntry,
    pub size: Option<i64>,
}

pub fn app_view(store: &AppIconStore, app: ClipboardApp) -> ClipboardAppView {
    let icon_path = app
        .icon_file
        .as_deref()
        .and_then(|name| store.icon_path(name).to_str().map(str::to_owned));

    ClipboardAppView {
        id: app.id,
        name: app.name,
        icon_file: app.icon_file,
        icon_path,
        platform: app.platform,
        created_at: app.created_at,
        updated_at: app.updated_at,
    }
}

/// The stores an item view reads files, images, and cached icons from.
pub struct ItemView<'a> {
    pub pool: &'a SqlitePool,
    pub images: &'a ImageStore,
    pub file_icons: &'a FileIconStore,
}

impl ItemView<'_> {
    pub async fn decorate_list(
        &self,
        app_icons: &AppIconStore,
        settings: &Clipboard,
        in_favorite_view: bool,
        items: &mut [ClipboardItem],
    ) -> Result<()> {
        let now = Local::now();
        let file_entry_limit = settings.display.file_entry_limit();
        let redact_sensitive = settings.sensitive.redact_secrets;

        for item in items {
            self.attach_image_thumbnail_path(item);
            attach_source_app_icon_path(app_icons, item);
            self.attach_file_entries(item, file_entry_limit).await?;
            attach_color_preview(item);
            attach_display_created_at(item, &now);
            redact_sensitive_list_item(item, redact_sensitive);
            item.available_actions =
                item_policy::available_actions(item, &settings.content, in_favorite_view);
        }

        Ok(())
    }

    pub async fn preview(
        &self,
        item: ClipboardItem,
        redact_sensitive: bool,
    ) -> Result<ClipboardPreviewPayload> {
        let mut text = None;
        let mut image_path = None;
        let mut image_exists = false;
        let mut files = Vec::new();
        let mut total_files = 0;

        match item.kind {
            ClipboardKind::Text => {
                text = Some(preview_text(&item, redact_sensitive));
            }
            ClipboardKind::Image => {
                validate_image_file_name(&item.content)?;
                let path = self.images.origin_path(&item.content);
                image_exists = path.exists();
                image_path = Some(path_to_string(&path, "image path")?);
            }
            ClipboardKind::Files => {
                total_files = stored_paths(&item.content).count();
                files = self.preview_file_entries(&item).await?;
            }
        }
        let preview_sub_kind = preview_sub_kind(&item, redact_sensitive);

        Ok(ClipboardPreviewPayload {
            id: item.id,
            kind: item.kind,
            sub_kind: preview_sub_kind,
            updated_at: item.updated_at,
            text,
            image_path,
            image_width: item.width,
            image_height: item.height,
            size: item.size,
            is_sensitive: item.is_sensitive,
            image_exists,
            files,
            total_files,
        })
    }

    fn attach_image_thumbnail_path(&self, item: &mut ClipboardItem) {
        if item.kind != ClipboardKind::Image {
            return;
        }

        if validate_image_file_name(&item.content).is_err() {
            item.image_thumbnail_path = None;
            return;
        }

        let file_name = item.content.clone();
        let thumb_path = self.images.thumbnail_path(&file_name);
        let thumb_exists = thumb_path.exists();

        let immediate_path = if thumb_exists {
            thumb_path
        } else {
            self.images.origin_path(&file_name)
        };

        item.image_thumbnail_path = immediate_path.to_str().map(str::to_owned);

        if !thumb_exists {
            let store = self.images.clone();
            tauri::async_runtime::spawn_blocking(move || {
                if let Err(err) = store.ensure_thumbnail(&file_name) {
                    log::warn!("ensure image thumbnail failed for {:?}: {err}", file_name);
                }
            });
        }
    }

    async fn attach_file_entries(&self, item: &mut ClipboardItem, limit: usize) -> Result<()> {
        if item.kind != ClipboardKind::Files {
            return Ok(());
        }

        let entries = self.list_file_entries(item, limit).await?;
        if entries.is_empty() {
            return Ok(());
        }

        item.files_preview_kind = Some(match entries.as_slice() {
            [only] if only.is_image && only.exists => FilesPreviewKind::ImagePreview,
            _ => FilesPreviewKind::List,
        });
        item.file_entries = Some(entries);
        Ok(())
    }

    /// The list trusts the recorded file type for `is_dir` to avoid a stat per row.
    async fn list_file_entries(
        &self,
        item: &ClipboardItem,
        limit: usize,
    ) -> Result<Vec<FileEntry>> {
        let file_types = item.file_types.as_deref();
        let mut entries = Vec::new();

        for (index, path) in stored_paths(&item.content).take(limit).enumerate() {
            let is_dir = recorded_as_dir(file_types, index);
            entries.push(self.file_entry(path, file_types, index, is_dir).await?);
        }

        Ok(entries)
    }

    /// The preview asks the file system first and falls back to the recorded type.
    async fn preview_file_entries(
        &self,
        item: &ClipboardItem,
    ) -> Result<Vec<ClipboardPreviewFileEntry>> {
        let file_types = item.file_types.as_deref();
        let mut entries = Vec::new();

        for (index, path) in stored_paths(&item.content)
            .take(PREVIEW_FILE_ENTRY_LIMIT)
            .enumerate()
        {
            let metadata = Path::new(path).metadata().ok();
            let is_dir = metadata
                .as_ref()
                .map_or_else(|| recorded_as_dir(file_types, index), |meta| meta.is_dir());
            let size = metadata.filter(|_| !is_dir).map(|meta| meta.len() as i64);

            entries.push(ClipboardPreviewFileEntry {
                entry: self.file_entry(path, file_types, index, is_dir).await?,
                size,
            });
        }

        Ok(entries)
    }

    async fn file_entry(
        &self,
        path: &str,
        file_types: Option<&str>,
        index: usize,
        is_dir: bool,
    ) -> Result<FileEntry> {
        let (icon_path, exists) = self.file_icon_path(path, file_types, index).await?;
        let name = Path::new(path)
            .file_name()
            .and_then(|name| name.to_str())
            .map_or_else(|| path.to_owned(), str::to_owned);

        Ok(FileEntry {
            path: path.to_owned(),
            name,
            is_dir,
            is_image: !is_dir && is_image_path(path),
            exists,
            icon_path,
        })
    }

    async fn file_icon_path(
        &self,
        path: &str,
        file_types: Option<&str>,
        index: usize,
    ) -> Result<(Option<String>, bool)> {
        let path_obj = Path::new(path);
        let exists = path_obj.exists();
        let platform = if cfg!(target_os = "macos") {
            Platform::Macos
        } else {
            Platform::Windows
        };

        let cache_key = if recorded_as_dir(file_types, index) {
            crate::clipboard::DIR_CACHE_KEY.to_string()
        } else {
            crate::clipboard::get_icon_cache_key(path_obj)
        };

        if let Some(icon_file) =
            crate::db::file_icons::get_icon(self.pool, &cache_key, platform).await?
        {
            let icon_path = self.file_icons.icon_path(&icon_file);
            if icon_path.exists() {
                return Ok((icon_path.to_str().map(str::to_owned), exists));
            }
        }

        if !exists {
            return Ok((None, false));
        }

        let path_for_extract = path_obj.to_path_buf();
        let png_bytes = tauri::async_runtime::spawn_blocking(move || {
            crate::clipboard::icon_png(&path_for_extract, None)
        })
        .await
        .map_err(|err| AppError::Clipboard(format!("icon extract task join failed: {err}")))?;

        let Some(png) = png_bytes else {
            return Ok((None, exists));
        };

        let icon_file = self.file_icons.store(&png)?;
        crate::db::file_icons::upsert_icon(self.pool, &cache_key, platform, &icon_file).await?;

        let icon_path = self.file_icons.icon_path(&icon_file);
        Ok((icon_path.to_str().map(str::to_owned), exists))
    }
}

/// Paths of a files item, one per non-empty line.
fn stored_paths(content: &str) -> impl Iterator<Item = &str> {
    content.split('\n').filter(|path| !path.is_empty())
}

/// `file_types` is a comma list parallel to the paths, with `d` for a directory.
fn recorded_as_dir(file_types: Option<&str>, index: usize) -> bool {
    file_types.and_then(|types| types.split(',').nth(index)) == Some("d")
}

fn attach_source_app_icon_path(store: &AppIconStore, item: &mut ClipboardItem) {
    item.source_app_icon_path = item
        .source_app_icon_file
        .as_deref()
        .and_then(|name| store.icon_path(name).to_str().map(str::to_owned));
}

fn attach_color_preview(item: &mut ClipboardItem) {
    if item.sub_kind != Some(ClipboardSubKind::Color) {
        return;
    }

    let source = item.summary.as_deref().unwrap_or(&item.content);
    item.color_preview = sanitize_css_color(source);
}

fn attach_display_created_at(item: &mut ClipboardItem, now: &DateTime<Local>) {
    let local = item.created_at.with_timezone(&Local);
    let today = now.date_naive() == local.date_naive();
    let same_year = now.year() == local.year();

    item.display_created_at = if today {
        local.format("%H:%M").to_string()
    } else if same_year {
        local.format("%m-%d %H:%M").to_string()
    } else {
        local.format("%Y-%m-%d %H:%M").to_string()
    };
}

fn redact_sensitive_list_item(item: &mut ClipboardItem, redact_sensitive: bool) {
    if !redact_sensitive || !item.is_sensitive || item.kind != ClipboardKind::Text {
        return;
    }

    if let Some(summary) = item.summary.as_mut() {
        *summary = mask_sensitive_text(summary);
    }
    item.color_preview = None;
}

fn mask_sensitive_text(text: &str) -> String {
    text.split('\n')
        .map(mask_sensitive_line)
        .collect::<Vec<_>>()
        .join("\n")
}

fn mask_sensitive_line(line: &str) -> String {
    const VISIBLE_EDGE_CHARS: usize = 4;
    const MAX_MASK_CHARS: usize = 8;

    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    if len == 0 {
        return String::new();
    }
    if len <= VISIBLE_EDGE_CHARS * 2 {
        return "*".repeat(len);
    }

    let head = chars[..VISIBLE_EDGE_CHARS].iter().collect::<String>();
    let tail = chars[len - VISIBLE_EDGE_CHARS..].iter().collect::<String>();
    let mask_len = (len - VISIBLE_EDGE_CHARS * 2).min(MAX_MASK_CHARS);

    [head, "*".repeat(mask_len), tail].concat()
}

fn preview_sub_kind(item: &ClipboardItem, redact_sensitive: bool) -> Option<ClipboardSubKind> {
    if redact_sensitive && item.is_sensitive && item.kind == ClipboardKind::Text {
        return None;
    }

    item.sub_kind
}

fn preview_text(item: &ClipboardItem, redact_sensitive: bool) -> String {
    let source = match item.sub_kind {
        Some(ClipboardSubKind::Html | ClipboardSubKind::Rtf) => {
            item.search_text.as_deref().unwrap_or(&item.content)
        }
        _ => &item.content,
    };

    if redact_sensitive && item.is_sensitive {
        return mask_sensitive_text(source);
    }

    source.to_owned()
}

fn path_to_string(path: &Path, label: &str) -> Result<String> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| AppError::Clipboard(format!("{label} is not valid utf-8")))
}

fn is_image_path(path: &str) -> bool {
    let Some(ext) = Path::new(path).extension().and_then(|e| e.to_str()) else {
        return false;
    };

    matches!(
        ext.to_ascii_lowercase().as_str(),
        "jpg"
            | "jpeg"
            | "png"
            | "webp"
            | "avif"
            | "gif"
            | "svg"
            | "bmp"
            | "ico"
            | "tif"
            | "tiff"
            | "heic"
            | "apng"
    )
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::db::items::content_hash;
    use crate::db::test_support::memory_pool;

    fn text_item(sub_kind: Option<ClipboardSubKind>, is_sensitive: bool) -> ClipboardItem {
        let content = "<b>secret</b>".to_owned();

        ClipboardItem {
            id: "item".to_owned(),
            kind: ClipboardKind::Text,
            sub_kind,
            group_id: None,
            source_app_id: None,
            content_hash: content_hash(ClipboardKind::Text, &content),
            content,
            search_text: None,
            summary: None,
            file_types: None,
            size: None,
            width: None,
            height: None,
            use_count: 1,
            is_favorite: false,
            is_pinned: false,
            is_sensitive,
            platform: Platform::Macos,
            note: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            source_app_name: None,
            source_app_icon_file: None,
            source_app_icon_path: None,
            image_thumbnail_path: None,
            file_entries: None,
            files_preview_kind: None,
            available_actions: Vec::new(),
            color_preview: None,
            display_created_at: String::new(),
        }
    }

    fn files_item(paths: &[&str], file_types: &str) -> ClipboardItem {
        let content = paths.join("\n");
        let mut item = text_item(None, false);

        item.kind = ClipboardKind::Files;
        item.content_hash = content_hash(ClipboardKind::Files, &content);
        item.content = content;
        item.file_types = Some(file_types.to_owned());
        item
    }

    #[tokio::test]
    async fn list_and_preview_build_the_same_entry_for_missing_files() {
        let dir = tempfile::tempdir().unwrap();
        let pool = memory_pool().await;
        let images = ImageStore::for_test(dir.path().join("images"));
        let file_icons = FileIconStore::for_test(dir.path().join("file-icons"));
        let view = ItemView {
            pool: &pool,
            images: &images,
            file_icons: &file_icons,
        };
        let missing = dir.path().join("gone");
        let dir_path = missing.join("folder").to_string_lossy().into_owned();
        let png_path = missing.join("shot.PNG").to_string_lossy().into_owned();
        let mut item = files_item(&[&dir_path, "", &png_path], "d,f");

        view.attach_file_entries(&mut item, 10).await.unwrap();
        let payload = view.preview(item.clone(), false).await.unwrap();

        let list = serde_json::to_value(item.file_entries.unwrap()).unwrap();
        let preview: Vec<_> = payload
            .files
            .iter()
            .map(|file| serde_json::to_value(&file.entry).unwrap())
            .collect();
        assert_eq!(list, json!(preview));
        assert_eq!(
            list,
            json!([
                { "path": dir_path, "name": "folder", "isDir": true, "isImage": false, "exists": false },
                { "path": png_path, "name": "shot.PNG", "isDir": false, "isImage": true, "exists": false },
            ])
        );
        assert_eq!(item.files_preview_kind, Some(FilesPreviewKind::List));
        assert_eq!(payload.total_files, 2);
        assert!(payload.files.iter().all(|file| file.size.is_none()));
    }

    #[test]
    fn preview_file_entry_serializes_flat_with_size() {
        let entry = ClipboardPreviewFileEntry {
            entry: FileEntry {
                path: "/a/b.txt".to_owned(),
                name: "b.txt".to_owned(),
                is_dir: false,
                is_image: false,
                exists: true,
                icon_path: Some("/icons/x.png".to_owned()),
            },
            size: Some(12),
        };

        assert_eq!(
            serde_json::to_value(entry).unwrap(),
            json!({
                "path": "/a/b.txt",
                "name": "b.txt",
                "isDir": false,
                "isImage": false,
                "exists": true,
                "iconPath": "/icons/x.png",
                "size": 12,
            })
        );
    }

    #[test]
    fn mask_sensitive_text_replaces_middle_with_stars() {
        assert_eq!(
            mask_sensitive_text("sk-abcdefghijklmnopqrstuvwxyzABCDE1234567890"),
            "sk-a********7890"
        );
    }

    #[test]
    fn mask_sensitive_text_replaces_short_lines_fully() {
        assert_eq!(mask_sensitive_text("secret"), "******");
    }

    #[test]
    fn mask_sensitive_text_masks_each_line() {
        assert_eq!(
            mask_sensitive_text("abcd1234xyz\nshort"),
            "abcd***4xyz\n*****"
        );
    }

    #[test]
    fn redact_sensitive_list_item_masks_summary_when_enabled() {
        let mut item = text_item(None, true);
        item.summary = Some("sk-abcdefghijklmnopqrstuvwxyzABCDE1234567890".to_owned());

        redact_sensitive_list_item(&mut item, true);

        assert_eq!(item.summary.as_deref(), Some("sk-a********7890"));
    }

    #[test]
    fn redact_sensitive_list_item_keeps_summary_when_disabled() {
        let mut item = text_item(None, true);
        item.summary = Some("sk-abcdefghijklmnopqrstuvwxyzABCDE1234567890".to_owned());

        redact_sensitive_list_item(&mut item, false);

        assert_eq!(
            item.summary.as_deref(),
            Some("sk-abcdefghijklmnopqrstuvwxyzABCDE1234567890")
        );
    }

    #[test]
    fn preview_sub_kind_drops_sensitive_text_subtype_when_redacted() {
        let item = text_item(Some(ClipboardSubKind::Html), true);

        assert_eq!(preview_sub_kind(&item, true), None);
    }

    #[test]
    fn preview_sub_kind_keeps_sensitive_text_subtype_when_not_redacted() {
        let item = text_item(Some(ClipboardSubKind::Html), true);

        assert_eq!(preview_sub_kind(&item, false), Some(ClipboardSubKind::Html));
    }

    #[test]
    fn preview_sub_kind_keeps_regular_text_subtype() {
        let item = text_item(Some(ClipboardSubKind::Html), false);

        assert_eq!(preview_sub_kind(&item, true), Some(ClipboardSubKind::Html));
    }

    #[test]
    fn preview_text_keeps_plain_text_content() {
        let mut item = text_item(None, false);
        item.content = "plain text".to_owned();

        assert_eq!(preview_text(&item, false), "plain text");
    }

    #[test]
    fn preview_text_uses_search_text_for_html() {
        let mut item = text_item(Some(ClipboardSubKind::Html), false);
        item.content = "<b>Hello</b> World".to_owned();
        item.search_text = Some("Hello World".to_owned());

        assert_eq!(preview_text(&item, false), "Hello World");
    }

    #[test]
    fn preview_text_uses_search_text_for_rtf() {
        let mut item = text_item(Some(ClipboardSubKind::Rtf), false);
        item.content = r"{\rtf1 Hello World}".to_owned();
        item.search_text = Some("Hello World".to_owned());

        assert_eq!(preview_text(&item, false), "Hello World");
    }

    #[test]
    fn preview_text_masks_sensitive_plain_source() {
        let mut item = text_item(Some(ClipboardSubKind::Html), true);
        item.content = "<b>sk-abcdefghijklmnopqrstuvwxyzABCDE1234567890</b>".to_owned();
        item.search_text = Some("sk-abcdefghijklmnopqrstuvwxyzABCDE1234567890".to_owned());

        assert_eq!(preview_text(&item, true), "sk-a********7890");
    }
}
