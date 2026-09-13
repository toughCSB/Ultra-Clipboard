//! Resolves the environment-specific data directories and validates custom storage locations.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::core::Result;

const DB_DIR: &str = "db";

const RESOURCES_DIR: &str = "resources";

const CONFIG_DIR: &str = "config";

const STATE_DIR: &str = "state";

const STORAGE_MANIFEST_FILENAME: &str = "storage.json";

const STORAGE_IDENTITY_FILENAME: &str = ".ultra-clipboard-storage.json";

const CUSTOM_DATA_DIR_NAME: &str = "UltraClipboardData";

const STORAGE_MANIFEST_VERSION: u16 = 1;

const DEV_ENV_DIR: &str = "dev";

const PROD_ENV_DIR: &str = "prod";

const fn env_dir() -> &'static str {
    if cfg!(dev) {
        DEV_ENV_DIR
    } else {
        PROD_ENV_DIR
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageLocation {
    pub current_path: String,
    pub default_path: String,
    pub is_custom: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StorageManifest {
    version: u16,
    environment: String,
    data_dir: PathBuf,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StorageIdentity {
    version: u16,
    environment: String,
    created_at: DateTime<Utc>,
}

pub fn bootstrap_dir(app: &AppHandle) -> Result<PathBuf> {
    let dir = app
        .path()
        .app_local_data_dir()
        .context("failed to resolve app local data dir")?;
    Ok(dir.join(env_dir()))
}

pub fn default_data_dir(app: &AppHandle) -> Result<PathBuf> {
    bootstrap_dir(app)
}

pub fn custom_data_dir(parent: &Path) -> PathBuf {
    parent.join(CUSTOM_DATA_DIR_NAME).join(env_dir())
}

pub fn storage_location(app: &AppHandle) -> Result<StorageLocation> {
    let current = app_data_dir(app)?;
    let default = default_data_dir(app)?;
    Ok(StorageLocation {
        is_custom: current != default,
        current_path: current.to_string_lossy().into_owned(),
        default_path: default.to_string_lossy().into_owned(),
    })
}

pub fn app_data_dir(app: &AppHandle) -> Result<PathBuf> {
    let bootstrap = bootstrap_dir(app)?;
    let default = default_data_dir(app)?;
    let manifest_path = storage_manifest_path(&bootstrap);

    fs::create_dir_all(&bootstrap)
        .with_context(|| format!("failed to create bootstrap dir at {bootstrap:?}"))?;

    let manifest = match read_storage_manifest(&manifest_path) {
        Ok(Some(manifest)) => manifest,
        Ok(None) => {
            let manifest = storage_manifest(default.clone());
            write_storage_manifest(&manifest_path, &manifest)?;
            manifest
        }
        Err(err) => {
            log::warn!("storage manifest unreadable, using default data dir: {err}");
            let manifest = storage_manifest(default.clone());
            write_storage_manifest(&manifest_path, &manifest)?;
            manifest
        }
    };

    if manifest.environment != env_dir() || manifest.version != STORAGE_MANIFEST_VERSION {
        log::warn!("storage manifest metadata mismatch, using default data dir");
        let manifest = storage_manifest(default.clone());
        write_storage_manifest(&manifest_path, &manifest)?;
        return Ok(default);
    }

    if !manifest.data_dir.exists() {
        log::warn!(
            "storage data dir {:?} is missing, falling back to default data dir",
            manifest.data_dir
        );
        let manifest = storage_manifest(default.clone());
        write_storage_manifest(&manifest_path, &manifest)?;
        return Ok(default);
    }

    if manifest.data_dir != default {
        match has_valid_storage_identity(&manifest.data_dir) {
            Ok(true) => {}
            Ok(false) => {
                log::warn!(
                    "storage data dir {:?} identity mismatch, falling back to default data dir",
                    manifest.data_dir
                );
                let manifest = storage_manifest(default.clone());
                write_storage_manifest(&manifest_path, &manifest)?;
                return Ok(default);
            }
            Err(err) => {
                log::warn!(
                    "storage data dir {:?} identity unreadable, falling back to default data dir: {err}",
                    manifest.data_dir
                );
                let manifest = storage_manifest(default.clone());
                write_storage_manifest(&manifest_path, &manifest)?;
                return Ok(default);
            }
        }
    }

    Ok(manifest.data_dir)
}

pub fn set_app_data_dir(app: &AppHandle, data_dir: PathBuf) -> Result<()> {
    let bootstrap = bootstrap_dir(app)?;
    fs::create_dir_all(&bootstrap)
        .with_context(|| format!("failed to create bootstrap dir at {bootstrap:?}"))?;

    write_storage_identity(&data_dir)?;
    write_storage_manifest(
        &storage_manifest_path(&bootstrap),
        &storage_manifest(data_dir),
    )
}

pub fn write_storage_identity(data_dir: &Path) -> Result<()> {
    fs::create_dir_all(data_dir)
        .with_context(|| format!("failed to create storage dir at {data_dir:?}"))?;
    let identity = StorageIdentity {
        version: STORAGE_MANIFEST_VERSION,
        environment: env_dir().to_owned(),
        created_at: Utc::now(),
    };
    let path = data_dir.join(STORAGE_IDENTITY_FILENAME);
    let json =
        serde_json::to_string_pretty(&identity).context("failed to serialize storage identity")?;

    fs::write(&path, json).with_context(|| format!("failed to write storage identity {path:?}"))?;
    Ok(())
}

pub fn validate_storage_target(data_dir: &Path) -> Result<()> {
    let identity_path = data_dir.join(STORAGE_IDENTITY_FILENAME);
    if identity_path.exists() {
        let content = fs::read_to_string(&identity_path)
            .with_context(|| format!("failed to read storage identity {identity_path:?}"))?;
        let identity: StorageIdentity =
            serde_json::from_str(&content).context("failed to parse storage identity")?;
        if identity.version == STORAGE_MANIFEST_VERSION && identity.environment == env_dir() {
            return Ok(());
        }

        return Err(anyhow::anyhow!(
            "대상 폴더는 현재 환경의 Ultra Clipboard 데이터 폴더가 아닙니다"
        )
        .into());
    }

    if data_dir.exists()
        && fs::read_dir(data_dir)
            .with_context(|| format!("failed to read storage target {data_dir:?}"))?
            .next()
            .is_some()
    {
        return Err(anyhow::anyhow!(
            "대상 Ultra Clipboard 데이터 폴더가 이미 존재하며 올바른 데이터 폴더가 아닙니다"
        )
        .into());
    }

    Ok(())
}

pub fn db_dir(app: &AppHandle) -> Result<PathBuf> {
    Ok(app_data_dir(app)?.join(DB_DIR))
}

pub fn resources_dir(app: &AppHandle) -> Result<PathBuf> {
    Ok(app_data_dir(app)?.join(RESOURCES_DIR))
}

pub fn config_dir(app: &AppHandle) -> Result<PathBuf> {
    Ok(app_data_dir(app)?.join(CONFIG_DIR))
}

pub fn state_dir(app: &AppHandle) -> Result<PathBuf> {
    Ok(app_data_dir(app)?.join(STATE_DIR))
}

fn storage_manifest_path(bootstrap: &Path) -> PathBuf {
    bootstrap.join(STORAGE_MANIFEST_FILENAME)
}

fn storage_manifest(data_dir: PathBuf) -> StorageManifest {
    StorageManifest {
        version: STORAGE_MANIFEST_VERSION,
        environment: env_dir().to_owned(),
        data_dir,
        updated_at: Utc::now(),
    }
}

fn read_storage_manifest(path: &Path) -> Result<Option<StorageManifest>> {
    if !path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(path).with_context(|| format!("failed to read {path:?}"))?;
    Ok(Some(
        serde_json::from_str(&content).with_context(|| format!("failed to parse {path:?}"))?,
    ))
}

fn write_storage_manifest(path: &Path, manifest: &StorageManifest) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create manifest dir at {parent:?}"))?;
    }

    let json =
        serde_json::to_string_pretty(manifest).context("failed to serialize storage manifest")?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json).with_context(|| format!("failed to write {tmp:?}"))?;
    fs::rename(&tmp, path).with_context(|| format!("failed to promote {tmp:?} to {path:?}"))?;
    Ok(())
}

