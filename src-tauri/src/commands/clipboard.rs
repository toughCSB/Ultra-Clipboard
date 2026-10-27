use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use anyhow::anyhow;
use chrono::{Local, Utc};
use serde::Deserialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;

use crate::clipboard::item_view::{ClipboardAppView, ClipboardPreviewPayload, ItemView};
use crate::clipboard::{
    add_app_from_path, delete_unreferenced_apps, emit_clipboard_updated, item_policy, item_view,
    paste, refresh_running_apps, validate_image_file_name, AppIconStore, AppsRegistry,
    ClipboardUpdated, FileIconStore, ImageStore, WritebackGuard,
};
use crate::core::events;
use crate::core::{AppError, Result};
use crate::db::items::{clear_items, find_item_by_id};
use crate::db::models::{
    ClipboardApp, ClipboardGroup, ClipboardItem, ClipboardItemPage, ClipboardItemQuery,
    ClipboardKind,
};
use crate::db::DatabaseState;
use crate::settings::SettingsStore;
use crate::window;

const DEFAULT_CLIPBOARD_GROUP_ICON: &str = "i-lets-icons:folder";
const MAX_GROUP_NAME_CHARS: usize = 32;
const MAX_GROUP_ICON_BYTES: usize = 256 * 1024;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardGroupInput {
    pub name: String,
    pub icon: String,
    #[serde(default)]
    pub is_hidden: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardGroupLayoutInput {
    pub order: Vec<String>,
    pub visible_ids: Vec<String>,
}

#[tauri::command]
pub async fn save_clipboard_image_to_file(
    app: AppHandle,
    db: State<'_, DatabaseState>,
    store: State<'_, ImageStore>,
    id: String,
) -> Result<Option<String>> {
    let pool = db.pool().await;
    let item = find_item_by_id(&pool, &id)
        .await?
        .ok_or_else(|| AppError::Clipboard(format!("clipboard item not found: {id}")))?;

    if item.kind != ClipboardKind::Image {
        return Err(AppError::Clipboard(
            "selected item is not an image".to_owned(),
        ));
    }

    validate_image_file_name(&item.content)?;

    let source = store.origin_path(&item.content);
    if !source.is_file() {
        return Err(AppError::Clipboard("image file does not exist".to_owned()));
    }

    let _auto_hide_guard = ClipboardAutoHideSuspendGuard::new();
    let mut dialog = app
        .dialog()
        .file()
        .add_filter("PNG", &["png"])
        .set_can_create_directories(true)
        .set_file_name(default_saved_image_file_name(&item))
        .set_title(crate::i18n::clipboard_menu::label(
            crate::i18n::current_language(&app),
            crate::i18n::clipboard_menu::Key::SaveImage,
        ));

    match app.path().download_dir() {
        Ok(download_dir) => {
            dialog = dialog.set_directory(download_dir);
        }
        Err(err) => {
            log::warn!("resolve download directory for image save failed: {err}");
        }
    }

    let Some(target_path) = dialog.blocking_save_file() else {
        return Ok(None);
    };

    let target = target_path
        .into_path()
        .map_err(|err| AppError::Clipboard(format!("save path is invalid: {err}")))?;
    let source_for_copy = source.clone();
    let target_for_copy = target.clone();

    tauri::async_runtime::spawn_blocking(move || std::fs::copy(source_for_copy, target_for_copy))
        .await
        .map_err(|err| AppError::Clipboard(format!("save image task join failed: {err}")))?
        .map_err(|err| AppError::Clipboard(format!("save image failed: {err}")))?;

    target
        .to_str()
        .map(str::to_owned)
        .map(Some)
        .ok_or_else(|| AppError::Clipboard("save path is not valid utf-8".to_owned()))
}

#[tauri::command]
pub async fn get_clipboard_preview_payload(
    app: AppHandle,
    db: State<'_, DatabaseState>,
    image_store: State<'_, ImageStore>,
    file_icon_store: State<'_, FileIconStore>,
    item_id: String,
) -> Result<Option<ClipboardPreviewPayload>> {
    let pool = db.pool().await;
    let Some(item) = find_item_by_id(&pool, &item_id).await? else {
        return Ok(None);
    };

    let redact_sensitive = app
        .state::<SettingsStore>()
        .snapshot()
        .clipboard
        .sensitive
        .redact_secrets;
    let view = ItemView {
        pool: &pool,
        images: &image_store,
        file_icons: &file_icon_store,
    };
    let payload = view.preview(item, redact_sensitive).await?;
    Ok(Some(payload))
}

#[tauri::command]
pub async fn play_copy_sound() -> Result<()> {
    tauri::async_runtime::spawn_blocking(crate::clipboard::play_copy_sound_now)
        .await
        .map_err(|e| anyhow!(e.to_string()))?
        .map_err(|e| anyhow!(e))?;
    Ok(())
}

#[tauri::command]
pub async fn write_to_clipboard(
    app: AppHandle,
    db: State<'_, DatabaseState>,
    store: State<'_, ImageStore>,
    guard: State<'_, Arc<WritebackGuard>>,
    id: String,
    plain: bool,
) -> Result<()> {
    let pool = db.pool().await;
    paste::copy_item(&app, &pool, &store, &guard, &id, plain).await
}

#[tauri::command]
pub async fn paste_clipboard_item(
    app: AppHandle,
    db: State<'_, DatabaseState>,
    store: State<'_, ImageStore>,
    guard: State<'_, Arc<WritebackGuard>>,
    id: String,
    plain: bool,
) -> Result<()> {
    let pool = db.pool().await;
    paste::paste_item(&app, &pool, &store, &guard, &id, plain).await
}

#[tauri::command]
pub async fn list_clipboard_items(
    app: AppHandle,
    db: State<'_, DatabaseState>,
    image_store: State<'_, ImageStore>,
    app_icon_store: State<'_, AppIconStore>,
    file_icon_store: State<'_, FileIconStore>,
    query: Option<ClipboardItemQuery>,
) -> Result<ClipboardItemPage> {
    let pool = db.pool().await;
    let q = query.unwrap_or_default();
    let (mut items, total) = crate::db::items::query_items_page(&pool, &q).await?;
    let settings = app.state::<SettingsStore>().snapshot();
    let view = ItemView {
        pool: &pool,
        images: &image_store,
        file_icons: &file_icon_store,
    };
    view.decorate_list(
        &app_icon_store,
        &settings.clipboard,
        q.favorite == Some(true),
        &mut items,
    )
    .await?;
    let has_more = q.offset + (items.len() as i64) < total;
    Ok(ClipboardItemPage {
        list: items,
        total,
        has_more,
    })
}

fn default_saved_image_file_name(item: &ClipboardItem) -> String {
    let local = item.created_at.with_timezone(&Local);

    format!("UltraClipboard-image-{}.png", local.format("%Y%m%d-%H%M%S"))
}

struct ClipboardAutoHideSuspendGuard;

impl ClipboardAutoHideSuspendGuard {
    fn new() -> Self {
        window::set_clipboard_window_auto_hide_suspended(true);

        Self
    }
}

impl Drop for ClipboardAutoHideSuspendGuard {
    fn drop(&mut self) {
        window::set_clipboard_window_auto_hide_suspended(false);
    }
}

#[tauri::command]
pub async fn list_all_apps(
    app_icon_store: State<'_, AppIconStore>,
    db: State<'_, DatabaseState>,
    registry: State<'_, AppsRegistry>,
) -> Result<Vec<ClipboardAppView>> {
    let pool = db.pool().await;
    let running_apps = refresh_running_apps(registry.inner().clone()).await?;
    let known_apps = crate::db::apps::list_all_apps(&pool).await?;
    let mut apps = merge_clipboard_apps(known_apps, running_apps);

    sort_clipboard_apps(&mut apps);
    Ok(apps
        .into_iter()
        .map(|app| item_view::app_view(&app_icon_store, app))
        .collect())
}

#[tauri::command]
pub async fn add_clipboard_app_from_path(
    app_icon_store: State<'_, AppIconStore>,
    registry: State<'_, AppsRegistry>,
    path: String,
) -> Result<ClipboardAppView> {
    let app = add_app_from_path(registry.inner().clone(), path).await?;

    Ok(item_view::app_view(&app_icon_store, app))
}

#[tauri::command]
pub async fn delete_unreferenced_clipboard_apps(
    registry: State<'_, AppsRegistry>,
    ids: Vec<String>,
) -> Result<Vec<String>> {
    delete_unreferenced_apps(registry.inner().clone(), ids).await
}

fn merge_clipboard_apps(
    known_apps: Vec<ClipboardApp>,
    running_apps: Vec<ClipboardApp>,
) -> Vec<ClipboardApp> {
    let mut merged = HashMap::with_capacity(known_apps.len() + running_apps.len());

    for app in running_apps {
        merged.insert(app.id.clone(), app);
    }
    for mut app in known_apps {
        if app.icon_file.is_none() {
            app.icon_file = merged
                .get(&app.id)
                .and_then(|running| running.icon_file.clone());
        }
        merged.insert(app.id.clone(), app);
    }

    merged.into_values().collect()
}

fn sort_clipboard_apps(apps: &mut [ClipboardApp]) {
    apps.sort_by(|left, right| {
        left.name
            .to_lowercase()
            .cmp(&right.name.to_lowercase())
            .then_with(|| left.id.cmp(&right.id))
    });
}

fn normalize_group_name(name: &str) -> Result<String> {
    let normalized = name.trim();
    if normalized.is_empty() {
        return Err(AppError::Clipboard("그룹 이름을 입력하세요".to_owned()));
    }

    if normalized.chars().count() > MAX_GROUP_NAME_CHARS {
        return Err(AppError::Clipboard(
            "그룹 이름은 32자를 넘을 수 없습니다".to_owned(),
        ));
    }

    Ok(normalized.to_owned())
}

fn normalize_group_icon(icon: &str) -> Result<String> {
    let normalized = icon.trim();
    if normalized.is_empty() {
        return Ok(DEFAULT_CLIPBOARD_GROUP_ICON.to_owned());
    }

    if normalized.len() > MAX_GROUP_ICON_BYTES {
        return Err(AppError::Clipboard(
            "SVG 아이콘은 256 KB를 넘을 수 없습니다".to_owned(),
        ));
    }

    if normalized.starts_with("<svg") {
        normalize_group_svg(normalized)?;
    }

    Ok(normalized.to_owned())
}

fn normalize_group_svg(icon: &str) -> Result<()> {
    let normalized = icon.trim_start();
    if !normalized.starts_with("<svg") {
        return Err(AppError::Clipboard(
            "올바른 SVG 아이콘을 선택하세요".to_owned(),
        ));
    }

    let lower = normalized.to_ascii_lowercase();
    if lower.contains("<script") || lower.contains("<foreignobject") {
        return Err(AppError::Clipboard(
            "SVG 아이콘에는 script 또는 foreignObject를 포함할 수 없습니다".to_owned(),
        ));
    }

    Ok(())
}

fn emit_clipboard_groups_updated(app: &AppHandle) {
    if let Err(err) = app.emit(events::CLIPBOARD_GROUPS_UPDATED, ()) {
        log::warn!("emit clipboard groups updated failed: {err}");
    }
}

#[tauri::command]
pub async fn list_clipboard_groups(db: State<'_, DatabaseState>) -> Result<Vec<ClipboardGroup>> {
    let pool = db.pool().await;
    crate::db::groups::list_groups(&pool).await
}

#[tauri::command]
pub async fn create_clipboard_group(
    app: AppHandle,
    db: State<'_, DatabaseState>,
    input: ClipboardGroupInput,
) -> Result<ClipboardGroup> {
    let pool = db.pool().await;
    let name = normalize_group_name(&input.name)?;
    let icon = normalize_group_icon(&input.icon)?;
    let now = Utc::now();
    let group = ClipboardGroup {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        icon,
        is_hidden: input.is_hidden,
        sort_order: crate::db::groups::next_group_sort_order(&pool).await?,
        created_at: now,
        updated_at: now,
    };

    crate::db::groups::insert_group(&pool, &group).await?;
    emit_clipboard_groups_updated(&app);

    Ok(group)
}

#[tauri::command]
pub async fn update_clipboard_group(
    app: AppHandle,
    db: State<'_, DatabaseState>,
    id: String,
    input: ClipboardGroupInput,
) -> Result<()> {
    let pool = db.pool().await;
    let name = normalize_group_name(&input.name)?;
    let icon = normalize_group_icon(&input.icon)?;

    crate::db::groups::update_group(&pool, &id, &name, &icon, input.is_hidden).await?;
    emit_clipboard_groups_updated(&app);

    Ok(())
}

#[tauri::command]
pub async fn update_clipboard_groups_layout(
    app: AppHandle,
    db: State<'_, DatabaseState>,
    input: ClipboardGroupLayoutInput,
) -> Result<()> {
    let pool = db.pool().await;

    crate::db::groups::update_group_layout(&pool, &input.order, &input.visible_ids).await?;
    emit_clipboard_groups_updated(&app);

    Ok(())
}

#[tauri::command]
pub async fn delete_clipboard_group(
    app: AppHandle,
    db: State<'_, DatabaseState>,
    id: String,
) -> Result<()> {
    let pool = db.pool().await;

    crate::db::groups::delete_group(&pool, &id).await?;
    emit_clipboard_groups_updated(&app);

    Ok(())
}

#[tauri::command]
pub async fn import_clipboard_group_svg(path: String) -> Result<String> {
    let path = Path::new(&path);
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if extension != "svg" {
        return Err(AppError::Clipboard("SVG 파일을 선택하세요".to_owned()));
    }

    let bytes = std::fs::read(path)
        .map_err(|err| AppError::Clipboard(format!("SVG 파일을 읽을 수 없습니다: {err}")))?;
    if bytes.len() > MAX_GROUP_ICON_BYTES {
        return Err(AppError::Clipboard(
            "SVG 파일은 256 KB를 넘을 수 없습니다".to_owned(),
        ));
    }

    let icon = String::from_utf8(bytes)
        .map_err(|_| AppError::Clipboard("SVG 파일은 UTF-8 텍스트여야 합니다".to_owned()))?;
    normalize_group_svg(&icon)?;

    Ok(icon)
}

#[tauri::command]
pub async fn toggle_clipboard_item_favorite(
    db: State<'_, DatabaseState>,
    id: String,
) -> Result<bool> {
    let pool = db.pool().await;
    crate::db::items::toggle_item_favorite(&pool, &id).await
}

#[tauri::command]
pub async fn toggle_clipboard_item_pinned(
    db: State<'_, DatabaseState>,
    id: String,
) -> Result<bool> {
    let pool = db.pool().await;
    crate::db::items::toggle_item_pinned(&pool, &id).await
}

#[tauri::command]
pub async fn delete_clipboard_item(
    app: AppHandle,
    db: State<'_, DatabaseState>,
    store: State<'_, ImageStore>,
    id: String,
    in_favorite_view: bool,
) -> Result<()> {
    let pool = db.pool().await;
    if let Some(item) = find_item_by_id(&pool, &id).await? {
        let content = app.state::<SettingsStore>().snapshot().clipboard.content;
        if !item_policy::can_delete(&item, &content, in_favorite_view) {
            return Err(AppError::Clipboard(
                "this item is protected by the delete settings".to_owned(),
            ));
        }
    }

    if let Some(file_name) = crate::db::items::delete_item(&pool, &id).await? {
        if let Err(err) = store.remove(&file_name) {
            log::warn!("remove deleted image {file_name} failed: {err}");
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn clear_clipboard_items(
    app: AppHandle,
    db: State<'_, DatabaseState>,
    store: State<'_, ImageStore>,
    delete_favorites: bool,
    delete_pinned: bool,
) -> Result<u64> {
    let pool = db.pool().await;
    let outcome = clear_items(&pool, delete_favorites, delete_pinned).await?;

    for file_name in &outcome.image_files {
        if let Err(err) = store.remove(file_name) {
            log::warn!("remove cleared image {file_name} failed: {err}");
        }
    }

    emit_clipboard_updated(
        &app,
        ClipboardUpdated::Cleanup {
            removed: outcome.removed,
        },
    );

    Ok(outcome.removed)
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateNoteResult {
    pub note: Option<String>,
    pub auto_favorited: bool,
}

#[tauri::command]
pub async fn update_clipboard_item_note(
    app: AppHandle,
    db: State<'_, DatabaseState>,
    id: String,
    note: Option<String>,
) -> Result<UpdateNoteResult> {
    let pool = db.pool().await;
    let normalized = note.as_deref().and_then(|s| {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    });
    crate::db::items::update_item_note(&pool, &id, normalized).await?;

    let mut auto_favorited = false;
    if normalized.is_some() {
        let auto_favorite = app
            .try_state::<crate::settings::SettingsStore>()
            .map(|s| s.snapshot().clipboard.content.auto_favorite)
            .unwrap_or(false);
        if auto_favorite {
            crate::db::items::mark_item_favorite(&pool, &id).await?;
            auto_favorited = true;
        }
    }
    Ok(UpdateNoteResult {
        note: normalized.map(str::to_owned),
        auto_favorited,
    })
}

#[tauri::command]
pub async fn update_clipboard_item_group(
    db: State<'_, DatabaseState>,
    id: String,
    group_id: String,
) -> Result<()> {
    let pool = db.pool().await;
    let exists = crate::db::groups::list_groups(&pool)
        .await?
        .iter()
        .any(|group| group.id == group_id);
    if !exists {
        return Err(AppError::Clipboard("그룹이 존재하지 않습니다".to_owned()));
    }

    crate::db::items::update_item_group(&pool, &id, Some(&group_id)).await
}

#[tauri::command]
pub async fn open_clipboard_item_link(
    app: AppHandle,
    db: State<'_, DatabaseState>,
    id: String,
    mailto: bool,
) -> Result<()> {
    use tauri_plugin_opener::OpenerExt;

    let pool = db.pool().await;
    let item = find_item_by_id(&pool, &id)
        .await?
        .ok_or_else(|| AppError::Clipboard(format!("clipboard item not found: {id}")))?;

    let value = item.content.trim();
    if value.is_empty() {
        return Ok(());
    }

    let url = if mailto {
        format!("mailto:{value}")
    } else if value.starts_with("www.") {
        format!("https://{value}")
    } else {
        value.to_owned()
    };

    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|err| AppError::Clipboard(err.to_string()))
}

#[tauri::command]
pub async fn reveal_clipboard_item(
    app: AppHandle,
    db: State<'_, DatabaseState>,
    id: String,
) -> Result<()> {
    use tauri_plugin_opener::OpenerExt;

    let pool = db.pool().await;
    let item = find_item_by_id(&pool, &id)
        .await?
        .ok_or_else(|| AppError::Clipboard(format!("clipboard item not found: {id}")))?;

    let target = if item.kind == ClipboardKind::Files {
        item.content
            .split('\n')
            .find(|s| !s.is_empty())
            .unwrap_or("")
            .to_owned()
    } else {
        item.content.trim().to_owned()
    };

    if target.is_empty() {
        return Ok(());
    }

    app.opener()
        .reveal_item_in_dir(&target)
        .map_err(|err| AppError::Clipboard(err.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::items::content_hash;
    use crate::db::models::Platform;
    use chrono::TimeZone;

    fn image_item() -> ClipboardItem {
        let content = "abcdef0123.png".to_owned();

        ClipboardItem {
            id: "item".to_owned(),
            kind: ClipboardKind::Image,
            sub_kind: None,
            group_id: None,
            source_app_id: None,
            content_hash: content_hash(ClipboardKind::Image, &content),
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
            is_sensitive: false,
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

    #[test]
    fn saved_image_file_name_uses_current_product_name() {
        let mut item = image_item();
        item.created_at = Utc.with_ymd_and_hms(2026, 9, 12, 12, 34, 56).unwrap();
        let stamp = item
            .created_at
            .with_timezone(&Local)
            .format("%Y%m%d-%H%M%S");

        assert_eq!(
            default_saved_image_file_name(&item),
            format!("UltraClipboard-image-{stamp}.png")
        );
    }
}
