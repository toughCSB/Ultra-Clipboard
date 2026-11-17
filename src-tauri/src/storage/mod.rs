//! Coordinates live data changes with resource writers and clipboard sync.

use std::future::Future;
use std::sync::Arc;

use sqlx::SqlitePool;
use tauri::{AppHandle, Manager};
use tokio::sync::{Mutex, MutexGuard, OwnedRwLockReadGuard, OwnedRwLockWriteGuard, RwLock};

use crate::core::Result;
use crate::db::DatabaseState;

pub(crate) mod change_location;
pub(crate) mod overwrite;
pub(crate) mod resources;

#[derive(Clone, Default)]
pub struct StorageAccess {
    operation: Arc<Mutex<()>>,
    data: Arc<RwLock<()>>,
}

impl StorageAccess {
    pub async fn operation(&self) -> MutexGuard<'_, ()> {
        self.operation.lock().await
    }

    pub async fn read_owned(&self) -> OwnedRwLockReadGuard<()> {
        self.data.clone().read_owned().await
    }

    pub async fn exclusive_owned(&self) -> OwnedRwLockWriteGuard<()> {
        self.data.clone().write_owned().await
    }
}

struct WatcherPauseRestore {
    pause: Option<crate::clipboard::WatcherPause>,
    previous: bool,
}

impl WatcherPauseRestore {
    fn new(app: &AppHandle) -> Self {
        let pause = app.try_state::<crate::clipboard::WatcherPause>();
        let previous = pause.as_ref().is_some_and(|state| state.is_paused());
        if let Some(state) = pause.as_ref() {
            state.set_paused(true);
        }

        Self {
            pause: pause.map(|state| state.inner().clone()),
            previous,
        }
    }
}

impl Drop for WatcherPauseRestore {
    fn drop(&mut self) {
        if let Some(pause) = &self.pause {
            pause.set_paused(self.previous);
        }
    }
}

/// Runs the irreversible part of a live data change while writers are quiesced.
/// A failed install must reopen the previous database. Finalize executes before
/// sync is restarted; it must leave recovery files available on error.
pub async fn change<F, Fut, R, RFut, V, VFut>(
    app: &AppHandle,
    db: &DatabaseState,
    install: F,
    mut recover: R,
    finalize: V,
) -> Result<()>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<SqlitePool>>,
    R: FnMut() -> RFut,
    RFut: Future<Output = Result<SqlitePool>>,
    V: FnOnce() -> VFut,
    VFut: Future<Output = Result<()>>,
{
    let access = app.state::<StorageAccess>();
    let _watcher_pause = WatcherPauseRestore::new(app);
    if let Some(sync) = app.try_state::<crate::sync::SyncRuntime>() {
        sync.stop().await;
    }

    let exclusive = access.exclusive_owned().await;
    let outcome = db
        .replace_with_finalize(install, &mut recover, finalize)
        .await;

    if outcome.is_err() {
        if let Some(registry) = app.try_state::<crate::clipboard::AppsRegistry>() {
            if let Err(err) = registry.load_from_db().await {
                log::warn!("reload app registry after storage recovery failed: {err}");
            }
        }
    }
    drop(exclusive);

    if app.try_state::<crate::sync::SyncRuntime>().is_some() {
        if let Err(err) = crate::sync::reconfigure(app).await {
            log::warn!("reconfigure sync after data change failed: {err}");
        }
    }

    outcome
}

pub fn emit_clipboard_imported(app: &AppHandle) {
    crate::clipboard::emit_clipboard_updated(
        app,
        crate::clipboard::ClipboardUpdated::Imported { imported: true },
    );
}

/// A failed rollback must leave its original files available for manual repair.
pub async fn finish_workspace(
    workspace: tempfile::TempDir,
    outcome: Result<()>,
    db: &DatabaseState,
) -> Result<()> {
    match outcome {
        Err(err) if db.pool().await.is_closed() => {
            let kept = workspace.keep();
            Err(anyhow::anyhow!("{err}; recovery files preserved at {kept:?}").into())
        }
        result => result,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn failed_recovery_retains_original_files() {
        let parent = tempfile::tempdir().unwrap();
        let workspace = tempfile::tempdir_in(parent.path()).unwrap();
        let rollback_path = workspace.path().join("previous.db");
        std::fs::write(&rollback_path, b"preserved").unwrap();
        let db = DatabaseState::new(crate::db::test_support::memory_pool().await);
        let _ = db
            .close_and_replace_with_recovery(
                || async { Err(crate::core::AppError::Clipboard("swap failed".into())) },
                || async { Err(crate::core::AppError::Clipboard("restore failed".into())) },
            )
            .await;

        let error = finish_workspace(
            workspace,
            Err(crate::core::AppError::Clipboard("restore failed".into())),
            &db,
        )
        .await
        .unwrap_err();

        assert!(error.to_string().contains("recovery files preserved"));
        assert_eq!(std::fs::read(&rollback_path).unwrap(), b"preserved");
    }
}
