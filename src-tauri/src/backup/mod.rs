use std::fs::{self, File};
use std::path::{Path, PathBuf};
#[cfg(target_os = "macos")]
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, Mutex};

use anyhow::{anyhow, Context};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter, Manager};
use tempfile::NamedTempFile;
use walkdir::WalkDir;
use zeroize::Zeroizing;

use crate::core::events;
use crate::core::{AppError, Result};
use crate::settings::change::{self, OnEffectFailure, SettingsWrite};

mod archive;
mod container;
pub mod legacy;
mod merge;

use archive::{
    extract_payload_zip, validate_extracted_payload, write_payload_zip, BackupManifest,
    BackupSourcePaths, ManifestEncryption, CONFIG_ARCHIVE_DIR, DB_ARCHIVE_DIR, DB_FILENAME,
    RESOURCES_ARCHIVE_DIR, SETTINGS_FILENAME,
};
use container::{inspect_backup_reader, read_backup_payload, write_container, FORMAT_VERSION};
use merge::{merge_history, open_backup_db};

pub const BACKUP_EXTENSION: &str = "ecopastebak";

static PENDING_BACKUP: LazyLock<Mutex<Option<BackupReceivedPayload>>> =
    LazyLock::new(|| Mutex::new(None));

#[cfg(target_os = "macos")]
static APP_READY: AtomicBool = AtomicBool::new(false);

