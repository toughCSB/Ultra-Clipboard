use axum::http::HeaderValue;
use image::ImageEncoder;
use std::io::Cursor;

use super::auth::{
    sign_headers_with_nonce, verify_headers, AuthError, PeerKey, HEADER_MAC, HEADER_VERSION,
};
use super::protocol::{
    ProtocolError, SyncEvent, MAX_EVENT_BYTES, MAX_IMAGE_BYTES, MAX_IMAGE_DIMENSION,
    MAX_PLAIN_TEXT_BYTES, MAX_RICH_TEXT_BYTES, PREFIX,
};
use super::secrets::{MemorySecretStore, SecretStore};
use crate::clipboard::{ClipboardPayload, ImagePayload, TextPayload};
use crate::settings::{SyncPeerSettings, SyncSettings};

fn text_event() -> SyncEvent {
    SyncEvent {
        origin_device_id: uuid::Uuid::new_v4().to_string(),
        event_id: uuid::Uuid::new_v4().to_string(),
        payload: ClipboardPayload::Text(TextPayload {
            text: "searchable plain".to_owned(),
            html: Some("<b>searchable plain</b>".to_owned()),
            rtf: Some(r"{\rtf1 searchable plain}".to_owned()),
        }),
    }
}

fn sample_png(width: u32, height: u32) -> Vec<u8> {
    let pixels = image::RgbaImage::from_pixel(width, height, image::Rgba([3, 4, 5, 255]));
    let mut output = Cursor::new(Vec::new());
    image::codecs::png::PngEncoder::new(&mut output)
        .write_image(
            pixels.as_raw(),
            width,
            height,
            image::ExtendedColorType::Rgba8,
        )
        .unwrap();
    output.into_inner()
}

fn raw_body(metadata: &str, binary: &[u8]) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(PREFIX);
    body.extend_from_slice(&(metadata.len() as u32).to_be_bytes());
    body.extend_from_slice(metadata.as_bytes());
    body.extend_from_slice(binary);
    body
}

#[test]
fn protocol_round_trips_text_html_rtf_and_png() {
    let text = text_event();
    assert_eq!(SyncEvent::decode(&text.encode().unwrap()).unwrap(), text);

    let image = SyncEvent {
        origin_device_id: uuid::Uuid::new_v4().to_string(),
        event_id: uuid::Uuid::new_v4().to_string(),
        payload: ClipboardPayload::Image(ImagePayload {
            bytes: sample_png(8, 6),
            width: 8,
            height: 6,
        }),
    };
    assert_eq!(SyncEvent::decode(&image.encode().unwrap()).unwrap(), image);
}

