use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use clipboard_rs::{ClipboardHandler, ClipboardWatcher, ClipboardWatcherContext};
use serde_json::json;
use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter, Manager};

use super::app_store::AppIconStore;
use super::apps_registry::AppsRegistry;
use super::guard::WritebackGuard;
use super::ingest::build_item_with_settings;
use super::read::ClipboardReader;
use super::sound;
use super::source::{self, FrontmostApp};
use super::storage::ImageStore;
use crate::db::apps::upsert_app;
use crate::db::items::{upsert_item, UpsertResult};
use crate::db::models::{ClipboardApp, ClipboardItem};
use crate::settings::SettingsStore;

pub const CLIPBOARD_UPDATED_EVENT: &str = "clipboard://updated";

const CLIPBOARD_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(120);

/// Another clipboard listener can briefly hold the Windows clipboard open. Retry those read
/// failures within a bounded window before dropping the update.
const CLIPBOARD_READ_RETRY_DELAYS: [Duration; 3] = [
    Duration::from_millis(15),
    Duration::from_millis(35),
    Duration::from_millis(75),
];

fn read_with_retry<T, E>(
    retry_delays: &[Duration],
    mut read: impl FnMut() -> Result<Option<T>, E>,
) -> Result<Option<T>, E> {
    let mut result = read();
    for delay in retry_delays {
        if result.is_ok() {
            return result;
        }
        std::thread::sleep(*delay);
        result = read();
    }
    result
}

#[derive(Debug, Default, Clone)]
pub struct WatcherPause(Arc<AtomicBool>);

impl WatcherPause {
    pub fn is_paused(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }

    pub fn set_paused(&self, paused: bool) {
        self.0.store(paused, Ordering::Relaxed);
    }
}

pub fn materialize_source(
    store: &AppIconStore,
    registry: Option<&AppsRegistry>,
    src: FrontmostApp,
) -> ClipboardApp {
    if let Some(reg) = registry {
        if let Some(cached) = reg.get(&src.id) {
            return cached;
        }
    }

    let icon_file = src
        .icon_png
        .as_deref()
        .and_then(|bytes| match store.store(bytes) {
            Ok(name) => Some(name),
            Err(err) => {
                log::warn!("app icon store failed for {}: {err}", src.id);
                None
            }
        });
    let now = Utc::now();
    let app = ClipboardApp {
        id: src.id,
        name: src.name,
        icon_file,
        platform: src.platform,
        created_at: now,
        updated_at: now,
    };
    if let Some(reg) = registry {
        reg.insert_into_cache(app.clone());
    }
    app
}

pub async fn persist_and_notify(
    app: &AppHandle,
    pool: &SqlitePool,
    item: &ClipboardItem,
    source_app: Option<&ClipboardApp>,
) -> crate::core::Result<UpsertResult> {
    let mut item_to_write = item.clone();
    if let Some(src) = source_app {
        match upsert_app(pool, src).await {
            Ok(()) => {}
            Err(err) => {
                log::warn!("clipboard source app upsert failed ({}): {err}", src.id);
                item_to_write.source_app_id = None;
            }
        }
    }
    let result = upsert_item(pool, &item_to_write).await?;
    if let Err(err) = app.emit(
        CLIPBOARD_UPDATED_EVENT,
        json!({
            "id": result.id,
            "kind": item_to_write.kind,
            "deduplicated": result.deduplicated,
        }),
    ) {
        log::warn!("emit {CLIPBOARD_UPDATED_EVENT} failed: {err}");
    }
    Ok(result)
}

pub fn init(app: &AppHandle) -> crate::core::Result<()> {
    let guard = Arc::new(WritebackGuard::new());
    app.manage(guard.clone());

    let store = ImageStore::new(app)?;
    app.manage(store.clone());

    let app_icon_store = AppIconStore::new(app)?;
    app.manage(app_icon_store.clone());

    let file_icon_store = super::FileIconStore::new(app)?;
    app.manage(file_icon_store);

    let registry = AppsRegistry::new(app.clone(), app_icon_store.clone());
    app.manage(registry.clone());

    let pause = WatcherPause::default();
    app.manage(pause.clone());

    {
        let registry = registry.clone();
        let excluded_app_ids = app
            .try_state::<SettingsStore>()
            .map(|store| store.snapshot().clipboard.filters.excluded_app_ids)
            .unwrap_or_default();
        tauri::async_runtime::spawn(async move {
            if let Err(err) = registry.load_from_db().await {
                log::warn!("apps registry: initial DB load failed: {err}");
            }
            if let Err(err) =
                super::apps_registry::add_apps_from_ids(registry.clone(), excluded_app_ids).await
            {
                log::warn!("apps registry: initial excluded app materialization failed: {err}");
            }
        });
    }

    super::cleanup::spawn(app.clone());
    spawn_watch_thread(app.clone(), guard, store, app_icon_store, registry, pause);
    Ok(())
}

