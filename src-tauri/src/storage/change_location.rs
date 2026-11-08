use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::Context;
use tauri::{AppHandle, Manager};

use crate::core::Result;
use crate::db::DatabaseState;
use crate::settings::change::{self, SettingsWrite};

const STORAGE_CONTENT_DIRS: [&str; 4] = ["db", "resources", "config", "state"];
const CUSTOM_STORAGE_CONTAINER_DIR: &str = "UltraClipboardData";

pub async fn change_location(app: &AppHandle, db: &DatabaseState, target: PathBuf) -> Result<()> {
    let access = app.state::<super::StorageAccess>();
    let _operation = access.operation().await;
    let current = crate::core::paths::app_data_dir(app)?;
    if current == target {
        return Ok(());
    }

    reject_nested_storage_move(&current, &target)?;
    if target != crate::core::paths::default_data_dir(app)? {
        crate::core::paths::validate_storage_target(&target)?;
    }

    let target_parent = target
        .parent()
        .ok_or_else(|| anyhow::anyhow!("storage target has no parent"))?;
    fs::create_dir_all(target_parent)
        .with_context(|| format!("failed to create storage parent {target_parent:?}"))?;
    let workspace = tempfile::Builder::new()
        .prefix(".ultra-storage-change-")
        .tempdir_in(target_parent)
        .with_context(|| format!("failed to stage storage in {target_parent:?}"))?;
    let staged = workspace.path().join("staged");
    let previous_target = workspace.path().join("previous-target");
    let touched = Mutex::new(Vec::new());
    let install_app = app.clone();
    let install_current = current.clone();
    let install_target = target.clone();
    let recovery_app = app.clone();
    let recovery_current = current.clone();
    let recovery_target = target.clone();
    let result = super::change(
        app,
        db,
        || async {
            copy_storage_data(&install_current, &staged)?;
            replace_target_contents(&staged, &install_target, &previous_target, &touched)?;
            crate::core::paths::set_app_data_dir(&install_app, install_target.clone())?;
            crate::db::init(&install_app).await
        },
        || {
            let app = recovery_app.clone();
            let current = recovery_current.clone();
            let target = recovery_target.clone();
            let staged = &staged;
            let previous_target = &previous_target;
            let touched = &touched;
            async move {
                restore_target_contents(staged, &target, previous_target, touched)?;
                crate::core::paths::set_app_data_dir(&app, current)?;
                rebase_resource_states(&app)?;
                app.state::<crate::settings::SettingsStore>().rebase(&app)?;
                crate::db::init(&app).await
            }
        },
        || async { rebase_storage_states(app).await },
    )
    .await;
    super::finish_workspace(workspace, result, db).await?;

    if let Err(err) = remove_old_storage_data(app, &current) {
        log::warn!("storage location changed, but old data cleanup failed: {err}");
    }
    super::emit_clipboard_imported(app);

    Ok(())
}

fn reject_nested_storage_move(current: &Path, target: &Path) -> Result<()> {
    if target.starts_with(current) || current.starts_with(target) {
        return Err(
            anyhow::anyhow!("기존 데이터 폴더와 새 데이터 폴더는 서로 포함할 수 없습니다").into(),
        );
    }

    Ok(())
}

fn copy_storage_data(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst).with_context(|| format!("failed to create storage dir {dst:?}"))?;
    for name in STORAGE_CONTENT_DIRS {
        replace_path(&src.join(name), &dst.join(name))?;
    }
    crate::core::paths::write_storage_identity(dst)?;
    Ok(())
}

const STORAGE_CONTENT_AND_IDENTITY: [&str; 5] = [
    "db",
    "resources",
    "config",
    "state",
    ".ultra-clipboard-storage.json",
];

fn replace_target_contents(
    staged: &Path,
    target: &Path,
    previous: &Path,
    touched: &Mutex<Vec<&'static str>>,
) -> Result<()> {
    fs::create_dir_all(target).with_context(|| format!("failed to create {target:?}"))?;
    fs::create_dir_all(previous).with_context(|| format!("failed to create {previous:?}"))?;
    for name in STORAGE_CONTENT_AND_IDENTITY {
        replace_one_path(staged, target, previous, touched, name)?;
    }
    Ok(())
}

fn replace_one_path(
    staged: &Path,
    target: &Path,
    previous: &Path,
    touched: &Mutex<Vec<&'static str>>,
    name: &'static str,
) -> Result<()> {
    let incoming = staged.join(name);
    let destination = target.join(name);
    if !incoming.exists() && !destination.exists() {
        return Ok(());
    }
    if destination.exists() {
        fs::rename(&destination, previous.join(name))
            .with_context(|| format!("failed to preserve target {destination:?}"))?;
    }
    touched.lock().expect("storage journal poisoned").push(name);
    if incoming.exists() {
        fs::rename(&incoming, &destination)
            .with_context(|| format!("failed to install storage content {incoming:?}"))?;
    }
    Ok(())
}

