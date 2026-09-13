use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use anyhow::anyhow;
use chrono::{DateTime, Datelike, Local, Utc};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;

use crate::clipboard::{
    add_app_from_path, build_item_with_settings, delete_unreferenced_apps, detect_frontmost,
    materialize_source, persist_and_notify, refresh_running_apps, sanitize_css_color, AppIconStore,
    AppsRegistry, ClipboardReader, FileIconStore, ImageStore, WritebackGuard,
};
use crate::core::{AppError, Result};
use crate::db::items::{
    clear_items, find_item_by_id, find_item_for_list_by_id, increment_item_use_count,
};
use crate::db::models::{
    ClipboardAction, ClipboardApp, ClipboardGroup, ClipboardItem, ClipboardItemPage,
    ClipboardItemQuery, ClipboardKind, ClipboardSubKind, FileEntry, Platform,
};
use crate::db::DatabaseState;
use crate::settings::SettingsStore;
use crate::window::{self, CLIPBOARD_WINDOW_LABEL};

const CLIPBOARD_UPDATED_EVENT: &str = "clipboard://updated";

const CLIPBOARD_GROUPS_UPDATED_EVENT: &str = "clipboard-groups://updated";

const DEFAULT_CLIPBOARD_GROUP_ICON: &str = "i-lets-icons:folder";
const MAX_GROUP_NAME_CHARS: usize = 32;
const MAX_GROUP_ICON_BYTES: usize = 256 * 1024;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadClipboardResult {
    pub item: ClipboardItem,
    pub deduplicated: bool,

    pub captured: bool,
}

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
pub async fn read_clipboard(
    app: AppHandle,
    db: State<'_, DatabaseState>,
    store: State<'_, ImageStore>,
    app_icon_store: State<'_, AppIconStore>,
    registry: State<'_, AppsRegistry>,
) -> Result<Option<ReadClipboardResult>> {
    let (item_opt, source) = {
        let source = detect_frontmost();
        let reader = ClipboardReader::new()?;
        let settings = app.state::<SettingsStore>().snapshot();
        let payload = reader.read_with_capture(&settings.clipboard.capture)?;
        let item = match payload {
            Some(payload) => build_item_with_settings(
                &store,
                &payload,
                &settings.clipboard.capture,
                &settings.clipboard.sensitive,
                settings.clipboard.content.copy_plain,
            )?,
            None => None,
        };
        (item, source)
    };

    let Some(mut item) = item_opt else {
        return Ok(None);
    };

    let source_app = source.map(|src| materialize_source(&app_icon_store, Some(&registry), src));
    if let Some(src) = &source_app {
        item.source_app_id = Some(src.id.clone());
    }

    let pool = db.pool().await;
    let result = persist_and_notify(&app, &pool, &item, source_app.as_ref()).await?;
    Ok(Some(ReadClipboardResult {
        item,
        deduplicated: result.deduplicated,
        captured: true,
    }))
}

#[tauri::command]
pub async fn get_clipboard_image_path(
    store: State<'_, ImageStore>,
    file_name: String,
    thumbnail: bool,
) -> Result<String> {
    validate_image_file_name(&file_name)?;

    let path = if thumbnail {
        let store = store.inner().clone();
        tauri::async_runtime::spawn_blocking(move || store.ensure_thumbnail(&file_name))
            .await
            .map_err(|err| AppError::Clipboard(format!("thumbnail task join failed: {err}")))??
    } else {
        store.origin_path(&file_name)
    };

    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| AppError::Clipboard("image path is not valid utf-8".to_owned()))
}

