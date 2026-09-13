use std::path::PathBuf;

use anyhow::Context;
use tauri::AppHandle;

use crate::core::Result;

const DB_FILENAME: &str = "clipboard.db";

pub fn db_path(app: &AppHandle) -> Result<PathBuf> {
    // The database and its WAL/SHM sidecars live in the dedicated `db` directory.
    // `core::paths` keeps development and production data isolated.
    let dir = crate::core::paths::db_dir(app)?;

    std::fs::create_dir_all(&dir).with_context(|| format!("failed to create db dir at {dir:?}"))?;

    Ok(dir.join(DB_FILENAME))
}