fn has_valid_storage_identity(data_dir: &Path) -> Result<bool> {
    let identity_path = data_dir.join(STORAGE_IDENTITY_FILENAME);
    if !identity_path.exists() {
        return Ok(false);
    }

    let content = fs::read_to_string(&identity_path)
        .with_context(|| format!("failed to read storage identity {identity_path:?}"))?;
    let identity: StorageIdentity =
        serde_json::from_str(&content).context("failed to parse storage identity")?;
    Ok(identity.version == STORAGE_MANIFEST_VERSION && identity.environment == env_dir())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "ultra-clipboard-storage-paths-{}",
                uuid::Uuid::new_v4()
            ));
            fs::create_dir_all(&path).unwrap();

            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).ok();
        }
    }

    #[test]
    fn empty_storage_target_is_allowed() {
        let temp = TempDir::new();

        validate_storage_target(temp.path()).unwrap();
    }

    #[test]
    fn custom_data_dir_uses_named_data_container() {
        let temp = TempDir::new();

        assert_eq!(
            custom_data_dir(temp.path()),
            temp.path().join("UltraClipboardData").join(env_dir())
        );
    }

    #[test]
    fn storage_target_with_matching_identity_is_allowed() {
        let temp = TempDir::new();

        write_storage_identity(temp.path()).unwrap();

        assert!(temp.path().join(".ultra-clipboard-storage.json").exists());
        validate_storage_target(temp.path()).unwrap();
    }

    #[test]
    fn storage_target_with_mismatched_identity_is_rejected() {
        let temp = TempDir::new();
        let identity = StorageIdentity {
            version: STORAGE_MANIFEST_VERSION,
            environment: "other".to_owned(),
            created_at: Utc::now(),
        };
        fs::write(
            temp.path().join(STORAGE_IDENTITY_FILENAME),
            serde_json::to_string_pretty(&identity).unwrap(),
        )
        .unwrap();

        assert!(validate_storage_target(temp.path()).is_err());
    }

    #[test]
    fn non_empty_storage_target_without_identity_is_rejected() {
        let temp = TempDir::new();
        fs::write(temp.path().join("random.txt"), "not Ultra Clipboard").unwrap();

        assert!(validate_storage_target(temp.path()).is_err());
    }
}