#[tauri::command]
pub async fn get_clipboard_app_icon_path(
    store: State<'_, AppIconStore>,
    file_name: String,
) -> Result<String> {
    validate_image_file_name(&file_name)?;
    store
        .icon_path(&file_name)
        .to_str()
        .map(str::to_owned)
        .ok_or_else(|| AppError::Clipboard("app icon path is not valid utf-8".to_owned()))
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileIconResult {
    pub icon_path: Option<String>,

    pub exists: bool,
}

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

const PREVIEW_FILE_ENTRY_LIMIT: usize = 64;

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
#[serde(rename_all = "camelCase")]
pub struct ClipboardPreviewFileEntry {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    pub is_image: bool,
    pub exists: bool,
    pub size: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_path: Option<String>,
}

#[tauri::command]
pub async fn get_file_icon_path(
    db: State<'_, DatabaseState>,
    file_icon_store: State<'_, FileIconStore>,
    path: String,
    file_types: Option<String>,
    index: usize,
) -> Result<FileIconResult> {
    let pool = db.pool().await;
    let (icon_path, exists) =
        resolve_file_icon_path(&pool, &file_icon_store, &path, file_types.as_deref(), index)
            .await?;

    Ok(FileIconResult { icon_path, exists })
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
    let payload = build_clipboard_preview_payload(
        &pool,
        &image_store,
        &file_icon_store,
        item,
        redact_sensitive,
    )
    .await?;
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
    let item = find_item_by_id(&pool, &id)
        .await?
        .ok_or_else(|| AppError::Clipboard(format!("clipboard item not found: {id}")))?;

    let settings = app.state::<SettingsStore>().snapshot();
    let write_plain =
        should_write_plain_for_copy(plain, item.kind, settings.clipboard.content.copy_plain);
    let hide_after_copy = settings.clipboard.content.copy_then_hide_window;

    crate::clipboard::write_to_clipboard(&store, guard.inner().as_ref(), &item, write_plain)?;
    mark_item_reused_if_enabled(&app, &pool, &id, item.kind).await?;

    if hide_after_copy {
        hide_clipboard_window_after_copy(&app);
    }

    Ok(())
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
    let item = find_item_by_id(&pool, &id)
        .await?
        .ok_or_else(|| AppError::Clipboard(format!("clipboard item not found: {id}")))?;

    let settings = app.state::<SettingsStore>().snapshot();
    let write_plain = should_write_plain_for_paste(
        plain,
        item.kind,
        settings.clipboard.content.paste_plain,
        settings.clipboard.content.paste_files_as_path,
    );

    crate::clipboard::write_to_clipboard(&store, guard.inner().as_ref(), &item, write_plain)?;
    mark_item_reused_if_enabled(&app, &pool, &id, item.kind).await?;

    if window::is_clipboard_window_pinned() {
        #[cfg(target_os = "macos")]
        if let Err(err) = window::macos::resign_clipboard_panel_key(&app) {
            log::warn!("resign clipboard panel key before paste failed: {err:?}");
        }
    } else if let Err(err) = window::hide_window(&app, CLIPBOARD_WINDOW_LABEL) {
        log::warn!("hide clipboard window before paste failed: {err:?}");
    }

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    crate::keystroke::simulate_paste()?;

    if window::is_clipboard_window_pinned() {
        #[cfg(target_os = "macos")]
        {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            if let Err(err) = window::macos::make_clipboard_panel_key(&app) {
                log::warn!("restore clipboard panel key after paste failed: {err:?}");
            }
        }
    }

    Ok(())
}

fn should_write_plain_for_copy(force_plain: bool, kind: ClipboardKind, copy_plain: bool) -> bool {
    force_plain || kind == ClipboardKind::Text && copy_plain
}

fn should_write_plain_for_paste(
    force_plain: bool,
    kind: ClipboardKind,
    paste_plain: bool,
    paste_files_as_path: bool,
) -> bool {
    force_plain
        || kind == ClipboardKind::Text && paste_plain
        || kind == ClipboardKind::Files && paste_files_as_path
}

async fn mark_item_reused_if_enabled(
    app: &AppHandle,
    pool: &SqlitePool,
    id: &str,
    kind: ClipboardKind,
) -> Result<()> {
    let settings = app.state::<SettingsStore>().snapshot();
    if !settings.clipboard.content.update_on_reuse {
        return Ok(());
    }

    increment_item_use_count(pool, id).await?;
    if let Err(err) = app.emit(
        CLIPBOARD_UPDATED_EVENT,
        serde_json::json!({
            "id": id,
            "kind": kind,
            "deduplicated": true,
        }),
    ) {
        log::warn!("emit {CLIPBOARD_UPDATED_EVENT} after item reuse failed: {err}");
    }

    Ok(())
}

fn hide_clipboard_window_after_copy(app: &AppHandle) {
    if window::is_clipboard_window_pinned() {
        return;
    }

    if let Err(err) = window::hide_window(app, CLIPBOARD_WINDOW_LABEL) {
        log::warn!("hide clipboard window after copy failed: {err:?}");
    }
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
    let now = Local::now();
    let settings = app.state::<SettingsStore>().snapshot();
    let file_entry_limit = settings.clipboard.display.file_entry_limit();
    let redact_sensitive = settings.clipboard.sensitive.redact_secrets;
    for item in &mut items {
        attach_image_thumbnail_path(&image_store, item).await?;
        attach_source_app_icon_path(&app_icon_store, item);
        attach_file_entries(&pool, &file_icon_store, item, file_entry_limit).await?;
        attach_color_preview(item);
        attach_display_created_at(item, &now);
        redact_sensitive_list_item(item, redact_sensitive);
        item.available_actions = compute_available_actions(item);
    }
    let has_more = q.offset + (items.len() as i64) < total;
    Ok(ClipboardItemPage {
        list: items,
        total,
        has_more,
    })
}

#[tauri::command]
pub async fn get_clipboard_item(
    app: AppHandle,
    db: State<'_, DatabaseState>,
    image_store: State<'_, ImageStore>,
    app_icon_store: State<'_, AppIconStore>,
    file_icon_store: State<'_, FileIconStore>,
    id: String,
) -> Result<Option<ClipboardItem>> {
    let pool = db.pool().await;
    let mut item = find_item_for_list_by_id(&pool, &id).await?;
    if let Some(item) = item.as_mut() {
        let settings = app.state::<SettingsStore>().snapshot();
        let file_entry_limit = settings.clipboard.display.file_entry_limit();
        let redact_sensitive = settings.clipboard.sensitive.redact_secrets;
        attach_image_thumbnail_path(&image_store, item).await?;
        attach_source_app_icon_path(&app_icon_store, item);
        attach_file_entries(&pool, &file_icon_store, item, file_entry_limit).await?;
        attach_color_preview(item);
        attach_display_created_at(item, &Local::now());
        redact_sensitive_list_item(item, redact_sensitive);
        item.available_actions = compute_available_actions(item);
    }
    Ok(item)
}

async fn attach_image_thumbnail_path(store: &ImageStore, item: &mut ClipboardItem) -> Result<()> {
    if item.kind != ClipboardKind::Image {
        return Ok(());
    }

    if validate_image_file_name(&item.content).is_err() {
        item.image_thumbnail_path = None;
        return Ok(());
    }

    let file_name = item.content.clone();
    let thumb_path = store.thumbnail_path(&file_name);
    let thumb_exists = thumb_path.exists();

    let immediate_path = if thumb_exists {
        thumb_path
    } else {
        store.origin_path(&file_name)
    };

    item.image_thumbnail_path = immediate_path.to_str().map(str::to_owned);

    if !thumb_exists {
        let store = store.clone();
        tauri::async_runtime::spawn_blocking(move || {
            if let Err(err) = store.ensure_thumbnail(&file_name) {
                log::warn!("ensure image thumbnail failed for {:?}: {err}", file_name);
            }
        });
    }

    Ok(())
}

fn attach_source_app_icon_path(store: &AppIconStore, item: &mut ClipboardItem) {
    item.source_app_icon_path = item
        .source_app_icon_file
        .as_deref()
        .and_then(|name| store.icon_path(name).to_str().map(str::to_owned));
}

fn build_clipboard_app_view(store: &AppIconStore, app: ClipboardApp) -> ClipboardAppView {
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

fn attach_color_preview(item: &mut ClipboardItem) {
    if item.sub_kind != Some(ClipboardSubKind::Color) {
        return;
    }

    let source = item.summary.as_deref().unwrap_or(&item.content);
    item.color_preview = sanitize_css_color(source);
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

fn attach_display_created_at(item: &mut ClipboardItem, now: &chrono::DateTime<Local>) {
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

fn compute_available_actions(item: &ClipboardItem) -> Vec<ClipboardAction> {
    let mut actions = Vec::with_capacity(10);

    actions.push(ClipboardAction::Paste);

    match item.kind {
        ClipboardKind::Text => actions.push(ClipboardAction::PasteAsPlainText),
        ClipboardKind::Files => actions.push(ClipboardAction::PasteAsPath),
        ClipboardKind::Image => {}
    }

    actions.push(ClipboardAction::Copy);
    if item.kind == ClipboardKind::Image {
        actions.push(ClipboardAction::SaveImage);
    }

    match item.sub_kind {
        Some(ClipboardSubKind::Url) => actions.push(ClipboardAction::OpenLink),
        Some(ClipboardSubKind::Email) => actions.push(ClipboardAction::SendEmail),
        _ => {}
    }

    let can_reveal =
        item.kind == ClipboardKind::Files || item.sub_kind == Some(ClipboardSubKind::Path);
    if can_reveal {
        #[cfg(target_os = "macos")]
        actions.push(ClipboardAction::RevealInFinder);
        #[cfg(target_os = "windows")]
        actions.push(ClipboardAction::RevealInExplorer);
    }

    actions.push(ClipboardAction::ToggleFavorite);
    actions.push(ClipboardAction::TogglePinned);
    actions.push(ClipboardAction::EditNote);
    actions.push(ClipboardAction::Delete);

    actions
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

async fn attach_file_entries(
    pool: &SqlitePool,
    store: &FileIconStore,
    item: &mut ClipboardItem,
    limit: usize,
) -> Result<()> {
    if item.kind != ClipboardKind::Files {
        return Ok(());
    }

    let paths: Vec<&str> = item
        .content
        .split('\n')
        .filter(|p| !p.is_empty())
        .take(limit)
        .collect();
    if paths.is_empty() {
        return Ok(());
    }

    let types: Vec<&str> = item
        .file_types
        .as_deref()
        .unwrap_or("")
        .split(',')
        .collect();

    let mut entries = Vec::with_capacity(paths.len());
    for (index, path) in paths.iter().enumerate() {
        let (icon_path, exists) =
            resolve_file_icon_path(pool, store, path, item.file_types.as_deref(), index).await?;
        let is_dir = types.get(index).copied() == Some("d");
        let name = std::path::Path::new(path)
            .file_name()
            .and_then(|n| n.to_str())
            .map(str::to_owned)
            .unwrap_or_else(|| (*path).to_owned());
        let is_image = !is_dir && is_image_path(path);

        entries.push(FileEntry {
            path: (*path).to_owned(),
            name,
            is_dir,
            is_image,
            exists,
            icon_path,
        });
    }

    item.file_entries = Some(entries);
    item.files_preview_kind = Some(match item.file_entries.as_deref() {
        Some([only]) if only.is_image && only.exists => {
            crate::db::models::FilesPreviewKind::ImagePreview
        }
        _ => crate::db::models::FilesPreviewKind::List,
    });
    Ok(())
}

async fn build_clipboard_preview_payload(
    pool: &SqlitePool,
    image_store: &ImageStore,
    file_icon_store: &FileIconStore,
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
            let path = image_store.origin_path(&item.content);
            image_exists = path.exists();
            image_path = Some(path_to_string(&path, "image path")?);
        }
        ClipboardKind::Files => {
            total_files = count_file_paths(&item.content);
            files = build_preview_file_entries(pool, file_icon_store, &item).await?;
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

async fn build_preview_file_entries(
    pool: &SqlitePool,
    store: &FileIconStore,
    item: &ClipboardItem,
) -> Result<Vec<ClipboardPreviewFileEntry>> {
    let paths: Vec<&str> = item
        .content
        .split('\n')
        .filter(|path| !path.is_empty())
        .take(PREVIEW_FILE_ENTRY_LIMIT)
        .collect();

    let mut entries = Vec::with_capacity(paths.len());
    for (index, path) in paths.iter().enumerate() {
        let (icon_path, exists) =
            resolve_file_icon_path(pool, store, path, item.file_types.as_deref(), index).await?;
        let path_obj = Path::new(path);
        let is_dir = resolve_preview_file_is_dir(path_obj, item.file_types.as_deref(), index);
        let is_image = !is_dir && is_image_path(path);
        let size = resolve_preview_file_size(path_obj, is_dir);
        let name = path_obj
            .file_name()
            .and_then(|n| n.to_str())
            .map(str::to_owned)
            .unwrap_or_else(|| (*path).to_owned());

        entries.push(ClipboardPreviewFileEntry {
            path: (*path).to_owned(),
            name,
            is_dir,
            is_image,
            exists,
            size,
            icon_path,
        });
    }

    Ok(entries)
}

fn count_file_paths(content: &str) -> usize {
    content.split('\n').filter(|path| !path.is_empty()).count()
}

fn resolve_preview_file_is_dir(path: &Path, file_types: Option<&str>, index: usize) -> bool {
    if let Ok(metadata) = path.metadata() {
        return metadata.is_dir();
    }

    file_types
        .and_then(|types| types.split(',').nth(index))
        .map(|file_type| file_type == "d")
        .unwrap_or(false)
}

fn resolve_preview_file_size(path: &Path, is_dir: bool) -> Option<i64> {
    if is_dir {
        return None;
    }

    path.metadata().ok().map(|metadata| metadata.len() as i64)
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

async fn resolve_file_icon_path(
    pool: &SqlitePool,
    file_icon_store: &FileIconStore,
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

    let is_directory = file_types
        .and_then(|types| types.split(',').nth(index))
        .map(|t| t == "d");

    let cache_key = if is_directory == Some(true) {
        crate::clipboard::DIR_CACHE_KEY.to_string()
    } else {
        crate::clipboard::get_icon_cache_key(path_obj)
    };

    if let Some(icon_file) = crate::db::file_icons::get_icon(pool, &cache_key, platform).await? {
        let icon_path = file_icon_store.icon_path(&icon_file);
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

    let icon_file = file_icon_store.store(&png)?;
    crate::db::file_icons::upsert_icon(pool, &cache_key, platform, &icon_file).await?;

    let icon_path = file_icon_store.icon_path(&icon_file);
    Ok((icon_path.to_str().map(str::to_owned), exists))
}

#[tauri::command]
pub async fn list_clipboard_apps(
    db: State<'_, DatabaseState>,
    ids: Vec<String>,
) -> Result<Vec<crate::db::models::ClipboardApp>> {
    let pool = db.pool().await;
    crate::db::apps::list_apps_by_ids(&pool, &ids).await
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
        .map(|app| build_clipboard_app_view(&app_icon_store, app))
        .collect())
}

#[tauri::command]
pub async fn add_clipboard_app_from_path(
    app_icon_store: State<'_, AppIconStore>,
    registry: State<'_, AppsRegistry>,
    path: String,
) -> Result<ClipboardAppView> {
    let app = add_app_from_path(registry.inner().clone(), path).await?;

    Ok(build_clipboard_app_view(&app_icon_store, app))
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
    if let Err(err) = app.emit(CLIPBOARD_GROUPS_UPDATED_EVENT, ()) {
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
    db: State<'_, DatabaseState>,
    store: State<'_, ImageStore>,
    id: String,
) -> Result<()> {
    let pool = db.pool().await;
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

    if let Err(err) = app.emit(
        CLIPBOARD_UPDATED_EVENT,
        serde_json::json!({
            "cleanup": outcome.removed,
        }),
    ) {
        log::warn!("emit {CLIPBOARD_UPDATED_EVENT} after clear failed: {err}");
    }

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

fn validate_image_file_name(file_name: &str) -> Result<()> {
    let invalid = file_name.is_empty()
        || file_name.contains('/')
        || file_name.contains('\\')
        || file_name.contains("..")
        || !file_name.ends_with(".png");

    if invalid {
        return Err(AppError::Clipboard(format!(
            "invalid image file name: {file_name:?}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::items::content_hash;
    use crate::db::models::Platform;
    use chrono::{TimeZone, Utc};

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

    fn image_item() -> ClipboardItem {
        let content = "abcdef0123.png".to_owned();
        let mut item = text_item(None, false);

        item.kind = ClipboardKind::Image;
        item.content_hash = content_hash(ClipboardKind::Image, &content);
        item.content = content;
        item
    }

    #[test]
    fn copy_plain_default_only_affects_text_items() {
        assert!(should_write_plain_for_copy(
            false,
            ClipboardKind::Text,
            true
        ));
        assert!(!should_write_plain_for_copy(
            false,
            ClipboardKind::Files,
            true
        ));
        assert!(!should_write_plain_for_copy(
            false,
            ClipboardKind::Image,
            true
        ));
        assert!(!should_write_plain_for_copy(
            false,
            ClipboardKind::Text,
            false
        ));
    }

    #[test]
    fn force_plain_copy_overrides_item_kind() {
        assert!(should_write_plain_for_copy(
            true,
            ClipboardKind::Files,
            false
        ));
        assert!(should_write_plain_for_copy(
            true,
            ClipboardKind::Image,
            false
        ));
    }

    #[test]
    fn paste_plain_defaults_follow_item_kind() {
        assert!(should_write_plain_for_paste(
            false,
            ClipboardKind::Text,
            true,
            false,
        ));
        assert!(should_write_plain_for_paste(
            false,
            ClipboardKind::Files,
            false,
            true,
        ));
        assert!(!should_write_plain_for_paste(
            false,
            ClipboardKind::Files,
            true,
            false,
        ));
        assert!(!should_write_plain_for_paste(
            false,
            ClipboardKind::Image,
            true,
            true,
        ));
    }

    #[test]
    fn force_plain_paste_overrides_item_kind() {
        assert!(should_write_plain_for_paste(
            true,
            ClipboardKind::Image,
            false,
            false,
        ));
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

    #[test]
    fn image_actions_include_save_image() {
        let actions = compute_available_actions(&image_item());

        assert!(actions.contains(&ClipboardAction::SaveImage));
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

    #[test]
    fn text_actions_do_not_include_save_image() {
        let actions = compute_available_actions(&text_item(None, false));

        assert!(!actions.contains(&ClipboardAction::SaveImage));
    }

    #[test]
    fn accepts_plain_png_file_name() {
        assert!(validate_image_file_name("abcdef0123.png").is_ok());
    }

    #[test]
    fn rejects_traversal_and_subpaths() {
        for bad in [
            "",
            "evil.txt",
            "../secret.png",
            "..\\secret.png",
            "sub/dir.png",
            "a/b.png",
            "/abs.png",
            "name..png",
        ] {
            assert!(
                validate_image_file_name(bad).is_err(),
                "should reject: {bad:?}"
            );
        }
    }
}