fn restore_target_contents(
    staged: &Path,
    target: &Path,
    previous: &Path,
    touched: &Mutex<Vec<&'static str>>,
) -> Result<()> {
    let names = touched.lock().expect("storage journal poisoned").clone();
    for name in names.into_iter().rev() {
        let incoming = staged.join(name);
        let prior = previous.join(name);
        let destination = target.join(name);
        if prior.exists() {
            if destination.exists() {
                remove_path(&destination)?;
            }
            fs::rename(&prior, &destination)
                .with_context(|| format!("failed to restore target {destination:?}"))?;
        } else if !incoming.exists() && destination.exists() {
            remove_path(&destination)?;
        }
    }
    Ok(())
}

fn remove_old_storage_data(app: &AppHandle, old: &Path) -> Result<()> {
    let default = crate::core::paths::default_data_dir(app)?;
    if old == default {
        remove_bootstrap_storage_payload(old)?;
        return Ok(());
    }

    remove_custom_storage_root(old)?;

    Ok(())
}

pub(crate) fn remove_custom_storage_root(old: &Path) -> Result<()> {
    if old.exists() {
        remove_known_storage_content(old)?;
        match fs::remove_dir(old) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::DirectoryNotEmpty => return Ok(()),
            Err(err) => {
                return Err(anyhow::anyhow!("failed to remove old data dir {old:?}: {err}").into());
            }
        }
    }

    let Some(container) = old.parent() else {
        return Ok(());
    };
    if container.file_name().and_then(|name| name.to_str()) != Some(CUSTOM_STORAGE_CONTAINER_DIR) {
        return Ok(());
    }

    match fs::remove_dir(container) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::DirectoryNotEmpty => Ok(()),
        Err(err) => {
            Err(anyhow::anyhow!("failed to remove old data container {container:?}: {err}").into())
        }
    }
}

pub(crate) fn remove_bootstrap_storage_payload(dir: &Path) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }

    remove_known_storage_content(dir)
}

fn remove_known_storage_content(dir: &Path) -> Result<()> {
    for name in STORAGE_CONTENT_AND_IDENTITY {
        let path = dir.join(name);
        if path.exists() {
            remove_path(&path)?;
        }
    }
    Ok(())
}

fn replace_path(src: &Path, dst: &Path) -> Result<()> {
    if dst.exists() {
        remove_path(dst)?;
    }

    if !src.exists() {
        return Ok(());
    }

    if src.is_dir() {
        copy_dir_all(src, dst)?;
        return Ok(());
    }

    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create parent dir {parent:?}"))?;
    }
    fs::copy(src, dst).with_context(|| format!("failed to copy {src:?} to {dst:?}"))?;
    Ok(())
}

fn remove_path(path: &Path) -> Result<()> {
    if path.is_dir() {
        fs::remove_dir_all(path).with_context(|| format!("failed to remove {path:?}"))?;
        return Ok(());
    }

    fs::remove_file(path).with_context(|| format!("failed to remove {path:?}"))?;
    Ok(())
}

pub(crate) fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst).with_context(|| format!("failed to create dir {dst:?}"))?;
    for entry in fs::read_dir(src).with_context(|| format!("failed to read dir {src:?}"))? {
        let entry = entry.with_context(|| format!("failed to read entry under {src:?}"))?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        let metadata = entry
            .metadata()
            .with_context(|| format!("failed to read metadata at {src_path:?}"))?;

        if metadata.is_dir() {
            copy_dir_all(&src_path, &dst_path)?;
            continue;
        }

        fs::copy(&src_path, &dst_path)
            .with_context(|| format!("failed to copy {src_path:?} to {dst_path:?}"))?;
    }
    Ok(())
}

async fn rebase_storage_states(app: &AppHandle) -> Result<()> {
    rebase_resource_states(app)?;

    if let Some(registry) = app.try_state::<crate::clipboard::AppsRegistry>() {
        registry.load_from_db().await?;
    }

    // Settings go last: the sync effect reads the rebased image store and pool.
    change::commit_storage_swap(app, SettingsWrite::Rebase).await?;

    Ok(())
}