fn spawn_watch_thread(
    app: AppHandle,
    guard: Arc<WritebackGuard>,
    store: ImageStore,
    app_icon_store: AppIconStore,
    registry: AppsRegistry,
    pause: WatcherPause,
) {
    std::thread::Builder::new()
        .name("clipboard-watcher".to_owned())
        .spawn(move || {
            let reader = match ClipboardReader::new() {
                Ok(reader) => reader,
                Err(err) => {
                    log::error!("clipboard watcher: failed to create reader: {err}");
                    return;
                }
            };

            let mut watcher =
                match ClipboardWatcherContext::new_with_interval(CLIPBOARD_POLL_INTERVAL) {
                    Ok(watcher) => watcher,
                    Err(err) => {
                        log::error!("clipboard watcher: failed to create watcher: {err}");
                        return;
                    }
                };

            watcher.add_handler(ClipboardChangeHandler {
                reader,
                app,
                guard,
                store,
                app_icon_store,
                registry,
                pause,
            });

            log::info!("clipboard watcher started");

            watcher.start_watch();
        })
        .expect("failed to spawn clipboard watcher thread");
}

struct ClipboardChangeHandler {
    reader: ClipboardReader,
    app: AppHandle,
    guard: Arc<WritebackGuard>,
    store: ImageStore,
    app_icon_store: AppIconStore,
    registry: AppsRegistry,
    pause: WatcherPause,
}

