use axum::http::{HeaderMap, HeaderValue, Method};
use rand::{rngs::OsRng, RngCore};
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;

use super::protocol::CONTENT_TYPE;

pub const PROTOCOL_VERSION: &str = "1";
pub const EVENT_PATH: &str = "/v1/clipboard/events";
pub const HEADER_VERSION: &str = "x-ultra-clipboard-version";
pub const HEADER_DEVICE: &str = "x-ultra-clipboard-device";
pub const HEADER_TIMESTAMP: &str = "x-ultra-clipboard-timestamp";
pub const HEADER_NONCE: &str = "x-ultra-clipboard-nonce";
pub const HEADER_MAC: &str = "x-ultra-clipboard-mac";
const TIMESTAMP_WINDOW_SECONDS: u64 = 5 * 60;
const KEY_CONTEXT: &str = "Ultra Clipboard Tailscale clipboard sync PSK v1";

#[derive(Clone)]
pub struct PeerKey(Zeroizing<[u8; 32]>);

#[derive(Debug)]
pub struct VerifiedAuth {
    pub device_id: String,
    pub timestamp: i64,
    pub nonce: String,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AuthError {
    #[error("clipboard sync request is not authenticated")]
    Rejected,
    #[error("clipboard sync request timestamp is outside the accepted window")]
    Timestamp,
}

impl PeerKey {
    pub fn derive(psk: &str) -> Self {
        Self(Zeroizing::new(blake3::derive_key(
            KEY_CONTEXT,
            psk.as_bytes(),
        )))
    }
}

pub fn sign_headers(
    key: &PeerKey,
    device_id: &str,
    timestamp: i64,
    body: &[u8],
) -> Result<HeaderMap, AuthError> {
    let mut nonce_bytes = [0_u8; 32];
    OsRng.fill_bytes(&mut nonce_bytes);
    sign_headers_with_nonce(key, device_id, timestamp, &hex_encode(&nonce_bytes), body)
}

pub fn sign_headers_with_nonce(
    key: &PeerKey,
    device_id: &str,
    timestamp: i64,
    nonce: &str,
    body: &[u8],
) -> Result<HeaderMap, AuthError> {
    if decode_hex_32(nonce).is_none() {
        return Err(AuthError::Rejected);
    }
    let mac = calculate_mac(key, device_id, timestamp, nonce, body);
    let mut headers = HeaderMap::new();
    insert(&mut headers, HEADER_VERSION, PROTOCOL_VERSION)?;
    insert(&mut headers, HEADER_DEVICE, device_id)?;
    insert(&mut headers, HEADER_TIMESTAMP, &timestamp.to_string())?;
    insert(&mut headers, HEADER_NONCE, nonce)?;
    insert(&mut headers, HEADER_MAC, &hex_encode(mac.as_bytes()))?;
    insert(&mut headers, "content-type", CONTENT_TYPE)?;
    Ok(headers)
}

pub fn verify_headers(
    key: &PeerKey,
    headers: &HeaderMap,
    body: &[u8],
    now: i64,
) -> Result<VerifiedAuth, AuthError> {
    let version = header(headers, HEADER_VERSION)?;
    let device_id = header(headers, HEADER_DEVICE)?;
    let timestamp = header(headers, HEADER_TIMESTAMP)?
        .parse::<i64>()
        .map_err(|_| AuthError::Rejected)?;
    let nonce = header(headers, HEADER_NONCE)?;
    let supplied_mac = decode_hex_32(header(headers, HEADER_MAC)?).ok_or(AuthError::Rejected)?;
    let content_type = header(headers, "content-type")?;
    if version != PROTOCOL_VERSION || content_type != CONTENT_TYPE || decode_hex_32(nonce).is_none()
    {
        return Err(AuthError::Rejected);
    }
    if timestamp.abs_diff(now) > TIMESTAMP_WINDOW_SECONDS {
        return Err(AuthError::Timestamp);
    }
    let expected = calculate_mac(key, device_id, timestamp, nonce, body);
    if !bool::from(expected.as_bytes().ct_eq(&supplied_mac)) {
        return Err(AuthError::Rejected);
    }
    Ok(VerifiedAuth {
        device_id: device_id.to_owned(),
        timestamp,
        nonce: nonce.to_owned(),
    })
}

pub fn body_hash(body: &[u8]) -> String {
    blake3::hash(body).to_hex().to_string()
}

fn calculate_mac(
    key: &PeerKey,
    device_id: &str,
    timestamp: i64,
    nonce: &str,
    body: &[u8],
) -> blake3::Hash {
    let mut hasher = blake3::Hasher::new_keyed(&key.0);
    for field in [
        PROTOCOL_VERSION.as_bytes(),
        Method::POST.as_str().as_bytes(),
        EVENT_PATH.as_bytes(),
        device_id.as_bytes(),
        timestamp.to_string().as_bytes(),
        nonce.as_bytes(),
        CONTENT_TYPE.as_bytes(),
        body,
    ] {
        hasher.update(&field.len().to_be_bytes());
        hasher.update(field);
    }
    hasher.finalize()
}

fn header<'a>(headers: &'a HeaderMap, name: &str) -> Result<&'a str, AuthError> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .ok_or(AuthError::Rejected)
}

fn insert(headers: &mut HeaderMap, name: &'static str, value: &str) -> Result<(), AuthError> {
    let value = HeaderValue::from_str(value).map_err(|_| AuthError::Rejected)?;
    headers.insert(name, value);
    Ok(())
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

fn decode_hex_32(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64 {
        return None;
    }
    let mut output = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let high = decode_nibble(pair[0])?;
        let low = decode_nibble(pair[1])?;
        output[index] = (high << 4) | low;
    }
    Some(output)
}

const fn decode_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}
