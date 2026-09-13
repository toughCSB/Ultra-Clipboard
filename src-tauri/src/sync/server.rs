use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};

use axum::body::Bytes;
use axum::extract::{ConnectInfo, DefaultBodyLimit, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::post;
use axum::Router;
use tokio::net::TcpListener;
use tokio::sync::watch;

use super::auth::{body_hash, verify_headers, PeerKey, EVENT_PATH, HEADER_DEVICE};
use super::protocol::{SyncEvent, MAX_EVENT_BYTES};
use super::receiver::{ReceiveError, Receiver};

#[derive(Clone)]
pub struct IncomingPeer {
    pub peer_id: String,
    pub address: IpAddr,
    pub key: PeerKey,
}

#[derive(Clone)]
pub struct ServerState {
    peers: HashMap<String, IncomingPeer>,
    receiver: Receiver,
}

impl ServerState {
    pub fn new(peers: Vec<IncomingPeer>, receiver: Receiver) -> Self {
        Self {
            peers: peers
                .into_iter()
                .map(|peer| (peer.peer_id.clone(), peer))
                .collect(),
            receiver,
        }
    }
}

pub async fn serve(
    listener: TcpListener,
    state: ServerState,
    mut shutdown: watch::Receiver<bool>,
) -> std::io::Result<()> {
    let router = Router::new()
        .route(EVENT_PATH, post(receive_event))
        .layer(DefaultBodyLimit::max(MAX_EVENT_BYTES))
        .with_state(state);
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(async move {
        while shutdown.changed().await.is_ok() {
            if *shutdown.borrow() {
                break;
            }
        }
    })
    .await
}

async fn receive_event(
    State(state): State<ServerState>,
    ConnectInfo(remote): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    body: Bytes,
) -> StatusCode {
    match receive_authenticated(&state, remote.ip(), &headers, &body).await {
        Ok(()) => StatusCode::OK,
        Err(ServerError::Authentication) => StatusCode::UNAUTHORIZED,
        Err(ServerError::Invalid) => StatusCode::UNPROCESSABLE_ENTITY,
        Err(ServerError::Replay) => StatusCode::CONFLICT,
        Err(ServerError::Persistence) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

#[derive(Debug)]
enum ServerError {
    Authentication,
    Invalid,
    Replay,
    Persistence,
}

async fn receive_authenticated(
    state: &ServerState,
    remote_ip: IpAddr,
    headers: &HeaderMap,
    body: &[u8],
) -> Result<(), ServerError> {
    let device_id = headers
        .get(HEADER_DEVICE)
        .and_then(|value| value.to_str().ok())
        .ok_or(ServerError::Authentication)?;
    let peer = state
        .peers
        .get(device_id)
        .ok_or(ServerError::Authentication)?;
    if peer.address != remote_ip {
        return Err(ServerError::Authentication);
    }
    let auth = verify_headers(&peer.key, headers, body, chrono::Utc::now().timestamp())
        .map_err(|_| ServerError::Authentication)?;
    let event = SyncEvent::decode(body).map_err(|_| ServerError::Invalid)?;
    state
        .receiver
        .receive(&peer.peer_id, &auth, &event, &body_hash(body))
        .await
        .map(|_| ())
        .map_err(|error| match error {
            ReceiveError::Invalid => ServerError::Invalid,
            ReceiveError::Replay => ServerError::Replay,
            ReceiveError::Persistence => ServerError::Persistence,
        })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::clipboard::ImageStore;
    use crate::db::test_support::memory_pool;
    use crate::settings::Sensitive;
    use crate::sync::auth::sign_headers_with_nonce;
    use crate::sync::receiver::{EventSink, Receiver};

    struct SilentSink;
    impl EventSink for SilentSink {
        fn clipboard_updated(
            &self,
            _result: &crate::db::items::UpsertResult,
            _kind: crate::db::models::ClipboardKind,
        ) {
        }
    }

    #[tokio::test]
    async fn unknown_peer_and_wrong_source_ip_are_rejected() {
        let pool = memory_pool().await;
        let dir = tempfile::tempdir().unwrap();
        let receiver = Receiver::new(
            pool,
            ImageStore::for_test(dir.path().join("images")),
            Sensitive::default(),
            Arc::new(SilentSink),
        );
        let peer_id = uuid::Uuid::new_v4().to_string();
        let key = PeerKey::derive("test secret with enough entropy");
        let state = ServerState::new(
            vec![IncomingPeer {
                peer_id: peer_id.clone(),
                address: "127.0.0.1".parse().unwrap(),
                key: key.clone(),
            }],
            receiver,
        );
        let nonce = "11".repeat(32);
        let headers = sign_headers_with_nonce(
            &key,
            &peer_id,
            chrono::Utc::now().timestamp(),
            &nonce,
            b"body",
        )
        .unwrap();
        assert!(matches!(
            receive_authenticated(&state, "127.0.0.2".parse().unwrap(), &headers, b"body").await,
            Err(ServerError::Authentication)
        ));

        let unknown = uuid::Uuid::new_v4().to_string();
        let headers = sign_headers_with_nonce(
            &key,
            &unknown,
            chrono::Utc::now().timestamp(),
            &nonce,
            b"body",
        )
        .unwrap();
        assert!(matches!(
            receive_authenticated(&state, "127.0.0.1".parse().unwrap(), &headers, b"body").await,
            Err(ServerError::Authentication)
        ));
    }
}
