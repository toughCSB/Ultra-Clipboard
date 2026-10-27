//! The `.ecopastebak` container: a plain zip, or a magic + JSON header followed
//! by the Argon2id/XChaCha20-Poly1305 encrypted zip.

use std::fs::{self, File};
use std::io::{BufReader, Cursor, Read, Write};
use std::path::Path;

use anyhow::{anyhow, Context};
use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;
use zeroize::Zeroizing;

use super::{app_error, BackupContainerMode, BackupExportMode};
use crate::core::{AppError, Result};

const MAGIC: &[u8; 12] = b"ECOPASTEBAK1";
const ZIP_MAGIC: &[u8; 2] = b"PK";
const HEADER_LEN_BYTES: usize = 4;
pub(super) const FORMAT_VERSION: u16 = 1;
const ARGON2_MEMORY_KIB: u32 = 64 * 1024;
const ARGON2_TIME_COST: u32 = 3;
const ARGON2_PARALLELISM: u32 = 1;
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 24;
const KEY_LEN: usize = 32;
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ContainerHeader {
    format_version: u16,
    mode: BackupContainerMode,
    kdf: Option<KdfHeader>,
    cipher: Option<CipherHeader>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct KdfHeader {
    algorithm: String,
    memory_kib: u32,
    time_cost: u32,
    parallelism: u32,
    salt: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CipherHeader {
    algorithm: String,
    nonce: Vec<u8>,
}

pub(super) fn write_container(
    target: &Path,
    payload_path: &Path,
    mode: BackupExportMode,
    password: Option<Zeroizing<String>>,
) -> Result<u64> {
    let parent = target
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).with_context(|| format!("failed to create {parent:?}"))?;

    let mut temp = NamedTempFile::new_in(parent)
        .with_context(|| format!("failed to create temporary file under {parent:?}"))?;
    match mode {
        BackupExportMode::Plain => {
            copy_file_into_writer(payload_path, &mut temp)?;
        }
        BackupExportMode::Encrypted => {
            let password = password.ok_or_else(|| anyhow!("missing backup password"))?;
            let mut payload = Vec::new();
            File::open(payload_path)
                .with_context(|| format!("failed to open payload {payload_path:?}"))?
                .read_to_end(&mut payload)
                .context("failed to read backup payload")?;

            let encrypted = encrypt_payload(&payload, &password)?;
            write_header(&mut temp, &encrypted.header)?;
            temp.write_all(&encrypted.ciphertext)
                .context("failed to write encrypted backup payload")?;
        }
    }

    temp.flush().context("failed to flush backup file")?;
    temp.as_file()
        .sync_all()
        .context("failed to sync backup file")?;
    temp.persist(target)
        .map_err(|err| anyhow!(err))
        .with_context(|| format!("failed to persist backup to {target:?}"))?;

    let total_bytes = fs::metadata(target)
        .with_context(|| format!("failed to read backup metadata {target:?}"))?
        .len();

    Ok(total_bytes)
}

struct EncryptedPayload {
    header: ContainerHeader,
    ciphertext: Vec<u8>,
}

fn encrypt_payload(payload: &[u8], password: &str) -> Result<EncryptedPayload> {
    let mut salt = [0u8; SALT_LEN];
    let mut nonce = [0u8; NONCE_LEN];
    rand::rngs::OsRng.fill_bytes(&mut salt);
    rand::rngs::OsRng.fill_bytes(&mut nonce);

    let mut key = Zeroizing::new([0u8; KEY_LEN]);
    let params = Params::new(
        ARGON2_MEMORY_KIB,
        ARGON2_TIME_COST,
        ARGON2_PARALLELISM,
        Some(KEY_LEN),
    )
    .context("failed to build argon2 params")?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    argon2
        .hash_password_into(password.as_bytes(), &salt, key.as_mut())
        .context("failed to derive backup key")?;

    let cipher = XChaCha20Poly1305::new_from_slice(key.as_ref())
        .context("failed to initialize backup cipher")?;
    let nonce = XNonce::from(nonce);
    let ciphertext = cipher
        .encrypt(&nonce, payload)
        .map_err(|_| anyhow!("failed to encrypt backup payload"))?;

    Ok(EncryptedPayload {
        header: ContainerHeader {
            format_version: FORMAT_VERSION,
            mode: BackupContainerMode::Encrypted,
            kdf: Some(KdfHeader {
                algorithm: "argon2id".to_owned(),
                memory_kib: ARGON2_MEMORY_KIB,
                time_cost: ARGON2_TIME_COST,
                parallelism: ARGON2_PARALLELISM,
                salt: salt.to_vec(),
            }),
            cipher: Some(CipherHeader {
                algorithm: "xchacha20poly1305".to_owned(),
                nonce: nonce.to_vec(),
            }),
        },
        ciphertext,
    })
}

fn write_header<W: Write>(writer: &mut W, header: &ContainerHeader) -> Result<()> {
    let header_bytes = serde_json::to_vec(header).context("failed to serialize backup header")?;
    let header_len: u32 = header_bytes
        .len()
        .try_into()
        .context("backup header is too large")?;

    writer
        .write_all(MAGIC)
        .context("failed to write backup magic")?;
    writer
        .write_all(&header_len.to_le_bytes())
        .context("failed to write backup header length")?;
    writer
        .write_all(&header_bytes)
        .context("failed to write backup header")?;

    Ok(())
}

pub(super) fn read_backup_payload(path: &Path, password: Option<&str>) -> Result<Vec<u8>> {
    let mut file = File::open(path).with_context(|| format!("failed to open backup {path:?}"))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .with_context(|| format!("failed to read backup {path:?}"))?;

    if bytes.starts_with(ZIP_MAGIC) {
        return Ok(bytes);
    }
    if !bytes.starts_with(MAGIC) {
        return app_error("올바른 Ultra Clipboard 백업 파일이 아닙니다");
    }

    let mut cursor = Cursor::new(bytes.as_slice());
    cursor.set_position(MAGIC.len() as u64);
    let header = read_container_header_after_magic(&mut cursor)?;
    let ciphertext_start = cursor.position() as usize;
    let Some(kdf) = header.kdf else {
        return app_error("암호화 백업 파일 헤더가 올바르지 않습니다");
    };
    let Some(cipher) = header.cipher else {
        return app_error("암호화 백업 파일 헤더가 올바르지 않습니다");
    };
    let Some(password) = password else {
        return app_error("백업 비밀번호를 입력하세요");
    };

    decrypt_payload(&bytes[ciphertext_start..], password, &kdf, &cipher)
}

fn decrypt_payload(
    ciphertext: &[u8],
    password: &str,
    kdf: &KdfHeader,
    cipher_header: &CipherHeader,
) -> Result<Vec<u8>> {
    if kdf.algorithm != "argon2id" || cipher_header.algorithm != "xchacha20poly1305" {
        return app_error("이 백업 암호화 형식은 지원하지 않습니다");
    }
    if kdf.salt.len() != SALT_LEN || cipher_header.nonce.len() != NONCE_LEN {
        return app_error("암호화 백업 파일 헤더가 올바르지 않습니다");
    }

    let mut key = Zeroizing::new([0u8; KEY_LEN]);
    let params = Params::new(
        kdf.memory_kib,
        kdf.time_cost,
        kdf.parallelism,
        Some(KEY_LEN),
    )
    .context("failed to build argon2 params")?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    argon2
        .hash_password_into(password.as_bytes(), &kdf.salt, key.as_mut())
        .context("failed to derive backup key")?;

    let cipher =
        XChaCha20Poly1305::new_from_slice(key.as_ref()).context("failed to initialize cipher")?;
    let mut nonce = [0u8; NONCE_LEN];
    nonce.copy_from_slice(&cipher_header.nonce);
    let nonce = XNonce::from(nonce);
    cipher.decrypt(&nonce, ciphertext).map_err(|_| {
        AppError::Other(anyhow!(
            "백업 비밀번호가 올바르지 않거나 파일이 손상되었습니다"
        ))
    })
}

pub(super) fn inspect_backup_reader<R: Read>(reader: &mut R) -> Result<BackupContainerMode> {
    let mut magic = [0u8; MAGIC.len()];
    let read = reader
        .read(&mut magic)
        .context("failed to read backup magic")?;
    if read >= MAGIC.len() && &magic == MAGIC {
        let header = read_container_header_after_magic(reader)?;
        return Ok(header.mode);
    }
    if read >= ZIP_MAGIC.len() && &magic[..ZIP_MAGIC.len()] == ZIP_MAGIC {
        return Ok(BackupContainerMode::Plain);
    }

    app_error("올바른 Ultra Clipboard 백업 파일이 아닙니다")
}

fn read_container_header_after_magic<R: Read>(reader: &mut R) -> Result<ContainerHeader> {
    let mut len = [0u8; HEADER_LEN_BYTES];
    reader
        .read_exact(&mut len)
        .context("failed to read backup header length")?;
    let header_len = u32::from_le_bytes(len) as usize;
    if header_len == 0 || header_len > 64 * 1024 {
        return app_error("백업 파일 헤더가 올바르지 않습니다");
    }

    let mut header = vec![0u8; header_len];
    reader
        .read_exact(&mut header)
        .context("failed to read backup header")?;
    let header: ContainerHeader =
        serde_json::from_slice(&header).context("failed to parse backup header")?;
    if header.format_version != FORMAT_VERSION {
        return app_error("이 백업 형식 버전은 지원하지 않습니다");
    }

    Ok(header)
}

fn copy_file_into_writer<W: Write>(path: &Path, writer: &mut W) -> Result<()> {
    let file = File::open(path).with_context(|| format!("failed to open {path:?}"))?;
    let mut reader = BufReader::new(file);
    std::io::copy(&mut reader, writer).with_context(|| format!("failed to copy {path:?}"))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypted_payload_header_round_trips() {
        let encrypted = encrypt_payload(b"hello", "password-123").unwrap();
        let mut bytes = Vec::new();
        write_header(&mut bytes, &encrypted.header).unwrap();
        bytes.extend_from_slice(&encrypted.ciphertext);

        let mut cursor = Cursor::new(bytes);

        assert_eq!(
            inspect_backup_reader(&mut cursor).unwrap(),
            BackupContainerMode::Encrypted
        );
    }

    #[test]
    fn inspect_backup_reader_recognizes_plain_zip() {
        let mut cursor = Cursor::new(b"PK\x03\x04demo".to_vec());

        assert_eq!(
            inspect_backup_reader(&mut cursor).unwrap(),
            BackupContainerMode::Plain
        );
    }

    #[test]
    fn inspect_backup_reader_recognizes_encrypted_container() {
        let encrypted = encrypt_payload(b"hello", "password-123").unwrap();
        let mut bytes = Vec::new();
        write_header(&mut bytes, &encrypted.header).unwrap();
        bytes.extend_from_slice(&encrypted.ciphertext);
        let mut cursor = Cursor::new(bytes);

        assert_eq!(
            inspect_backup_reader(&mut cursor).unwrap(),
            BackupContainerMode::Encrypted
        );
    }
}