impl ClipboardHandler for ClipboardChangeHandler {
    fn on_clipboard_change(&mut self) {
        if self.pause.is_paused() {
            return;
        }

        let source = source::detect_frontmost();

        if let Some(src) = &source {
            let excluded = self
                .app
                .try_state::<SettingsStore>()
                .map(|s| {
                    s.snapshot()
                        .clipboard
                        .filters
                        .excluded_app_ids
                        .iter()
                        .any(|id| id == &src.id)
                })
                .unwrap_or(false);
            if excluded {
                return;
            }
        }

        let settings = self
            .app
            .try_state::<SettingsStore>()
            .map(|s| s.snapshot())
            .unwrap_or_default();

        let payload = match read_with_retry(&CLIPBOARD_READ_RETRY_DELAYS, || {
            self.reader.read_with_capture(&settings.clipboard.capture)
        }) {
            Ok(Some(payload)) => payload,
            Ok(None) => return,
            Err(err) => {
                log::warn!("clipboard watcher: read failed: {err}");
                return;
            }
        };

        let mut item = match build_item_with_settings(
            &self.store,
            &payload,
            &settings.clipboard.capture,
            &settings.clipboard.sensitive,
            settings.clipboard.content.copy_plain,
        ) {
            Ok(Some(item)) => item,
            Ok(None) => return,
            Err(err) => {
                log::warn!("clipboard watcher: build item failed: {err}");
                return;
            }
        };

        if self.guard.should_skip(&item.content_hash) {
            return;
        }

        let source_app =
            source.map(|src| materialize_source(&self.app_icon_store, Some(&self.registry), src));
        if let Some(src) = &source_app {
            item.source_app_id = Some(src.id.clone());
        }

        let app = self.app.clone();
        tauri::async_runtime::spawn(async move {
            let pool = app.state::<crate::db::DatabaseState>().pool().await;
            match persist_and_notify(&app, &pool, &item, source_app.as_ref()).await {
                Ok(_) => {
                    if let Some(runtime) = app.try_state::<crate::sync::SyncRuntime>() {
                        runtime.publish_local(&payload);
                    }
                    sound::maybe_play_copy(&app);
                }
                Err(err) => log::error!("clipboard watcher: persist failed: {err}"),
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use clipboard_rs::{Clipboard, ClipboardContext};

    use super::*;
    use crate::clipboard::{build_item, ImageStore, WritebackGuard};
    use crate::db::items::find_item_by_id;
    use crate::db::test_support::memory_pool;

    const ZERO_DELAY_RETRIES: [Duration; 3] = [Duration::ZERO; 3];

    #[test]
    fn clipboard_read_retry_returns_immediate_success() {
        let attempts = Cell::new(0);

        let result = read_with_retry(&ZERO_DELAY_RETRIES, || {
            attempts.set(attempts.get() + 1);
            Ok::<_, &'static str>(Some("captured"))
        });

        assert_eq!(result, Ok(Some("captured")));
        assert_eq!(attempts.get(), 1);
    }

    #[test]
    fn clipboard_read_retry_recovers_after_transient_error() {
        let attempts = Cell::new(0);

        let result = read_with_retry(&ZERO_DELAY_RETRIES, || {
            attempts.set(attempts.get() + 1);
            if attempts.get() == 1 {
                Err("clipboard busy")
            } else {
                Ok(Some("captured"))
            }
        });

        assert_eq!(result, Ok(Some("captured")));
        assert_eq!(attempts.get(), 2);
    }

    #[test]
    fn clipboard_read_retry_does_not_retry_empty_content() {
        let attempts = Cell::new(0);

        let result = read_with_retry(&ZERO_DELAY_RETRIES, || {
            attempts.set(attempts.get() + 1);
            Ok::<Option<&'static str>, &'static str>(None)
        });

        assert_eq!(result, Ok(None));
        assert_eq!(attempts.get(), 1);
    }

    #[test]
    fn clipboard_read_retry_returns_final_error_after_exhaustion() {
        let attempts = Cell::new(0);

        let result = read_with_retry(&ZERO_DELAY_RETRIES, || {
            attempts.set(attempts.get() + 1);
            Err::<Option<&'static str>, _>(attempts.get())
        });

        assert_eq!(result, Err(4));
        assert_eq!(attempts.get(), 4);
    }

    fn temp_image_store() -> (TempDir, ImageStore) {
        let dir = TempDir::new();
        let store = ImageStore::for_test(dir.path().join("resources").join("clipboard-images"));
        (dir, store)
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "touches the real system clipboard; run with --ignored on a desktop session"]
    async fn end_to_end_text_ingests_once_then_dedups() {
        let pool = memory_pool().await;
        let guard = WritebackGuard::new();
        let (_dir, store) = temp_image_store();

        let item = {
            let _serial = crate::clipboard::test_lock::serial();
            let ctx = ClipboardContext::new().unwrap();
            ctx.set_text("e2e ultra clipboard watcher".to_owned())
                .unwrap();

            let reader = ClipboardReader::new().unwrap();
            let payload = reader
                .read_with_capture(&crate::settings::Capture::default())
                .unwrap()
                .expect("should read text");
            build_item(&store, &payload)
                .unwrap()
                .expect("should map to item")
        };
        assert!(!guard.should_skip(&item.content_hash));

        let first = upsert_item(&pool, &item).await.unwrap();
        assert!(!first.deduplicated);
        assert_eq!(
            find_item_by_id(&pool, &first.id)
                .await
                .unwrap()
                .unwrap()
                .content,
            "e2e ultra clipboard watcher"
        );

        let second = upsert_item(&pool, &item).await.unwrap();
        assert!(second.deduplicated);
        assert_eq!(first.id, second.id);
        assert_eq!(
            find_item_by_id(&pool, &first.id)
                .await
                .unwrap()
                .unwrap()
                .use_count,
            2
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "touches the real system clipboard; run with --ignored on a desktop session"]
    async fn writeback_guard_suppresses_self_copy() {
        let (_dir, store) = temp_image_store();
        let _serial = crate::clipboard::test_lock::serial();
        let ctx = ClipboardContext::new().unwrap();
        ctx.set_text("self writeback content".to_owned()).unwrap();

        let reader = ClipboardReader::new().unwrap();
        let payload = reader
            .read_with_capture(&crate::settings::Capture::default())
            .unwrap()
            .unwrap();
        let item = build_item(&store, &payload).unwrap().unwrap();

        let guard = WritebackGuard::new();
        guard.suppress(item.content_hash.clone());
        assert!(guard.should_skip(&item.content_hash));
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "touches the real system clipboard; run with --ignored on a desktop session"]
    async fn end_to_end_image_stores_and_ingests() {
        use clipboard_rs::common::RustImage;

        let pool = memory_pool().await;
        let (_dir, store) = temp_image_store();

        let item = {
            let _serial = crate::clipboard::test_lock::serial();
            let png = {
                use std::io::Cursor;
                let buf = image::RgbaImage::from_pixel(40, 24, image::Rgba([7, 8, 9, 255]));
                let mut out = Cursor::new(Vec::new());
                image::DynamicImage::ImageRgba8(buf)
                    .write_to(&mut out, image::ImageFormat::Png)
                    .unwrap();
                out.into_inner()
            };
            let ctx = ClipboardContext::new().unwrap();
            ctx.set_image(clipboard_rs::RustImageData::from_bytes(&png).unwrap())
                .unwrap();

            let reader = ClipboardReader::new().unwrap();
            let payload = reader
                .read_with_capture(&crate::settings::Capture::default())
                .unwrap()
                .expect("should read image");
            build_item(&store, &payload)
                .unwrap()
                .expect("image should map to item")
        };

        assert_eq!(item.kind, crate::db::models::ClipboardKind::Image);
        assert!(item.content.ends_with(".png"));
        assert!(item.width.unwrap() > 0 && item.height.unwrap() > 0);

        assert!(store.origin_path(&item.content).exists());
        assert!(!store.thumbnail_path(&item.content).exists());

        assert!(store.ensure_thumbnail(&item.content).unwrap().exists());

        let result = upsert_item(&pool, &item).await.unwrap();
        assert!(!result.deduplicated);
        assert_eq!(
            find_item_by_id(&pool, &result.id)
                .await
                .unwrap()
                .unwrap()
                .kind,
            crate::db::models::ClipboardKind::Image
        );
    }

    struct TempDir(std::path::PathBuf);
    impl TempDir {
        fn new() -> Self {
            let p = std::env::temp_dir()
                .join(format!("ultra-clipboard-watcher-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&p).unwrap();
            Self(p)
        }
        fn path(&self) -> &std::path::Path {
            &self.0
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).ok();
        }
    }
}