#[cfg(target_os = "macos")]
static PENDING_OPEN_FILES: LazyLock<Mutex<Vec<PathBuf>>> = LazyLock::new(|| Mutex::new(Vec::new()));

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportHistoryBackupOptions {
    pub mode: BackupExportMode,
    pub password: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportHistoryBackupInput {
    pub path: String,
    pub password: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportHistoryBackupOptions {
    pub strategy: BackupImportStrategy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BackupImportStrategy {
    Merge,
    Overwrite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BackupExportMode {
    Encrypted,
    Plain,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportHistoryBackupResult {
    pub path: String,
    pub total_bytes: u64,
    pub item_count: i64,
    pub text_count: i64,
    pub image_count: i64,
    pub files_count: i64,
    pub resource_bytes: u64,
    pub exported_at: DateTime<Utc>,
    pub mode: BackupExportMode,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportHistoryBackupResult {
    pub strategy: BackupImportStrategy,
    pub imported_items: u64,
    pub skipped_items: u64,
    pub imported_resources: u64,
    pub imported_settings: bool,
    pub requires_restart: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupReceivedPayload {
    pub path: String,
    pub source: BackupReceiveSource,
    pub mode: BackupContainerMode,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BackupReceiveSource {
    OpenFile,
    DragDrop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BackupContainerMode {
    Encrypted,
    Plain,
}

#[derive(Debug, Clone, Copy)]
struct BackupCounts {
    item_count: i64,
    text_count: i64,
    image_count: i64,
    files_count: i64,
}

pub async fn export_history_backup(
    app: &AppHandle,
    pool: &SqlitePool,
    target_path: String,
    options: ExportHistoryBackupOptions,
) -> Result<ExportHistoryBackupResult> {
    let target = normalize_backup_path(PathBuf::from(target_path))?;
    let password = validate_password_options(&options)?;
    let exported_at = Utc::now();
    let counts = load_counts(pool).await?;
    let source_paths = backup_source_paths(app)?;
    let resource_bytes = dir_size(&source_paths.resources_dir)?;
    let manifest = build_manifest(app, exported_at, options.mode, counts, resource_bytes)?;

    checkpoint_database(pool).await?;

    let payload_file = NamedTempFile::new().context("failed to create temporary backup payload")?;
    write_payload_zip(&source_paths, payload_file.path(), &manifest, &target)?;
    let total_bytes = write_container(&target, payload_file.path(), options.mode, password)?;

    Ok(ExportHistoryBackupResult {
        path: target.to_string_lossy().into_owned(),
        total_bytes,
        item_count: counts.item_count,
        text_count: counts.text_count,
        image_count: counts.image_count,
        files_count: counts.files_count,
        resource_bytes,
        exported_at,
        mode: options.mode,
    })
}

pub async fn import_history_backup(
    app: &AppHandle,
    db: &crate::db::DatabaseState,
    input: ImportHistoryBackupInput,
    options: ImportHistoryBackupOptions,
) -> Result<ImportHistoryBackupResult> {
    validate_import_options(&input, &options)?;

    let path = PathBuf::from(input.path);
    ensure_backup_extension(&path)?;
    let payload = read_backup_payload(&path, input.password.as_deref())?;
    let temp = extract_payload_zip(&payload)?;
    validate_extracted_payload(temp.path())?;

    match options.strategy {
        BackupImportStrategy::Merge => {
            let pool = db.pool().await;
            merge_import(app, &pool, temp.path(), &options).await
        }
        BackupImportStrategy::Overwrite => overwrite_import(app, db, temp.path(), &options).await,
    }
}

pub fn inspect_backup_file(path: &Path) -> Result<BackupContainerMode> {
    ensure_backup_extension(path)?;

    let mut file = File::open(path).with_context(|| format!("failed to open backup {path:?}"))?;
    inspect_backup_reader(&mut file)
}

pub fn emit_received_backup(
    app: &AppHandle,
    path: PathBuf,
    source: BackupReceiveSource,
) -> Result<()> {
    let mode = inspect_backup_file(&path)?;

    let payload = BackupReceivedPayload {
        path: path.to_string_lossy().into_owned(),
        source,
        mode,
    };

    let exists = app
        .get_webview_window(crate::window::PREFERENCE_WINDOW_LABEL)
        .is_some();

    if !exists {
        set_pending_backup(payload.clone());
    }

    crate::window::show_window(app, crate::window::PREFERENCE_WINDOW_LABEL)?;

    if exists {
        app.emit(events::BACKUP_RECEIVED, payload)
            .context("failed to emit backup received event")?;
    }

    Ok(())
}

fn set_pending_backup(payload: BackupReceivedPayload) {
    let mut guard = PENDING_BACKUP.lock().unwrap_or_else(|poisoned| {
        log::error!("pending backup mutex poisoned on set, recovering");
        poisoned.into_inner()
    });
    *guard = Some(payload);
}

pub fn take_pending_backup() -> Option<BackupReceivedPayload> {
    let mut guard = PENDING_BACKUP.lock().unwrap_or_else(|poisoned| {
        log::error!("pending backup mutex poisoned on take, recovering");
        poisoned.into_inner()
    });
    guard.take()
}

#[cfg(target_os = "macos")]
pub fn handle_open_file(app: &AppHandle, path: PathBuf, source: BackupReceiveSource) {
    if !is_backup_path(&path) {
        return;
    }

    if !APP_READY.load(Ordering::Acquire) {
        let mut guard = PENDING_OPEN_FILES.lock().unwrap_or_else(|poisoned| {
            log::error!("pending open files mutex poisoned on push, recovering");
            poisoned.into_inner()
        });
        guard.push(path);
        return;
    }

    if let Err(err) = emit_received_backup(app, path, source) {
        log::error!("handle open file failed: {err:?}");
    }
}

#[cfg(target_os = "macos")]
pub fn mark_app_ready(app: &AppHandle) {
    APP_READY.store(true, Ordering::Release);

    let pending: Vec<PathBuf> = {
        let mut guard = PENDING_OPEN_FILES.lock().unwrap_or_else(|poisoned| {
            log::error!("pending open files mutex poisoned on drain, recovering");
            poisoned.into_inner()
        });
        std::mem::take(&mut *guard)
    };

    for path in pending {
        if let Err(err) = emit_received_backup(app, path, BackupReceiveSource::OpenFile) {
            log::error!("drain pending open file failed: {err:?}");
        }
    }
}

pub fn backup_path_from_args(args: &[String]) -> Option<PathBuf> {
    args.iter()
        .map(PathBuf::from)
        .find(|path| is_backup_path(path))
}

pub fn is_backup_path(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case(BACKUP_EXTENSION))
}

fn validate_password_options(
    options: &ExportHistoryBackupOptions,
) -> Result<Option<Zeroizing<String>>> {
    match options.mode {
        BackupExportMode::Encrypted => {
            let Some(password) = options.password.as_ref() else {
                return app_error("백업 비밀번호를 입력하세요");
            };
            if password.chars().count() < 8 {
                return app_error("백업 비밀번호는 8자 이상이어야 합니다");
            }

            Ok(Some(Zeroizing::new(password.clone())))
        }
        BackupExportMode::Plain => {
            if options
                .password
                .as_deref()
                .is_some_and(|value| !value.is_empty())
            {
                return app_error("암호화되지 않은 백업에는 비밀번호를 포함할 수 없습니다");
            }

            Ok(None)
        }
    }
}

fn validate_import_options(
    input: &ImportHistoryBackupInput,
    _options: &ImportHistoryBackupOptions,
) -> Result<()> {
    let path = PathBuf::from(input.path.as_str());
    match inspect_backup_file(&path)? {
        BackupContainerMode::Encrypted => {
            if input.password.as_deref().is_none_or(str::is_empty) {
                return app_error("백업 비밀번호를 입력하세요");
            }
        }
        BackupContainerMode::Plain => {
            if input
                .password
                .as_deref()
                .is_some_and(|value| !value.is_empty())
            {
                return app_error("암호화되지 않은 백업에는 비밀번호를 포함할 수 없습니다");
            }
        }
    }

    Ok(())
}

fn normalize_backup_path(path: PathBuf) -> Result<PathBuf> {
    if path.as_os_str().is_empty() {
        return app_error("백업 저장 위치를 선택하세요");
    }

    match path.extension().and_then(|value| value.to_str()) {
        Some(ext) if ext.eq_ignore_ascii_case(BACKUP_EXTENSION) => Ok(path),
        Some(_) => app_error(format!(
            "백업 파일 확장자는 .{BACKUP_EXTENSION}이어야 합니다"
        )),
        None => Ok(path.with_extension(BACKUP_EXTENSION)),
    }
}

fn ensure_backup_extension(path: &Path) -> Result<()> {
    if is_backup_path(path) {
        return Ok(());
    }

    app_error(format!(".{BACKUP_EXTENSION} 백업 파일을 선택하세요"))
}

async fn checkpoint_database(pool: &SqlitePool) -> Result<()> {
    sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
        .execute(pool)
        .await
        .context("failed to checkpoint backup database")?;

    Ok(())
}

async fn load_counts(pool: &SqlitePool) -> Result<BackupCounts> {
    let item_count = count_items(pool, None).await?;
    let text_count = count_items(pool, Some("text")).await?;
    let image_count = count_items(pool, Some("image")).await?;
    let files_count = count_items(pool, Some("files")).await?;

    Ok(BackupCounts {
        item_count,
        text_count,
        image_count,
        files_count,
    })
}

async fn count_items(pool: &SqlitePool, kind: Option<&str>) -> Result<i64> {
    let count = if let Some(kind) = kind {
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM clipboard_items WHERE kind = ?")
            .bind(kind)
            .fetch_one(pool)
            .await
    } else {
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM clipboard_items")
            .fetch_one(pool)
            .await
    }
    .context("failed to count backup items")?;

    Ok(count)
}

fn build_manifest(
    app: &AppHandle,
    exported_at: DateTime<Utc>,
    mode: BackupExportMode,
    counts: BackupCounts,
    resource_bytes: u64,
) -> Result<BackupManifest> {
    Ok(BackupManifest {
        format_version: FORMAT_VERSION,
        app_name: app.package_info().name.to_string(),
        app_version: app.package_info().version.to_string(),
        exported_at,
        platform: current_platform().to_owned(),
        encryption: match mode {
            BackupExportMode::Encrypted => ManifestEncryption::Password,
            BackupExportMode::Plain => ManifestEncryption::None,
        },
        item_count: counts.item_count,
        text_count: counts.text_count,
        image_count: counts.image_count,
        files_count: counts.files_count,
        resource_bytes,
    })
}

fn current_platform() -> &'static str {
    if cfg!(target_os = "macos") {
        "macos"
    } else {
        "windows"
    }
}

fn backup_source_paths(app: &AppHandle) -> Result<BackupSourcePaths> {
    Ok(BackupSourcePaths {
        db_path: crate::db::db_path(app)?,
        resources_dir: crate::core::paths::resources_dir(app)?,
        settings_path: crate::core::paths::config_dir(app)?.join(SETTINGS_FILENAME),
    })
}

async fn merge_import(
    app: &AppHandle,
    pool: &SqlitePool,
    root: &Path,
    _options: &ImportHistoryBackupOptions,
) -> Result<ImportHistoryBackupResult> {
    let db_path = root.join(DB_ARCHIVE_DIR).join(DB_FILENAME);
    let backup_pool = open_backup_db(&db_path).await?;
    let outcome = merge_history(pool, &backup_pool).await?;
    let resources = root.join(RESOURCES_ARCHIVE_DIR);
    let imported_resources =
        copy_dir_missing(&resources, &crate::core::paths::resources_dir(app)?)?;
    emit_clipboard_imported(app);

    let settings_path = root.join(CONFIG_ARCHIVE_DIR).join(SETTINGS_FILENAME);
    let mut patch = read_json_file(&settings_path)?;
    if let Some(settings) = patch.as_object_mut() {
        settings.remove("sync");
    }
    change::commit(app, SettingsWrite::Patch(patch), OnEffectFailure::LogOnly).await?;

    Ok(ImportHistoryBackupResult {
        strategy: BackupImportStrategy::Merge,
        imported_items: outcome.imported_items,
        skipped_items: outcome.skipped_items,
        imported_resources,
        imported_settings: true,
        requires_restart: false,
    })
}

async fn overwrite_import(
    app: &AppHandle,
    db: &crate::db::DatabaseState,
    root: &Path,
    _options: &ImportHistoryBackupOptions,
) -> Result<ImportHistoryBackupResult> {
    let _pause_guard = pause_watcher(app);

    let db_src = root.join(DB_ARCHIVE_DIR).join(DB_FILENAME);
    let app_for_db = app.clone();
    db.close_and_replace(|| {
        let app = app_for_db.clone();
        let db_src = db_src.clone();
        async move {
            replace_live_database(&app, &db_src)?;
            crate::db::init(&app).await
        }
    })
    .await?;

    let resources_src = root.join(RESOURCES_ARCHIVE_DIR);
    if resources_src.exists() {
        replace_dir(&resources_src, &crate::core::paths::resources_dir(app)?)?;
    }

    refresh_apps_registry(app).await;
    emit_clipboard_imported(app);

    let settings_path = root.join(CONFIG_ARCHIVE_DIR).join(SETTINGS_FILENAME);
    change::commit(
        app,
        SettingsWrite::ReplaceFromFile(&settings_path),
        OnEffectFailure::LogOnly,
    )
    .await?;

    Ok(ImportHistoryBackupResult {
        strategy: BackupImportStrategy::Overwrite,
        imported_items: 0,
        skipped_items: 0,
        imported_resources: 0,
        imported_settings: true,
        requires_restart: false,
    })
}

struct WatcherPauseRestore {
    pause: Option<crate::clipboard::WatcherPause>,
    previous: bool,
}

impl Drop for WatcherPauseRestore {
    fn drop(&mut self) {
        if let Some(pause) = &self.pause {
            pause.set_paused(self.previous);
        }
    }
}

fn pause_watcher(app: &AppHandle) -> WatcherPauseRestore {
    let pause = app.try_state::<crate::clipboard::WatcherPause>();
    let previous = pause.as_ref().is_some_and(|state| state.is_paused());
    if let Some(state) = pause.as_ref() {
        state.set_paused(true);
    }

    WatcherPauseRestore {
        pause: pause.map(|state| state.inner().clone()),
        previous,
    }
}

fn replace_live_database(app: &AppHandle, src: &Path) -> Result<()> {
    let dst = crate::db::db_path(app)?;
    copy_file_to(src, &dst)?;

    for suffix in ["-wal", "-shm"] {
        let sidecar = PathBuf::from(format!("{}{suffix}", dst.display()));
        if sidecar.exists() {
            fs::remove_file(&sidecar).with_context(|| format!("failed to remove {sidecar:?}"))?;
        }
    }

    Ok(())
}

async fn refresh_apps_registry(app: &AppHandle) {
    let Some(registry) = app.try_state::<crate::clipboard::AppsRegistry>() else {
        return;
    };

    if let Err(err) = registry.load_from_db().await {
        log::warn!("refresh apps registry after backup overwrite failed: {err}");
    }
}

fn read_json_file(path: &Path) -> Result<serde_json::Value> {
    let content = fs::read_to_string(path).with_context(|| format!("failed to read {path:?}"))?;
    Ok(serde_json::from_str(&content).with_context(|| format!("failed to parse {path:?}"))?)
}

fn copy_file_to(src: &Path, dst: &Path) -> Result<()> {
    let parent = dst
        .parent()
        .ok_or_else(|| anyhow!("target path has no parent"))?;
    fs::create_dir_all(parent).with_context(|| format!("failed to create {parent:?}"))?;
    fs::copy(src, dst).with_context(|| format!("failed to copy {src:?} to {dst:?}"))?;

    Ok(())
}

fn copy_dir_all(src: &Path, dst: &Path) -> Result<u64> {
    if !src.exists() {
        return Ok(0);
    }

    let mut copied = 0;
    for entry in WalkDir::new(src)
        .into_iter()
        .filter_map(std::result::Result::ok)
    {
        let path = entry.path();
        let relative = path
            .strip_prefix(src)
            .with_context(|| format!("failed to strip {src:?} from {path:?}"))?;
        if relative.as_os_str().is_empty() {
            continue;
        }

        let target = dst.join(relative);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&target)
                .with_context(|| format!("failed to create directory {target:?}"))?;
            continue;
        }

        copy_file_to(path, &target)?;
        copied += 1;
    }

    Ok(copied)
}

fn copy_dir_missing(src: &Path, dst: &Path) -> Result<u64> {
    if !src.exists() {
        return Ok(0);
    }

    let mut copied = 0;
    for entry in WalkDir::new(src)
        .into_iter()
        .filter_map(std::result::Result::ok)
    {
        let path = entry.path();
        if !entry.file_type().is_file() {
            continue;
        }

        let relative = path
            .strip_prefix(src)
            .with_context(|| format!("failed to strip {src:?} from {path:?}"))?;
        let target = dst.join(relative);
        if target.exists() {
            continue;
        }

        copy_file_to(path, &target)?;
        copied += 1;
    }

    Ok(copied)
}

fn replace_dir(src: &Path, dst: &Path) -> Result<()> {
    if dst.exists() {
        fs::remove_dir_all(dst).with_context(|| format!("failed to remove {dst:?}"))?;
    }
    copy_dir_all(src, dst)?;

    Ok(())
}

fn emit_clipboard_imported(app: &AppHandle) {
    crate::clipboard::emit_clipboard_updated(
        app,
        crate::clipboard::ClipboardUpdated::Imported { imported: true },
    );
}

fn dir_size(path: &Path) -> Result<u64> {
    if !path.exists() {
        return Ok(0);
    }

    let mut total = 0;
    for entry in WalkDir::new(path)
        .into_iter()
        .filter_map(std::result::Result::ok)
    {
        let path = entry.path();
        let metadata = entry
            .metadata()
            .with_context(|| format!("failed to read metadata at {path:?}"))?;

        if metadata.is_file() {
            total += metadata.len();
        }
    }

    Ok(total)
}

fn app_error<T>(message: impl Into<String>) -> Result<T> {
    Err(AppError::Other(anyhow!(message.into())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backup_extension_validation_accepts_expected_suffix() {
        let path = normalize_backup_path(PathBuf::from("demo.ecopastebak")).unwrap();
        assert_eq!(path, PathBuf::from("demo.ecopastebak"));
    }

    #[test]
    fn backup_extension_validation_appends_missing_suffix() {
        let path = normalize_backup_path(PathBuf::from("demo")).unwrap();
        assert_eq!(path, PathBuf::from("demo.ecopastebak"));
    }

    #[test]
    fn backup_extension_validation_rejects_other_suffix() {
        assert!(normalize_backup_path(PathBuf::from("demo.zip")).is_err());
    }
}
