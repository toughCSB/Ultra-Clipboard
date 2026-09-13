use serde::{Deserialize, Serialize};

use crate::clipboard::{ClipboardPayload, ImagePayload, TextPayload};

mod image_validation;

use image_validation::validate_image;

pub const CONTENT_TYPE: &str = "application/vnd.ultra-clipboard.event";
pub const MAX_EVENT_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_IMAGE_BYTES: usize = 12 * 1024 * 1024;
pub const MAX_PLAIN_TEXT_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_RICH_TEXT_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_IMAGE_DIMENSION: u32 = 8_192;
pub const MAX_IMAGE_PIXELS: u64 = 3 * 1024 * 1024;

pub(super) const PREFIX: &[u8; 5] = b"UCE1\n";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncEvent {
    pub origin_device_id: String,
    pub event_id: String,
    pub payload: ClipboardPayload,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ProtocolError {
    #[error("unsupported clipboard payload")]
    Unsupported,
    #[error("clipboard sync event is invalid")]
    Invalid,
    #[error("clipboard sync event is too large")]
    TooLarge,
    #[error("clipboard sync image is invalid")]
    InvalidImage,
    #[error("clipboard sync content hash does not match")]
    HashMismatch,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
enum Metadata {
    Text {
        origin_device_id: String,
        event_id: String,
        plain_text: String,
        html: Option<String>,
        rtf: Option<String>,
        payload_hash: String,
    },
    Image {
        origin_device_id: String,
        event_id: String,
        width: u32,
        height: u32,
        payload_hash: String,
    },
}

impl SyncEvent {
    pub fn from_local(origin_device_id: &str, payload: &ClipboardPayload) -> Option<Self> {
        match payload {
            ClipboardPayload::Text(text) => Some(Self {
                origin_device_id: origin_device_id.to_owned(),
                event_id: uuid::Uuid::new_v4().to_string(),
                payload: ClipboardPayload::Text(text.clone()),
            }),
            ClipboardPayload::Image(image) => Some(Self {
                origin_device_id: origin_device_id.to_owned(),
                event_id: uuid::Uuid::new_v4().to_string(),
                payload: ClipboardPayload::Image(image.clone()),
            }),
            ClipboardPayload::Files(_) => None,
        }
    }

    pub fn encode(&self) -> Result<Vec<u8>, ProtocolError> {
        validate_identity(&self.origin_device_id, &self.event_id)?;
        let (metadata, binary) = match &self.payload {
            ClipboardPayload::Text(text) => {
                validate_text(text)?;
                (
                    Metadata::Text {
                        origin_device_id: self.origin_device_id.clone(),
                        event_id: self.event_id.clone(),
                        plain_text: text.text.clone(),
                        html: text.html.clone(),
                        rtf: text.rtf.clone(),
                        payload_hash: text_hash(text),
                    },
                    &[][..],
                )
            }
            ClipboardPayload::Image(image) => {
                validate_image(image)?;
                (
                    Metadata::Image {
                        origin_device_id: self.origin_device_id.clone(),
                        event_id: self.event_id.clone(),
                        width: image.width,
                        height: image.height,
                        payload_hash: blake3::hash(&image.bytes).to_hex().to_string(),
                    },
                    image.bytes.as_slice(),
                )
            }
            ClipboardPayload::Files(_) => return Err(ProtocolError::Unsupported),
        };
        let metadata = serde_json::to_vec(&metadata).map_err(|_| ProtocolError::Invalid)?;
        let metadata_len = u32::try_from(metadata.len()).map_err(|_| ProtocolError::TooLarge)?;
        let total = PREFIX.len() + 4 + metadata.len() + binary.len();
        if total > MAX_EVENT_BYTES {
            return Err(ProtocolError::TooLarge);
        }
        let mut body = Vec::with_capacity(total);
        body.extend_from_slice(PREFIX);
        body.extend_from_slice(&metadata_len.to_be_bytes());
        body.extend_from_slice(&metadata);
        body.extend_from_slice(binary);
        Ok(body)
    }

    pub fn decode(body: &[u8]) -> Result<Self, ProtocolError> {
        if body.len() > MAX_EVENT_BYTES {
            return Err(ProtocolError::TooLarge);
        }
        if body.len() < PREFIX.len() + 4 || &body[..PREFIX.len()] != PREFIX {
            return Err(ProtocolError::Invalid);
        }
        let length_bytes: [u8; 4] = body[PREFIX.len()..PREFIX.len() + 4]
            .try_into()
            .map_err(|_| ProtocolError::Invalid)?;
        let metadata_len = usize::try_from(u32::from_be_bytes(length_bytes))
            .map_err(|_| ProtocolError::Invalid)?;
        let metadata_start = PREFIX.len() + 4;
        let metadata_end = metadata_start
            .checked_add(metadata_len)
            .filter(|end| *end <= body.len())
            .ok_or(ProtocolError::Invalid)?;
        let metadata: Metadata = serde_json::from_slice(&body[metadata_start..metadata_end])
            .map_err(|_| ProtocolError::Invalid)?;
        let binary = &body[metadata_end..];
        match metadata {
            Metadata::Text {
                origin_device_id,
                event_id,
                plain_text,
                html,
                rtf,
                payload_hash,
            } => {
                if !binary.is_empty() {
                    return Err(ProtocolError::Invalid);
                }
                let text = TextPayload {
                    text: plain_text,
                    html,
                    rtf,
                };
                validate_identity(&origin_device_id, &event_id)?;
                validate_text(&text)?;
                if text_hash(&text) != payload_hash {
                    return Err(ProtocolError::HashMismatch);
                }
                Ok(Self {
                    origin_device_id,
                    event_id,
                    payload: ClipboardPayload::Text(text),
                })
            }
            Metadata::Image {
                origin_device_id,
                event_id,
                width,
                height,
                payload_hash,
            } => {
                validate_identity(&origin_device_id, &event_id)?;
                if blake3::hash(binary).to_hex().as_str() != payload_hash {
                    return Err(ProtocolError::HashMismatch);
                }
                let image = ImagePayload {
                    bytes: binary.to_vec(),
                    width,
                    height,
                };
                validate_image(&image)?;
                Ok(Self {
                    origin_device_id,
                    event_id,
                    payload: ClipboardPayload::Image(image),
                })
            }
        }
    }
}

fn validate_identity(origin: &str, event: &str) -> Result<(), ProtocolError> {
    if uuid::Uuid::parse_str(origin).is_err() || uuid::Uuid::parse_str(event).is_err() {
        return Err(ProtocolError::Invalid);
    }
    Ok(())
}

fn validate_text(text: &TextPayload) -> Result<(), ProtocolError> {
    if text.text.trim().is_empty() || text.text.len() > MAX_PLAIN_TEXT_BYTES {
        return Err(ProtocolError::Invalid);
    }
    if text
        .html
        .as_ref()
        .is_some_and(|value| value.len() > MAX_RICH_TEXT_BYTES)
        || text
            .rtf
            .as_ref()
            .is_some_and(|value| value.len() > MAX_RICH_TEXT_BYTES)
    {
        return Err(ProtocolError::TooLarge);
    }
    Ok(())
}

fn text_hash(text: &TextPayload) -> String {
    let mut hasher = blake3::Hasher::new();
    hash_field(&mut hasher, text.text.as_bytes());
    hash_optional_field(&mut hasher, text.html.as_deref());
    hash_optional_field(&mut hasher, text.rtf.as_deref());
    hasher.finalize().to_hex().to_string()
}

fn hash_optional_field(hasher: &mut blake3::Hasher, value: Option<&str>) {
    match value {
        Some(value) => {
            hasher.update(&[1]);
            hash_field(hasher, value.as_bytes());
        }
        None => {
            hasher.update(&[0]);
        }
    }
}

fn hash_field(hasher: &mut blake3::Hasher, value: &[u8]) {
    hasher.update(&value.len().to_be_bytes());
    hasher.update(value);
}