fn rebase_resource_states(app: &AppHandle) -> Result<()> {
    app.state::<crate::window::WindowStateStore>().rebase(app)?;
    app.state::<crate::clipboard::ImageStore>().rebase(app)?;
    app.state::<crate::clipboard::AppIconStore>().rebase(app)?;
    app.state::<crate::clipboard::FileIconStore>().rebase(app)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_staging_does_not_remove_existing_target() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("target");
        fs::create_dir_all(target.join("db")).unwrap();
        fs::write(target.join("db").join("clipboard.db"), b"original").unwrap();
        let staged = temp.path().join("staged");
        fs::create_dir_all(&staged).unwrap();

        restore_target_contents(
            &staged,
            &target,
            &temp.path().join("previous"),
            &Mutex::new(Vec::new()),
        )
        .unwrap();

        assert_eq!(
            fs::read(target.join("db").join("clipboard.db")).unwrap(),
            b"original"
        );
    }

    #[test]
    fn failed_install_restores_previous_target_and_manifest() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("target");
        let staged = temp.path().join("staged");
        let previous = temp.path().join("previous");
        fs::create_dir_all(target.join("db")).unwrap();
        fs::create_dir_all(staged.join("db")).unwrap();
        fs::write(target.join("storage.json"), b"bootstrap").unwrap();
        fs::write(target.join("db").join("clipboard.db"), b"old").unwrap();
        fs::write(staged.join("db").join("clipboard.db"), b"new").unwrap();

        let touched = Mutex::new(Vec::new());
        replace_target_contents(&staged, &target, &previous, &touched).unwrap();
        assert_eq!(
            fs::read(target.join("db").join("clipboard.db")).unwrap(),
            b"new"
        );
        restore_target_contents(&staged, &target, &previous, &touched).unwrap();

        assert_eq!(
            fs::read(target.join("db").join("clipboard.db")).unwrap(),
            b"old"
        );
        assert_eq!(fs::read(target.join("storage.json")).unwrap(), b"bootstrap");
    }

    #[test]
    fn partial_install_leaves_untouched_target_state_intact() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("target");
        let staged = temp.path().join("staged");
        let previous = temp.path().join("previous");
        fs::create_dir_all(target.join("db")).unwrap();
        fs::create_dir_all(target.join("state")).unwrap();
        fs::create_dir_all(staged.join("db")).unwrap();
        fs::create_dir_all(&previous).unwrap();
        fs::write(target.join("db").join("clipboard.db"), b"old").unwrap();
        fs::write(target.join("state").join("window.json"), b"untouched").unwrap();
        fs::write(staged.join("db").join("clipboard.db"), b"new").unwrap();
        let touched = Mutex::new(Vec::new());

        replace_one_path(&staged, &target, &previous, &touched, "db").unwrap();
        restore_target_contents(&staged, &target, &previous, &touched).unwrap();

        assert_eq!(
            fs::read(target.join("db").join("clipboard.db")).unwrap(),
            b"old"
        );
        assert_eq!(
            fs::read(target.join("state").join("window.json")).unwrap(),
            b"untouched"
        );
    }

    #[test]
    fn failed_preserve_does_not_mark_or_delete_existing_target() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("target");
        let staged = temp.path().join("staged");
        let previous = temp.path().join("previous");
        fs::create_dir_all(&target).unwrap();
        fs::create_dir_all(&staged).unwrap();
        fs::create_dir_all(previous.join("state")).unwrap();
        fs::write(target.join("state"), b"original").unwrap();
        let touched = Mutex::new(Vec::new());

        assert!(replace_one_path(&staged, &target, &previous, &touched, "state").is_err());
        restore_target_contents(&staged, &target, &previous, &touched).unwrap();

        assert_eq!(fs::read(target.join("state")).unwrap(), b"original");
        assert!(touched.lock().unwrap().is_empty());
    }

    #[test]
    fn bootstrap_cleanup_preserves_unknown_entries() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir_all(temp.path().join("db")).unwrap();
        fs::create_dir_all(temp.path().join("user-notes")).unwrap();
        fs::write(temp.path().join("db").join("clipboard.db"), b"old").unwrap();
        fs::write(temp.path().join("storage.json"), b"manifest").unwrap();
        fs::write(temp.path().join("user-notes").join("keep.txt"), b"keep").unwrap();

        remove_bootstrap_storage_payload(temp.path()).unwrap();

        assert!(!temp.path().join("db").exists());
        assert_eq!(
            fs::read(temp.path().join("storage.json")).unwrap(),
            b"manifest"
        );
        assert_eq!(
            fs::read(temp.path().join("user-notes").join("keep.txt")).unwrap(),
            b"keep"
        );
    }

    #[test]
    fn custom_cleanup_preserves_unknown_entries_and_container() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("UltraClipboardData").join("dev");
        fs::create_dir_all(root.join("resources")).unwrap();
        fs::write(root.join("resources").join("old.png"), b"old").unwrap();
        fs::write(root.join("keep.txt"), b"unrelated").unwrap();

        remove_custom_storage_root(&root).unwrap();

        assert!(!root.join("resources").exists());
        assert_eq!(fs::read(root.join("keep.txt")).unwrap(), b"unrelated");
        assert!(root.parent().unwrap().exists());
    }
}
