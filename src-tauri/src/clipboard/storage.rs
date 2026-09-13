//! ```text
//! <app_local_data>/resources/clipboard-images/

//! ```

use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use anyhow::Context;
use blake3::Hasher;
use clipboard_rs::common::{RustImage, RustImageData};
use tauri::AppHandle;

use super::payload::ImagePayload;
use crate::core::{AppError, Result};

const THUMBNAIL_MAX: u32 = 300;

const IMAGES_DIR: &str = "clipboard-images";
const ORIGIN_DIR: &str = "origin";
const THUMBNAILS_DIR: &str = "thumbnails";

pub struct StoredImage {
    pub file_name: String,

    #[allow(dead_code)]
    pub content_digest: String,
    pub width: i64,
    pub height: i64,

    pub size: i64,
}

#[derive(Clone)]
pub struct ImageStore {
    images_root: Arc<RwLock<PathBuf>>,
}

impl ImageStore {
    pub fn new(app: &AppHandle) -> Result<Self> {
        let images_root = crate::core::paths::resources_dir(app)?.join(IMAGES_DIR);
        Ok(Self {
            images_root: Arc::new(RwLock::new(images_root)),
        })
    }

    #[cfg(test)]
    pub(crate) fn for_test(images_root: PathBuf) -> Self {
        Self {
            images_root: Arc::new(RwLock::new(images_root)),
        }
    }

    pub fn rebase(&self, app: &AppHandle) -> Result<()> {
        let next = crate::core::paths::resources_dir(app)?.join(IMAGES_DIR);
        *self
            .images_root
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = next;
        Ok(())
    }

    pub fn store(&self, image: &ImagePayload) -> Result<StoredImage> {
        let content_digest = blake3_hex(&image.bytes);
        let file_name = format!("{content_digest}.png");

        let origin_path = self.shard_path(ORIGIN_DIR, &content_digest, &file_name);
        write_if_absent(&origin_path, &image.bytes)?;

        Ok(StoredImage {
            file_name,
            content_digest,
            width: i64::from(image.width),
            height: i64::from(image.height),
            size: image.bytes.len() as i64,
        })
    }

    pub fn ensure_thumbnail(&self, file_name: &str) -> Result<PathBuf> {
        let thumb_path = self.thumbnail_path(file_name);
        if thumb_path.exists() {
            return Ok(thumb_path);
        }

        let origin_path = self.origin_path(file_name);
        let origin_bytes = std::fs::read(&origin_path)
            .with_context(|| format!("failed to read origin image {origin_path:?}"))?;
        let thumb_bytes = encode_thumbnail(&origin_bytes)?;
        write_if_absent(&thumb_path, &thumb_bytes)?;
        Ok(thumb_path)
    }

    pub fn remove(&self, file_name: &str) -> Result<()> {
        let origin = self.origin_path(file_name);
        let thumb = self.thumbnail_path(file_name);
        remove_if_present(&origin)?;
        remove_if_present(&thumb)?;

        remove_dir_if_empty(origin.parent());
        remove_dir_if_empty(thumb.parent());
        Ok(())
    }

    pub fn origin_path(&self, file_name: &str) -> PathBuf {
        self.shard_path(ORIGIN_DIR, shard_key(file_name), file_name)
    }

    pub fn thumbnail_path(&self, file_name: &str) -> PathBuf {
        self.shard_path(THUMBNAILS_DIR, shard_key(file_name), file_name)
    }

    fn shard_path(&self, kind_dir: &str, shard_src: &str, file_name: &str) -> PathBuf {
        self.images_root()
            .join(kind_dir)
            .join(shard_dir(shard_src))
            .join(file_name)
    }

    fn images_root(&self) -> PathBuf {
        self.images_root
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }
}

fn shard_dir(src: &str) -> &str {
    if src.len() >= 2 {
        &src[..2]
    } else {
        "00"
    }
}

fn shard_key(file_name: &str) -> &str {
    file_name.split('.').next().unwrap_or(file_name)
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
            .with_context(|| format!("failed to create image dir {parent:?}"))?;
    }
    std::fs::write(path, bytes).with_context(|| format!("failed to write image {path:?}"))?;
    Ok(())
}

fn remove_if_present(path: &Path) -> Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(AppError::from(
            anyhow::Error::new(err).context(format!("failed to remove image {path:?}")),
        )),
    }
}

fn remove_dir_if_empty(dir: Option<&Path>) {
    if let Some(dir) = dir {
        let _ = std::fs::remove_dir(dir);
    }
}

fn encode_thumbnail(png_bytes: &[u8]) -> Result<Vec<u8>> {
    let image = RustImageData::from_bytes(png_bytes).map_err(clip_err)?;
    let thumb = image
        .thumbnail(THUMBNAIL_MAX, THUMBNAIL_MAX)
        .map_err(clip_err)?;
    Ok(thumb.to_png().map_err(clip_err)?.get_bytes().to_vec())
}

