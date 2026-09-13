use std::sync::atomic::AtomicUsize;

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::post;
use axum::Router;

use super::*;

#[test]
fn queue_rejects_event_count_and_byte_overflow() {
    let (count_queue, _receiver) = OutboundQueue::with_limits(2, 100);
    assert!(count_queue.enqueue(Bytes::from_static(b"a")));
    assert!(count_queue.enqueue(Bytes::from_static(b"b")));
    assert!(!count_queue.enqueue(Bytes::from_static(b"c")));

    let (byte_queue, _receiver) = OutboundQueue::with_limits(4, 3);
    assert!(byte_queue.enqueue(Bytes::from_static(b"abc")));
    assert!(!byte_queue.enqueue(Bytes::from_static(b"d")));
}

async fn start_test_server(handler: Router) -> (std::net::SocketAddr, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        axum::serve(listener, handler).await.unwrap();
    });
    (address, task)
}

fn test_worker(address: std::net::SocketAddr, client: reqwest::Client) -> PeerWorker {
    PeerWorker {
        peer_id: uuid::Uuid::new_v4().to_string(),
        address: address.ip(),
        port: address.port(),
        local_device_id: uuid::Uuid::new_v4().to_string(),
        key: PeerKey::derive("test retry secret value"),
        client,
    }
}

#[tokio::test]
async fn worker_retries_a_server_error_then_succeeds() {
    async fn handler(State(attempts): State<Arc<AtomicUsize>>) -> StatusCode {
        if attempts.fetch_add(1, Ordering::SeqCst) == 0 {
            StatusCode::INTERNAL_SERVER_ERROR
        } else {
            StatusCode::OK
        }
    }
    let attempts = Arc::new(AtomicUsize::new(0));
    let router = Router::new()
        .route(EVENT_PATH, post(handler))
        .with_state(Arc::clone(&attempts));
    let (address, server) = start_test_server(router).await;
    let worker = test_worker(address, build_http_client().unwrap());
    let (_shutdown_sender, mut shutdown) = watch::channel(false);

    assert!(
        send_with_retry(&worker, Bytes::from_static(b"event"), &mut shutdown)
            .await
            .is_ok()
    );
    assert_eq!(attempts.load(Ordering::SeqCst), 2);
    server.abort();
}

#[tokio::test]
async fn worker_request_timeout_has_bounded_retries() {
    async fn handler(State(attempts): State<Arc<AtomicUsize>>) -> StatusCode {
        attempts.fetch_add(1, Ordering::SeqCst);
        tokio::time::sleep(Duration::from_millis(100)).await;
        StatusCode::OK
    }
    let attempts = Arc::new(AtomicUsize::new(0));
    let router = Router::new()
        .route(EVENT_PATH, post(handler))
        .with_state(Arc::clone(&attempts));
    let (address, server) = start_test_server(router).await;
    let client =
        build_http_client_with_timeouts(Duration::from_millis(20), Duration::from_millis(20))
            .unwrap();
    let worker = test_worker(address, client);
    let (_shutdown_sender, mut shutdown) = watch::channel(false);

    assert!(
        send_with_retry(&worker, Bytes::from_static(b"event"), &mut shutdown)
            .await
            .is_err()
    );
    assert_eq!(attempts.load(Ordering::SeqCst), 3);
    server.abort();
}

#[tokio::test]
async fn configured_client_does_not_follow_redirects() {
    use axum::response::Redirect;

    async fn redirect() -> Redirect {
        Redirect::temporary("/unexpected")
    }
    async fn unexpected(State(visits): State<Arc<AtomicUsize>>) -> StatusCode {
        visits.fetch_add(1, Ordering::SeqCst);
        StatusCode::OK
    }
    let visits = Arc::new(AtomicUsize::new(0));
    let router = Router::new()
        .route(EVENT_PATH, post(redirect))
        .route("/unexpected", post(unexpected))
        .with_state(Arc::clone(&visits));
    let (address, server) = start_test_server(router).await;
    let worker = test_worker(address, build_http_client().unwrap());
    let (_shutdown_sender, mut shutdown) = watch::channel(false);

    assert!(
        send_with_retry(&worker, Bytes::from_static(b"event"), &mut shutdown)
            .await
            .is_err()
    );
    assert_eq!(visits.load(Ordering::SeqCst), 0);
    server.abort();
}
