use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context};
use tauri::{AppHandle, Manager};

use crate::core::Result;
use crate::db::DatabaseState;
use crate::settings::change::{self, SettingsWrite};

fn copy_file_to(src: &Path, dst: &Path) -> Result<()> {
    let parent = dst
        .parent()
        .ok_or_else(|| anyhow!("target path has no parent"))?;
    fs::create_dir_all(parent).with_context(|| format!("failed to create {parent:?}"))?;
    fs::copy(src, dst).with_context(|| format!("failed to copy {src:?} to {dst:?}"))?;
    Ok(())
}

pub async fn overwrite(
    app: &AppHandle,
    db: &DatabaseState,
    db_src: &Path,
    resources_src: &Path,
    settings_path: &Path,
) -> Result<()> {
    let access = app.state::<super::StorageAccess>();
    let _operation = access.operation().await;
    let settings_bytes = fs::read(settings_path)
        .with_context(|| format!("failed to read backup settings {settings_path:?}"))?;
    let mut proposed: crate::settings::Settings = serde_json::from_slice(&settings_bytes)
        .with_context(|| format!("invalid backup settings at {settings_path:?}"))?;
    proposed.sync = app
        .state::<crate::settings::SettingsStore>()
        .snapshot()
        .sync;
    crate::settings::validate_settings(&proposed)?;
    let source_pool = crate::backup::open_backup_db(db_src).await?;
    let integrity: String = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_one(&source_pool)
        .await
        .context("failed to inspect backup database integrity")?;
    if integrity != "ok" {
        return Err(anyhow!("backup database integrity check failed: {integrity}").into());
    }
    let _: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM clipboard_items")
        .fetch_one(&source_pool)
        .await
        .context("backup database has no readable clipboard history")?;
    source_pool.close().await;

    let db_live = crate::db::db_path(app)?;
    let live_resources = crate::core::paths::resources_dir(app)?;
    let workspace = tempfile::Builder::new()
        .prefix(".ultra-backup-overwrite-")
        .tempdir_in(
            db_live
                .parent()
                .ok_or_else(|| anyhow!("database has no parent"))?,
        )
        .context("failed to stage backup near live database")?;
    let incoming_db = workspace.path().join("incoming.db");
    copy_file_to(db_src, &incoming_db)?;
    let incoming_resources = workspace.path().join("incoming-resources");
    let has_resources = resources_src.exists();
    if has_resources {
        super::change_location::copy_dir_all(resources_src, &incoming_resources)?;
    }
    let previous_db = workspace.path().join("previous.db");
    let previous_resources = workspace.path().join("previous-resources");
    let old_db_ready = std::sync::atomic::AtomicBool::new(false);
    let install_app = app.clone();
    let recovery_app = app.clone();
    let outcome = super::change(
        app,
        db,
        || async {
            copy_database_files(&db_live, &previous_db)?;
            old_db_ready.store(true, std::sync::atomic::Ordering::Release);
            preserve_resources(&live_resources, &previous_resources, has_resources)?;
            replace_live_database(&install_app, &incoming_db)?;
            install_resources(&incoming_resources, &live_resources, has_resources)?;
            crate::db::init(&install_app).await
        },
        || {
            let app = recovery_app.clone();
            let old_db_ready = &old_db_ready;
            let previous_db = &previous_db;
            let db_live = &db_live;
            let incoming_resources = &incoming_resources;
            let live_resources = &live_resources;
            let previous_resources = &previous_resources;
            async move {
                if old_db_ready.load(std::sync::atomic::Ordering::Acquire) {
                    restore_database_files(previous_db, db_live)?;
                }
                restore_resources(
                    incoming_resources,
                    live_resources,
                    previous_resources,
                    has_resources,
                )?;
                crate::db::init(&app).await
            }
        },
        || async {
            refresh_apps_registry(app).await;
            change::commit_storage_swap(app, SettingsWrite::ReplaceFromFile(settings_path)).await?;
            Ok(())
        },
    )
    .await;
    super::finish_workspace(workspace, outcome, db).await?;
    super::emit_clipboard_imported(app);

    Ok(())
}

