//! The zip payload inside a backup: manifest, database, resources, and
//! sanitized settings.

use std::fs::{self, File};
use std::io::{BufWriter, Cursor, Seek, Write};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context};
use chrono::{DateTime, Utc};
use serde::Serialize;
use tempfile::TempDir;
use walkdir::WalkDir;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use super::app_error;
use crate::core::Result;

pub(super) const SETTINGS_FILENAME: &str = "settings.json";
pub(super) const DB_FILENAME: &str = "clipboard.db";
pub(super) const DB_ARCHIVE_DIR: &str = "db";
pub(super) const RESOURCES_ARCHIVE_DIR: &str = "resources";
pub(super) const CONFIG_ARCHIVE_DIR: &str = "config";
const MANIFEST_FILENAME: &str = "manifest.json";
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct BackupManifest {
    pub(super) format_version: u16,
    pub(super) app_name: String,
    pub(super) app_version: String,
    pub(super) exported_at: DateTime<Utc>,
    pub(super) platform: String,
    pub(super) encryption: ManifestEncryption,
    pub(super) item_count: i64,
    pub(super) text_count: i64,
    pub(super) image_count: i64,
    pub(super) files_count: i64,
    pub(super) resource_bytes: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) enum ManifestEncryption {
    None,
    Password,
}

#[derive(Debug, Clone)]
pub(super) struct BackupSourcePaths {
    pub(super) db_path: PathBuf,
    pub(super) resources_dir: PathBuf,
    pub(super) settings_path: PathBuf,
}

pub(super) fn write_payload_zip(
    source_paths: &BackupSourcePaths,
    path: &Path,
    manifest: &BackupManifest,
    target_path: &Path,
) -> Result<()> {
    let file = File::create(path).with_context(|| format!("failed to create payload {path:?}"))?;
    let mut zip = ZipWriter::new(BufWriter::new(file));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    zip.start_file("manifest.json", options)
        .context("failed to write backup manifest entry")?;
    let manifest_bytes =
        serde_json::to_vec_pretty(manifest).context("failed to serialize backup manifest")?;
    zip.write_all(&manifest_bytes)
        .context("failed to write backup manifest")?;

    add_optional_file(
        &mut zip,
        &source_paths.db_path,
        &archive_path(DB_ARCHIVE_DIR, Path::new("clipboard.db"))?,
        options,
        target_path,
    )?;
    add_dir_contents(
        &mut zip,
        &source_paths.resources_dir,
        RESOURCES_ARCHIVE_DIR,
        options,
        target_path,
    )?;
    if source_paths.settings_path.exists() && !same_path(&source_paths.settings_path, target_path) {
        let settings_bytes = fs::read(&source_paths.settings_path)
            .with_context(|| format!("failed to read {:?}", source_paths.settings_path))?;
        let mut settings: serde_json::Value = serde_json::from_slice(&settings_bytes)
            .context("failed to parse settings for backup")?;
        if let Some(webdav) = settings
            .get_mut("webdav")
            .and_then(serde_json::Value::as_object_mut)
        {
            webdav.remove("password");
        }
        if let Some(settings) = settings.as_object_mut() {
            settings.remove("sync");
        }

        let archive_name = archive_path(CONFIG_ARCHIVE_DIR, Path::new(SETTINGS_FILENAME))?;
        zip.start_file(&archive_name, options)
            .with_context(|| format!("failed to start archive file {archive_name}"))?;
        serde_json::to_writer_pretty(&mut zip, &settings)
            .context("failed to write sanitized backup settings")?;
    }

    zip.finish()
        .context("failed to finish backup payload archive")?;

    Ok(())
}

fn add_file<W: Write + Seek>(
    zip: &mut ZipWriter<W>,
    path: &Path,
    archive_name: &str,
    options: SimpleFileOptions,
) -> Result<()> {
    zip.start_file(archive_name, options)
        .with_context(|| format!("failed to start archive file {archive_name}"))?;
    let mut file = File::open(path).with_context(|| format!("failed to open {path:?}"))?;
    std::io::copy(&mut file, zip).with_context(|| format!("failed to archive {path:?}"))?;

    Ok(())
}

fn add_optional_file<W: Write + Seek>(
    zip: &mut ZipWriter<W>,
    path: &Path,
    archive_name: &str,
    options: SimpleFileOptions,
    target_path: &Path,
) -> Result<()> {
    if !path.exists() || same_path(path, target_path) {
        return Ok(());
    }

    add_file(zip, path, archive_name, options)
}

fn add_dir_contents<W: Write + Seek>(
    zip: &mut ZipWriter<W>,
    root: &Path,
    archive_root: &str,
    options: SimpleFileOptions,
    target_path: &Path,
) -> Result<()> {
    if !root.exists() {
        return Ok(());
    }

    for entry in WalkDir::new(root)
        .into_iter()
        .filter_map(std::result::Result::ok)
    {
        let path = entry.path();
        let metadata = entry
            .metadata()
            .with_context(|| format!("failed to read metadata at {path:?}"))?;
        if !metadata.is_file() {
            continue;
        }
        let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if should_skip_backup_path(path, file_name, target_path) {
            continue;
        }

        let relative = path
            .strip_prefix(root)
            .with_context(|| format!("failed to strip root {root:?} from {path:?}"))?;
        let archive_name = archive_path(archive_root, relative)?;
        add_file(zip, path, &archive_name, options)?;
    }

    Ok(())
}

