CREATE TABLE sync_received_events (
    peer_id          TEXT NOT NULL,
    origin_device_id TEXT NOT NULL,
    event_id         TEXT NOT NULL,
    nonce            TEXT NOT NULL,
    body_hash        TEXT NOT NULL,
    expires_at       TEXT NOT NULL,
    created_at       TEXT NOT NULL,
    updated_at       TEXT NOT NULL,
    PRIMARY KEY (peer_id, origin_device_id, event_id, nonce),
    UNIQUE (peer_id, nonce),
    UNIQUE (peer_id, origin_device_id, event_id)
);

CREATE INDEX idx_sync_received_events_expiry ON sync_received_events (expires_at);