fn clip_err<E: std::fmt::Display>(err: E) -> AppError {
    AppError::Clipboard(err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_png(w: u32, h: u32) -> Vec<u8> {
        use std::io::Cursor;
        let buf = image::RgbaImage::from_pixel(w, h, image::Rgba([10, 20, 30, 255]));
        let mut out = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(buf)
            .write_to(&mut out, image::ImageFormat::Png)
            .unwrap();
        out.into_inner()
    }

    fn temp_store() -> (tempdir_guard::TempDir, ImageStore) {
        let dir = tempdir_guard::TempDir::new();
        let store = ImageStore::for_test(dir.path().join("resources").join("clipboard-images"));
        (dir, store)
    }

    #[test]
    fn stores_origin_under_hash_shard_without_thumbnail() {
        let (_dir, store) = temp_store();
        let payload = ImagePayload {
            bytes: sample_png(64, 48),
            width: 64,
            height: 48,
        };

        let stored = store.store(&payload).unwrap();
        assert!(stored.file_name.ends_with(".png"));
        assert_eq!(stored.file_name, format!("{}.png", stored.content_digest));
        assert_eq!(stored.width, 64);
        assert_eq!(stored.height, 48);
        assert!(stored.size > 0);

        let origin = store.origin_path(&stored.file_name);
        let thumb = store.thumbnail_path(&stored.file_name);
        assert!(origin.exists(), "origin should exist: {origin:?}");
        assert!(
            !thumb.exists(),
            "thumbnail should NOT exist before ensure_thumbnail: {thumb:?}"
        );
        assert_eq!(
            origin.parent().unwrap().file_name().unwrap().to_str(),
            Some(&stored.content_digest[..2])
        );

        assert_eq!(std::fs::read(&origin).unwrap(), payload.bytes);
    }

    #[test]
    fn ensure_thumbnail_generates_then_caches() {
        let (_dir, store) = temp_store();
        let payload = ImagePayload {
            bytes: sample_png(64, 48),
            width: 64,
            height: 48,
        };
        let stored = store.store(&payload).unwrap();

        let thumb = store.ensure_thumbnail(&stored.file_name).unwrap();
        assert!(thumb.exists(), "thumbnail should be generated: {thumb:?}");
        assert_eq!(thumb, store.thumbnail_path(&stored.file_name));
        let first_bytes = std::fs::read(&thumb).unwrap();
        assert!(!first_bytes.is_empty());

        let thumb2 = store.ensure_thumbnail(&stored.file_name).unwrap();
        assert_eq!(thumb, thumb2);
        assert_eq!(std::fs::read(&thumb2).unwrap(), first_bytes);
    }

    #[test]
    fn ensure_thumbnail_errors_when_origin_missing() {
        let (_dir, store) = temp_store();

        let result = store.ensure_thumbnail("0000000000000000.png");
        assert!(result.is_err());
    }

    #[test]
    fn store_is_idempotent_for_same_bytes() {
        let (_dir, store) = temp_store();
        let payload = ImagePayload {
            bytes: sample_png(32, 32),
            width: 32,
            height: 32,
        };

        let a = store.store(&payload).unwrap();
        let b = store.store(&payload).unwrap();

        assert_eq!(a.file_name, b.file_name);
        assert_eq!(a.content_digest, b.content_digest);
    }

    #[test]
    fn remove_deletes_origin_and_thumbnail_idempotently() {
        let (_dir, store) = temp_store();
        let payload = ImagePayload {
            bytes: sample_png(40, 30),
            width: 40,
            height: 30,
        };
        let stored = store.store(&payload).unwrap();
        store.ensure_thumbnail(&stored.file_name).unwrap();

        let origin = store.origin_path(&stored.file_name);
        let thumb = store.thumbnail_path(&stored.file_name);
        assert!(origin.exists() && thumb.exists());

        store.remove(&stored.file_name).unwrap();
        assert!(!origin.exists(), "origin should be removed");
        assert!(!thumb.exists(), "thumbnail should be removed");

        assert!(
            !origin.parent().unwrap().exists(),
            "empty origin shard dir should be removed"
        );
        assert!(
            !thumb.parent().unwrap().exists(),
            "empty thumbnail shard dir should be removed"
        );

        store.remove(&stored.file_name).unwrap();
    }

    #[test]
    fn remove_keeps_shard_dir_when_other_image_shares_prefix() {
        let (_dir, store) = temp_store();
        let payload = ImagePayload {
            bytes: sample_png(40, 30),
            width: 40,
            height: 30,
        };
        let stored = store.store(&payload).unwrap();
        let shard = store
            .origin_path(&stored.file_name)
            .parent()
            .unwrap()
            .to_path_buf();

        let sibling = shard.join("sibling.png");
        std::fs::write(&sibling, b"x").unwrap();

        store.remove(&stored.file_name).unwrap();

        assert!(!store.origin_path(&stored.file_name).exists());
        assert!(shard.exists(), "non-empty shard dir must be kept");
        assert!(sibling.exists(), "sibling image must survive");
    }

    #[test]
    fn remove_succeeds_when_thumbnail_never_generated() {
        let (_dir, store) = temp_store();
        let payload = ImagePayload {
            bytes: sample_png(16, 16),
            width: 16,
            height: 16,
        };
        let stored = store.store(&payload).unwrap();

        assert!(!store.thumbnail_path(&stored.file_name).exists());
        store.remove(&stored.file_name).unwrap();
        assert!(!store.origin_path(&stored.file_name).exists());
    }

    #[test]
    fn path_resolution_matches_store_layout() {
        let (_dir, store) = temp_store();
        let file_name = "abcdef0123456789.png";
        assert_eq!(
            store.origin_path(file_name),
            store
                .images_root()
                .join("origin")
                .join("ab")
                .join(file_name)
        );
        assert_eq!(
            store.thumbnail_path(file_name),
            store
                .images_root()
                .join("thumbnails")
                .join("ab")
                .join(file_name)
        );
    }

    mod tempdir_guard {
        use std::path::{Path, PathBuf};

        pub struct TempDir(PathBuf);

        impl TempDir {
            pub fn new() -> Self {
                let path = std::env::temp_dir()
                    .join(format!("ultra-clipboard-imgstore-{}", uuid::Uuid::new_v4()));
                std::fs::create_dir_all(&path).unwrap();
                Self(path)
            }
            pub fn path(&self) -> &Path {
                &self.0
            }
        }

        impl Drop for TempDir {
            fn drop(&mut self) {
                std::fs::remove_dir_all(&self.0).ok();
            }
        }
    }
}