#[test]
fn protocol_rejects_files_unknown_types_and_oversized_events() {
    assert!(SyncEvent::from_local(
        &uuid::Uuid::new_v4().to_string(),
        &ClipboardPayload::Files(vec!["C:\\secret.txt".to_owned()])
    )
    .is_none());
    let unknown = raw_body(r#"{"type":"files","paths":["C:\\secret.txt"]}"#, &[]);
    assert_eq!(SyncEvent::decode(&unknown), Err(ProtocolError::Invalid));
    assert_eq!(
        SyncEvent::decode(&vec![0; MAX_EVENT_BYTES + 1]),
        Err(ProtocolError::TooLarge)
    );
}

#[test]
fn protocol_rejects_explicit_text_representation_and_image_byte_limits() {
    let mut event = text_event();
    event.payload = ClipboardPayload::Text(TextPayload {
        text: "a".repeat(MAX_PLAIN_TEXT_BYTES + 1),
        html: None,
        rtf: None,
    });
    assert_eq!(event.encode(), Err(ProtocolError::Invalid));

    event.payload = ClipboardPayload::Text(TextPayload {
        text: "plain".to_owned(),
        html: Some("a".repeat(MAX_RICH_TEXT_BYTES + 1)),
        rtf: None,
    });
    assert_eq!(event.encode(), Err(ProtocolError::TooLarge));

    event.payload = ClipboardPayload::Image(ImagePayload {
        bytes: vec![0; MAX_IMAGE_BYTES + 1],
        width: 1,
        height: 1,
    });
    assert_eq!(event.encode(), Err(ProtocolError::TooLarge));
}

#[test]
fn protocol_rejects_hash_tampering_malformed_png_and_dimension_bombs() {
    let event = SyncEvent {
        origin_device_id: uuid::Uuid::new_v4().to_string(),
        event_id: uuid::Uuid::new_v4().to_string(),
        payload: ClipboardPayload::Image(ImagePayload {
            bytes: sample_png(2, 2),
            width: 2,
            height: 2,
        }),
    };
    let mut tampered = event.encode().unwrap();
    let last = tampered.len() - 1;
    tampered[last] ^= 1;
    assert_eq!(
        SyncEvent::decode(&tampered),
        Err(ProtocolError::HashMismatch)
    );

    let malformed = b"not a png";
    let metadata = format!(
        r#"{{"type":"image","origin_device_id":"{}","event_id":"{}","width":1,"height":1,"payload_hash":"{}"}}"#,
        uuid::Uuid::new_v4(),
        uuid::Uuid::new_v4(),
        blake3::hash(malformed).to_hex()
    );
    assert_eq!(
        SyncEvent::decode(&raw_body(&metadata, malformed)),
        Err(ProtocolError::InvalidImage)
    );

    let too_wide = SyncEvent {
        origin_device_id: uuid::Uuid::new_v4().to_string(),
        event_id: uuid::Uuid::new_v4().to_string(),
        payload: ClipboardPayload::Image(ImagePayload {
            bytes: sample_png(1, 1),
            width: MAX_IMAGE_DIMENSION + 1,
            height: 1,
        }),
    };
    assert_eq!(too_wide.encode(), Err(ProtocolError::InvalidImage));

    let too_many_decoded_pixels = SyncEvent {
        origin_device_id: uuid::Uuid::new_v4().to_string(),
        event_id: uuid::Uuid::new_v4().to_string(),
        payload: ClipboardPayload::Image(ImagePayload {
            bytes: sample_png(2_048, 1_537),
            width: 2_048,
            height: 1_537,
        }),
    };
    assert_eq!(
        too_many_decoded_pixels.encode(),
        Err(ProtocolError::InvalidImage)
    );
}

#[test]
fn authentication_rejects_body_header_version_and_timestamp_tampering() {
    let key = PeerKey::derive("correct horse battery staple");
    let device = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp();
    let nonce = "ab".repeat(32);
    let headers = sign_headers_with_nonce(&key, &device, now, &nonce, b"body").unwrap();
    assert!(verify_headers(&key, &headers, b"body", now).is_ok());
    assert_eq!(
        verify_headers(&key, &headers, b"changed", now).unwrap_err(),
        AuthError::Rejected
    );

    let mut bad_version = headers.clone();
    bad_version.insert(HEADER_VERSION, HeaderValue::from_static("2"));
    assert_eq!(
        verify_headers(&key, &bad_version, b"body", now).unwrap_err(),
        AuthError::Rejected
    );
    let mut bad_mac = headers.clone();
    bad_mac.insert(HEADER_MAC, HeaderValue::from_static("00"));
    assert_eq!(
        verify_headers(&key, &bad_mac, b"body", now).unwrap_err(),
        AuthError::Rejected
    );
    assert_eq!(
        verify_headers(&key, &headers, b"body", now + 301).unwrap_err(),
        AuthError::Timestamp
    );
}

#[test]
fn secret_store_abstraction_replaces_deletes_and_never_serializes_psk() {
    let store = MemorySecretStore::new();
    store.set("peer-ref", "first secret value").unwrap();
    assert_eq!(
        store.get("peer-ref").unwrap().expose(),
        "first secret value"
    );
    store.set("peer-ref", "replacement secret").unwrap();
    assert_eq!(
        store.get("peer-ref").unwrap().expose(),
        "replacement secret"
    );
    store.delete("peer-ref").unwrap();
    assert!(store.get("peer-ref").is_err());

    let settings = SyncSettings {
        peers: vec![SyncPeerSettings {
            id: uuid::Uuid::new_v4().to_string(),
            address: "100.64.0.2".to_owned(),
            port: 45_321,
            secret_reference: "peer-ref".to_owned(),
        }],
        ..SyncSettings::default()
    };
    let serialized = serde_json::to_string(&settings).unwrap();
    assert!(serialized.contains("peer-ref"));
    assert!(!serialized.contains("replacement secret"));
}
