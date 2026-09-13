use std::sync::Arc;

use serde_json::json;
use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter};

use super::auth::VerifiedAuth;
use super::persistence::{self, ReplayRecord, Reservation};
use super::protocol::SyncEvent;
use crate::clipboard::{build_item_with_settings, ImageStore};
use crate::db::items::{upsert_item, UpsertResult};
use crate::db::models::ClipboardKind;
use crate::settings::{Capture, Sensitive};

pub trait EventSink: Send + Sync {
    fn clipboard_updated(&self, result: &UpsertResult, kind: ClipboardKind);
}

pub struct AppEventSink(AppHandle);

impl AppEventSink {
    pub fn new(app: AppHandle) -> Self {
        Self(app)
    }
}

impl EventSink for AppEventSink {
    fn clipboard_updated(&self, result: &UpsertResult, kind: ClipboardKind) {
        if let Err(error) = self.0.emit(
            crate::clipboard::CLIPBOARD_UPDATED_EVENT,
            json!({
                "id": result.id,
                "kind": kind,
                "deduplicated": result.deduplicated,
            }),
        ) {
            log::warn!("emit remote clipboard update failed: {error}");
        }
    }
}

#[derive(Clone)]
pub struct Receiver {
    pool: SqlitePool,
    image_store: ImageStore,
    sensitive: Sensitive,
    sink: Arc<dyn EventSink>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReceiveOutcome {
    Stored,
    Duplicate,
}

#[derive(Debug, thiserror::Error)]
pub enum ReceiveError {
    #[error("clipboard sync request is invalid")]
    Invalid,
    #[error("clipboard sync request was replayed")]
    Replay,
    #[error("clipboard sync persistence failed")]
    Persistence,
}

impl Receiver {
    pub fn new(
        pool: SqlitePool,
        image_store: ImageStore,
        sensitive: Sensitive,
        sink: Arc<dyn EventSink>,
    ) -> Self {
        Self {
            pool,
            image_store,
            sensitive,
            sink,
        }
    }

    pub async fn receive(
        &self,
        peer_id: &str,
        auth: &VerifiedAuth,
        event: &SyncEvent,
        body_hash: &str,
    ) -> Result<ReceiveOutcome, ReceiveError> {
        if auth.device_id != event.origin_device_id {
            return Err(ReceiveError::Invalid);
        }
        let received_at =
            chrono::DateTime::from_timestamp(auth.timestamp, 0).ok_or(ReceiveError::Invalid)?;
        let record = ReplayRecord {
            peer_id,
            origin_device_id: &event.origin_device_id,
            event_id: &event.event_id,
            nonce: &auth.nonce,
            body_hash,
            received_at,
        };
        match persistence::reserve(&self.pool, &record).await {
            Ok(Reservation::Duplicate) => return Ok(ReceiveOutcome::Duplicate),
            Ok(Reservation::Fresh) => {}
            Err(persistence::ReplayError::Replay) => return Err(ReceiveError::Replay),
            Err(persistence::ReplayError::Storage(_)) => return Err(ReceiveError::Persistence),
        }

        let result = self.persist_event(event).await;
        if result.is_err() {
            if let Err(error) = persistence::release(&self.pool, &record).await {
                log::warn!("release failed sync replay reservation: {error}");
            }
        }
        result
    }

    async fn persist_event(&self, event: &SyncEvent) -> Result<ReceiveOutcome, ReceiveError> {
        let capture = Capture {
            text: true,
            html: true,
            rtf: true,
            image: true,
            files: false,
            max_text_mb: 16,
            max_image_mb: 12,
            ..Capture::default()
        };
        let item = build_item_with_settings(
            &self.image_store,
            &event.payload,
            &capture,
            &self.sensitive,
            false,
        )
        .map_err(|_| ReceiveError::Persistence)?
        .ok_or(ReceiveError::Invalid)?;
        let result = upsert_item(&self.pool, &item)
            .await
            .map_err(|_| ReceiveError::Persistence)?;
        self.sink.clipboard_updated(&result, item.kind);
        Ok(ReceiveOutcome::Stored)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;
    use crate::clipboard::{ClipboardPayload, ImagePayload};
    use crate::db::items::find_item_by_content_hash;
    use crate::db::models::ClipboardKind;
    use crate::db::test_support::memory_pool;

    struct CountingSink(AtomicUsize);

    impl EventSink for CountingSink {
        fn clipboard_updated(&self, _result: &UpsertResult, _kind: ClipboardKind) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn png() -> Vec<u8> {
        use image::ImageEncoder;
        let pixels = image::RgbaImage::from_pixel(2, 2, image::Rgba([1, 2, 3, 255]));
        let mut bytes = Vec::new();
        image::codecs::png::PngEncoder::new(&mut bytes)
            .write_image(pixels.as_raw(), 2, 2, image::ExtendedColorType::Rgba8)
            .unwrap();
        bytes
    }

    #[tokio::test]
    async fn remote_image_persists_once_without_clipboard_or_publisher_dependencies() {
        let pool = memory_pool().await;
        let root = tempfile::tempdir().unwrap();
        let store = ImageStore::for_test(root.path().join("images"));
        let sink = Arc::new(CountingSink(AtomicUsize::new(0)));
        let receiver = Receiver::new(
            pool.clone(),
            store.clone(),
            Sensitive::default(),
            sink.clone(),
        );
        let peer_id = uuid::Uuid::new_v4().to_string();
        let event = SyncEvent {
            origin_device_id: peer_id.clone(),
            event_id: uuid::Uuid::new_v4().to_string(),
            payload: ClipboardPayload::Image(ImagePayload {
                bytes: png(),
                width: 2,
                height: 2,
            }),
        };
        let auth = VerifiedAuth {
            device_id: peer_id.clone(),
            timestamp: chrono::Utc::now().timestamp(),
            nonce: "22".repeat(32),
        };

        assert_eq!(
            receiver
                .receive(&peer_id, &auth, &event, "wire-hash")
                .await
                .unwrap(),
            ReceiveOutcome::Stored
        );
        assert_eq!(
            receiver
                .receive(&peer_id, &auth, &event, "wire-hash")
                .await
                .unwrap(),
            ReceiveOutcome::Duplicate
        );
        let file_name = format!(
            "{}.png",
            blake3::hash(match &event.payload {
                ClipboardPayload::Image(image) => &image.bytes,
                ClipboardPayload::Text(_) | ClipboardPayload::Files(_) => unreachable!(),
            })
            .to_hex()
        );
        let hash = crate::db::items::content_hash(ClipboardKind::Image, &file_name);
        assert!(find_item_by_content_hash(&pool, &hash)
            .await
            .unwrap()
            .is_some());
        assert!(store.origin_path(&file_name).exists());
        assert_eq!(sink.0.load(Ordering::SeqCst), 1);
    }
}
