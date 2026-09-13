use chrono::{DateTime, Duration, Utc};
use sqlx::SqlitePool;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reservation {
    Fresh,
    Duplicate,
}

#[derive(Debug, thiserror::Error)]
pub enum ReplayError {
    #[error("clipboard sync request was replayed")]
    Replay,
    #[error("clipboard sync replay state is unavailable")]
    Storage(#[from] sqlx::Error),
}

pub struct ReplayRecord<'a> {
    pub peer_id: &'a str,
    pub origin_device_id: &'a str,
    pub event_id: &'a str,
    pub nonce: &'a str,
    pub body_hash: &'a str,
    pub received_at: DateTime<Utc>,
}

pub async fn reserve(
    pool: &SqlitePool,
    record: &ReplayRecord<'_>,
) -> Result<Reservation, ReplayError> {
    let mut transaction = pool.begin().await?;
    sqlx::query("DELETE FROM sync_received_events WHERE expires_at < ?")
        .bind(record.received_at)
        .execute(&mut *transaction)
        .await?;

    let nonce_row = sqlx::query_as::<_, (String, String, String)>(
        "SELECT origin_device_id, event_id, body_hash FROM sync_received_events \
         WHERE peer_id = ? AND nonce = ?",
    )
    .bind(record.peer_id)
    .bind(record.nonce)
    .fetch_optional(&mut *transaction)
    .await?;
    if let Some((origin, event, body_hash)) = nonce_row {
        transaction.commit().await?;
        return if origin == record.origin_device_id
            && event == record.event_id
            && body_hash == record.body_hash
        {
            Ok(Reservation::Duplicate)
        } else {
            Err(ReplayError::Replay)
        };
    }

    let event_hash = sqlx::query_scalar::<_, String>(
        "SELECT body_hash FROM sync_received_events \
         WHERE peer_id = ? AND origin_device_id = ? AND event_id = ?",
    )
    .bind(record.peer_id)
    .bind(record.origin_device_id)
    .bind(record.event_id)
    .fetch_optional(&mut *transaction)
    .await?;
    if let Some(existing_hash) = event_hash {
        transaction.commit().await?;
        return if existing_hash == record.body_hash {
            Ok(Reservation::Duplicate)
        } else {
            Err(ReplayError::Replay)
        };
    }

    let expires_at = record.received_at + Duration::hours(24);
    sqlx::query(
        "INSERT INTO sync_received_events \
         (peer_id, origin_device_id, event_id, nonce, body_hash, expires_at, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(record.peer_id)
    .bind(record.origin_device_id)
    .bind(record.event_id)
    .bind(record.nonce)
    .bind(record.body_hash)
    .bind(expires_at)
    .bind(record.received_at)
    .bind(record.received_at)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(Reservation::Fresh)
}

pub async fn release(pool: &SqlitePool, record: &ReplayRecord<'_>) -> Result<(), sqlx::Error> {
    sqlx::query(
        "DELETE FROM sync_received_events WHERE peer_id = ? AND origin_device_id = ? \
         AND event_id = ? AND nonce = ?",
    )
    .bind(record.peer_id)
    .bind(record.origin_device_id)
    .bind(record.event_id)
    .bind(record.nonce)
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_support::memory_pool;

    #[tokio::test]
    async fn duplicate_event_is_idempotent_but_changed_body_replay_is_rejected() {
        let pool = memory_pool().await;
        let now = Utc::now();
        let first = ReplayRecord {
            peer_id: "peer",
            origin_device_id: "origin",
            event_id: "event",
            nonce: "nonce",
            body_hash: "hash-a",
            received_at: now,
        };
        assert_eq!(reserve(&pool, &first).await.unwrap(), Reservation::Fresh);
        assert_eq!(
            reserve(&pool, &first).await.unwrap(),
            Reservation::Duplicate
        );

        let changed = ReplayRecord {
            body_hash: "hash-b",
            ..first
        };
        assert!(matches!(
            reserve(&pool, &changed).await,
            Err(ReplayError::Replay)
        ));
    }

    #[tokio::test]
    async fn same_event_with_a_new_nonce_is_idempotent() {
        let pool = memory_pool().await;
        let now = Utc::now();
        let first = ReplayRecord {
            peer_id: "peer",
            origin_device_id: "origin",
            event_id: "event",
            nonce: "nonce-a",
            body_hash: "hash",
            received_at: now,
        };
        assert_eq!(reserve(&pool, &first).await.unwrap(), Reservation::Fresh);
        let second = ReplayRecord {
            nonce: "nonce-b",
            ..first
        };
        assert_eq!(
            reserve(&pool, &second).await.unwrap(),
            Reservation::Duplicate
        );
    }
}