fn preserve_resources(live: &Path, previous: &Path, has_incoming: bool) -> Result<()> {
    if has_incoming && live.exists() {
        fs::rename(live, previous).context("failed to preserve existing resources")?;
    }
    Ok(())
}

fn install_resources(incoming: &Path, live: &Path, has_incoming: bool) -> Result<()> {
    if has_incoming {
        fs::rename(incoming, live).context("failed to install imported resources")?;
    }
    Ok(())
}

fn restore_resources(
    incoming: &Path,
    live: &Path,
    previous: &Path,
    has_incoming: bool,
) -> Result<()> {
    if !has_incoming {
        return Ok(());
    }
    if previous.exists() {
        if live.exists() {
            fs::remove_dir_all(live).context("failed to remove imported resources for recovery")?;
        }
        fs::rename(previous, live).context("failed to restore previous resources")?;
    } else if !incoming.exists() && live.exists() {
        fs::remove_dir_all(live).context("failed to remove imported resources")?;
    }
    Ok(())
}

fn copy_database_files(src: &Path, backup: &Path) -> Result<()> {
    copy_file_to(src, backup)?;
    for suffix in ["-wal", "-shm"] {
        let sidecar = PathBuf::from(format!("{}{suffix}", src.display()));
        if sidecar.exists() {
            copy_file_to(
                &sidecar,
                &PathBuf::from(format!("{}{suffix}", backup.display())),
            )?;
        }
    }
    Ok(())
}

fn restore_database_files(backup: &Path, dst: &Path) -> Result<()> {
    copy_file_to(backup, dst)?;
    for suffix in ["-wal", "-shm"] {
        let sidecar = PathBuf::from(format!("{}{suffix}", dst.display()));
        let saved = PathBuf::from(format!("{}{suffix}", backup.display()));
        if saved.exists() {
            copy_file_to(&saved, &sidecar)?;
        } else if sidecar.exists() {
            fs::remove_file(&sidecar)
                .with_context(|| format!("failed to remove imported sidecar {sidecar:?}"))?;
        }
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_finalization_restores_original_resources() {
        let temp = tempfile::tempdir().unwrap();
        let live = temp.path().join("resources");
        let incoming = temp.path().join("incoming");
        let previous = temp.path().join("previous");
        fs::create_dir_all(&live).unwrap();
        fs::create_dir_all(&incoming).unwrap();
        fs::write(live.join("image.png"), b"original").unwrap();
        fs::write(incoming.join("image.png"), b"imported").unwrap();

        preserve_resources(&live, &previous, true).unwrap();
        install_resources(&incoming, &live, true).unwrap();
        assert_eq!(fs::read(live.join("image.png")).unwrap(), b"imported");
        restore_resources(&incoming, &live, &previous, true).unwrap();

        assert_eq!(fs::read(live.join("image.png")).unwrap(), b"original");
        assert!(!previous.exists());
    }

    #[test]
    fn failure_between_preserve_and_install_restores_original_resources() {
        let temp = tempfile::tempdir().unwrap();
        let live = temp.path().join("resources");
        let incoming = temp.path().join("incoming");
        let previous = temp.path().join("previous");
        fs::create_dir_all(&live).unwrap();
        fs::create_dir_all(&incoming).unwrap();
        fs::write(live.join("image.png"), b"original").unwrap();

        preserve_resources(&live, &previous, true).unwrap();
        restore_resources(&incoming, &live, &previous, true).unwrap();

        assert_eq!(fs::read(live.join("image.png")).unwrap(), b"original");
    }

    #[test]
    fn absent_incoming_resources_leave_live_directory_untouched() {
        let temp = tempfile::tempdir().unwrap();
        let live = temp.path().join("resources");
        let incoming = temp.path().join("missing");
        let previous = temp.path().join("previous");
        fs::create_dir_all(&live).unwrap();
        fs::write(live.join("image.png"), b"original").unwrap();

        preserve_resources(&live, &previous, false).unwrap();
        install_resources(&incoming, &live, false).unwrap();
        restore_resources(&incoming, &live, &previous, false).unwrap();

        assert_eq!(fs::read(live.join("image.png")).unwrap(), b"original");
    }
}
