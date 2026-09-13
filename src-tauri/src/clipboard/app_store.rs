use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use anyhow::Context;
use blake3::Hasher;
use tauri::AppHandle;

use crate::core::Result;

const APP_ICONS_DIR: &str = "app-icons";

#[derive(Clone)]
pub struct AppIconStore {
    root: Arc<RwLock<PathBuf>>,
}

impl AppIconStore {
    pub fn new(app: &AppHandle) -> Result<Self> {
        Ok(Self {
            root: Arc::new(RwLock::new(
                crate::core::paths::resources_dir(app)?.join(APP_ICONS_DIR),
            )),
        })
    }

    #[cfg(test)]
    pub(crate) fn for_test(root: PathBuf) -> Self {
        Self {
            root: Arc::new(RwLock::new(root)),
        }
    }

    pub fn rebase(&self, app: &AppHandle) -> Result<()> {
        let next = crate::core::paths::resources_dir(app)?.join(APP_ICONS_DIR);
        *self
            .root
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = next;
        Ok(())
    }

    pub fn store(&self, png_bytes: &[u8]) -> Result<String> {
        let digest = blake3_hex(png_bytes);
        let file_name = format!("{digest}.png");
        write_if_absent(&self.root().join(&file_name), png_bytes)?;
        Ok(file_name)
    }

    pub fn icon_path(&self, file_name: &str) -> PathBuf {
        self.root().join(file_name)
    }

    fn root(&self) -> PathBuf {
        self.root
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }
}

fn blake3_hex(bytes: &[u8]) -> String {
    let mut hasher = Hasher::new();
    hasher.update(bytes);
    hasher.finalize().to_hex().to_string()
}

fn write_if_absent(path: &Path, bytes: &[u8]) -> Result<()> {
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create app-icons dir {parent:?}"))?;
    }
    std::fs::write(path, bytes).with_context(|| format!("failed to write app icon {path:?}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store() -> (TempDir, AppIconStore) {
        let dir = TempDir::new();
        let store = AppIconStore::for_test(dir.path().join("app-icons"));
        (dir, store)
    }

    #[test]
    fn stores_flat_under_root_and_is_idempotent() {
        let (_d, store) = temp_store();
        let bytes = b"fake-png-bytes-for-test".to_vec();

        let a = store.store(&bytes).unwrap();
        let b = store.store(&bytes).unwrap();
        assert_eq!(a, b, "same bytes -> same file name");
        assert!(a.ends_with(".png"));

        let path = store.icon_path(&a);
        assert!(path.exists());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);

        assert_eq!(path, store.root().join(&a));
    }

    struct TempDir(PathBuf);
    impl TempDir {
        fn new() -> Self {
            let p = std::env::temp_dir()
                .join(format!("ultra-clipboard-appicon-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&p).unwrap();
            Self(p)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).ok();
        }
    }
}
