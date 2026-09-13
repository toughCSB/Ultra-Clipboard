use std::sync::Arc;
use std::time::Duration;

use image::ImageEncoder;
use tokio::sync::mpsc;

use super::receiver::EventSink;
use super::runtime::{RuntimeInput, SyncRuntime};
use super::secrets::{MemorySecretStore, SecretStore};
use crate::clipboard::{ClipboardPayload, ImagePayload, ImageStore, TextPayload};
use crate::db::items::{find_item_by_content_hash, UpsertResult};
use crate::db::models::ClipboardKind;
use crate::db::test_support::memory_pool;
use crate::settings::{Sensitive, SyncPeerSettings, SyncSettings};

#[derive(Clone)]
struct ChannelSink(mpsc::Sender<UpsertResult>);

impl EventSink for ChannelSink {
    fn clipboard_updated(&self, result: &UpsertResult, _kind: ClipboardKind) {
        let _ = self.0.try_send(result.clone());
    }
}

fn free_port() -> u16 {
    std::net::TcpListener::bind(("127.0.0.1", 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn sample_png(width: u32, height: u32) -> Vec<u8> {
    let pixels = image::RgbaImage::from_pixel(width, height, image::Rgba([3, 4, 5, 255]));
    let mut output = Vec::new();
    image::codecs::png::PngEncoder::new(&mut output)
        .write_image(
            pixels.as_raw(),
            width,
            height,
            image::ExtendedColorType::Rgba8,
        )
        .unwrap();
    output
}

fn test_settings(device_id: &str, port: u16, peer_id: &str, peer_port: u16) -> SyncSettings {
    SyncSettings {
        enabled: true,
        bind_address: "127.0.0.1".to_owned(),
        listen_port: port,
        device_id: device_id.to_owned(),
        peers: vec![SyncPeerSettings {
            id: peer_id.to_owned(),
            address: "127.0.0.1".to_owned(),
            port: peer_port,
            secret_reference: "shared-peer-secret".to_owned(),
        }],
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn two_localhost_runtimes_exchange_without_resend_or_clipboard_write() {
    let port_a = free_port();
    let port_b = free_port();
    let device_a = uuid::Uuid::new_v4().to_string();
    let device_b = uuid::Uuid::new_v4().to_string();
    let pool_a = memory_pool().await;
    let pool_b = memory_pool().await;
    let root_a = tempfile::tempdir().unwrap();
    let root_b = tempfile::tempdir().unwrap();
    let secrets_a = Arc::new(MemorySecretStore::new());
    let secrets_b = Arc::new(MemorySecretStore::new());
    secrets_a
        .set("shared-peer-secret", "integration shared secret")
        .unwrap();
    secrets_b
        .set("shared-peer-secret", "integration shared secret")
        .unwrap();
    let runtime_a = SyncRuntime::new(secrets_a);
    let runtime_b = SyncRuntime::new(secrets_b);
    let (sink_a_tx, mut sink_a_rx) = mpsc::channel(4);
    let (sink_b_tx, mut sink_b_rx) = mpsc::channel(4);

    runtime_b
        .configure_for_test(RuntimeInput {
            settings: test_settings(&device_b, port_b, &device_a, port_a),
            pool: pool_b.clone(),
            image_store: ImageStore::for_test(root_b.path().join("images")),
            sensitive: Sensitive::default(),
            sink: Arc::new(ChannelSink(sink_b_tx)),
        })
        .await
        .unwrap();
    runtime_a
        .configure_for_test(RuntimeInput {
            settings: test_settings(&device_a, port_a, &device_b, port_b),
            pool: pool_a.clone(),
            image_store: ImageStore::for_test(root_a.path().join("images")),
            sensitive: Sensitive::default(),
            sink: Arc::new(ChannelSink(sink_a_tx)),
        })
        .await
        .unwrap();

    let text = ClipboardPayload::Text(TextPayload {
        text: "integration searchable text".to_owned(),
        html: Some("<strong>integration searchable text</strong>".to_owned()),
        rtf: None,
    });
    assert_eq!(runtime_a.publish_local(&text), 1);
    let stored_b = tokio::time::timeout(Duration::from_secs(3), sink_b_rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(!stored_b.deduplicated);
    assert!(
        tokio::time::timeout(Duration::from_millis(250), sink_a_rx.recv())
            .await
            .is_err()
    );

    let hash = crate::db::items::content_hash(
        ClipboardKind::Text,
        "<strong>integration searchable text</strong>",
    );
    let item_b = find_item_by_content_hash(&pool_b, &hash)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        item_b.search_text.as_deref(),
        Some("integration searchable text")
    );

    let image_bytes = sample_png(12, 10);
    let image = ClipboardPayload::Image(ImagePayload {
        bytes: image_bytes.clone(),
        width: 12,
        height: 10,
    });
    assert_eq!(runtime_b.publish_local(&image), 1);
    tokio::time::timeout(Duration::from_secs(3), sink_a_rx.recv())
        .await
        .unwrap()
        .unwrap();
    let file_name = format!("{}.png", blake3::hash(&image_bytes).to_hex());
    assert_eq!(
        std::fs::read(ImageStore::for_test(root_a.path().join("images")).origin_path(&file_name))
            .unwrap(),
        image_bytes
    );

    runtime_a.stop().await;
    runtime_b.stop().await;
}
