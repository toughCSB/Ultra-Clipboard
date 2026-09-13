use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use anyhow::Context;
use blake3::Hasher;
use tauri::AppHandle;

use crate::core::Result;

const FILE_ICONS_DIR: &str = "file-icons";

#[derive(Clone)]
pub struct FileIconStore {
    root: Arc<RwLock<PathBuf>>,
}

impl FileIconStore {
    pub fn new(app: &AppHandle) -> Result<Self> {
        Ok(Self {
            root: Arc::new(RwLock::new(
                crate::core::paths::resources_dir(app)?.join(FILE_ICONS_DIR),
            )),
        })
    }

    pub fn rebase(&self, app: &AppHandle) -> Result<()> {
        let next = crate::core::paths::resources_dir(app)?.join(FILE_ICONS_DIR);
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
            .with_context(|| format!("failed to create file-icons dir {parent:?}"))?;
    }
    std::fs::write(path, bytes).with_context(|| format!("failed to write file icon {path:?}"))?;
    Ok(())
}