fn archive_path(prefix: &str, relative: &Path) -> Result<String> {
    let mut parts = Vec::new();
    if !prefix.is_empty() {
        parts.push(prefix.to_owned());
    }
    for part in relative.components() {
        let std::path::Component::Normal(value) = part else {
            return app_error("backup path contains unsupported component");
        };
        let value = value
            .to_str()
            .ok_or_else(|| anyhow!("backup path is not valid utf-8"))?;
        parts.push(value.to_owned());
    }

    Ok(parts.join("/"))
}

fn should_skip_backup_path(path: &Path, file_name: &str, target_path: &Path) -> bool {
    if same_path(path, target_path) {
        return true;
    }

    file_name.ends_with(".tmp")
        || file_name.ends_with(".temp")
        || file_name.starts_with(".tmp")
        || file_name == ".DS_Store"
}

fn same_path(left: &Path, right: &Path) -> bool {
    let left = fs::canonicalize(left).unwrap_or_else(|_| left.to_path_buf());
    let right = fs::canonicalize(right).unwrap_or_else(|_| right.to_path_buf());

    left == right
}

pub(super) fn extract_payload_zip(payload: &[u8]) -> Result<TempDir> {
    let temp = tempfile::tempdir().context("failed to create temporary import directory")?;
    let cursor = Cursor::new(payload);
    let mut archive = ZipArchive::new(cursor).context("failed to read backup zip payload")?;
    archive
        .extract(temp.path())
        .context("failed to extract backup payload")?;

    Ok(temp)
}

pub(super) fn validate_extracted_payload(root: &Path) -> Result<()> {
    if !root.join(MANIFEST_FILENAME).exists() {
        return app_error("백업 파일에 manifest.json이 없습니다");
    }
    if !root.join(DB_ARCHIVE_DIR).join(DB_FILENAME).exists() {
        return app_error("백업 파일에 기록 데이터베이스가 없습니다");
    }
    if !root
        .join(CONFIG_ARCHIVE_DIR)
        .join(SETTINGS_FILENAME)
        .exists()
    {
        return app_error("백업 파일에 설정 파일이 없습니다");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use tempfile::tempdir;
    use zip::ZipArchive;

    use crate::backup::container::FORMAT_VERSION;

    #[test]
    fn payload_zip_contains_only_backup_whitelist_and_omits_webdav_password() {
        let temp = tempdir().unwrap();
        let root = temp.path();
        let resources = root.join("resources");
        fs::create_dir_all(resources.join("clipboard-images/origin")).unwrap();
        fs::write(root.join("clipboard.db"), b"db").unwrap();
        fs::write(
            root.join("settings.json"),
            br#"{"webdav":{"username":"me","password":"remote-secret"}}"#,
        )
        .unwrap();
        fs::write(root.join("window-state.json"), b"skip").unwrap();
        fs::write(root.join("settings.json.bak"), b"skip").unwrap();
        fs::write(resources.join("clipboard-images/origin/demo.png"), b"image").unwrap();

        let payload = root.join("payload.zip");
        let target = root.join("backup.ecopastebak");
        let manifest = BackupManifest {
            format_version: FORMAT_VERSION,
            app_name: "Ultra Clipboard".to_owned(),
            app_version: "0.0.0".to_owned(),
            exported_at: Utc::now(),
            platform: "macos".to_owned(),
            encryption: ManifestEncryption::None,
            item_count: 1,
            text_count: 1,
            image_count: 0,
            files_count: 0,
            resource_bytes: 5,
        };
        let source_paths = BackupSourcePaths {
            db_path: root.join("clipboard.db"),
            resources_dir: resources,
            settings_path: root.join("settings.json"),
        };

        write_payload_zip(&source_paths, &payload, &manifest, &target).unwrap();

        let file = File::open(payload).unwrap();
        let mut archive = ZipArchive::new(file).unwrap();
        let mut names = Vec::new();
        for index in 0..archive.len() {
            let file = archive.by_index(index).unwrap();
            names.push(file.name().to_owned());
        }
        names.sort();

        assert_eq!(
            names,
            vec![
                "config/settings.json",
                "db/clipboard.db",
                "manifest.json",
                "resources/clipboard-images/origin/demo.png",
            ]
        );

        let mut settings_entry = archive.by_name("config/settings.json").unwrap();
        let mut settings_json = String::new();
        settings_entry.read_to_string(&mut settings_json).unwrap();
        let settings: serde_json::Value = serde_json::from_str(&settings_json).unwrap();

        assert_eq!(settings.pointer("/webdav/username").unwrap(), "me");
        assert!(settings.pointer("/webdav/password").is_none());
    }
}
