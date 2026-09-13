use std::net::IpAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use axum::body::Bytes;
use chrono::Utc;
use tokio::sync::{mpsc, watch};

use super::auth::{sign_headers, PeerKey, EVENT_PATH};

pub const MAX_QUEUE_EVENTS: usize = 128;
pub const MAX_QUEUE_BYTES: usize = 64 * 1024 * 1024;
const RETRY_DELAYS: [Duration; 2] = [Duration::from_millis(250), Duration::from_secs(1)];
const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone)]
pub struct OutboundQueue {
    sender: mpsc::Sender<QueuedEvent>,
    queued_bytes: Arc<AtomicUsize>,
    max_bytes: usize,
}

pub(super) struct QueuedEvent {
    body: Bytes,
    size: usize,
    queued_bytes: Arc<AtomicUsize>,
}

pub struct PeerWorker {
    pub peer_id: String,
    pub address: IpAddr,
    pub port: u16,
    pub local_device_id: String,
    pub key: PeerKey,
    pub client: reqwest::Client,
}

impl OutboundQueue {
    pub(super) fn new() -> (Self, mpsc::Receiver<QueuedEvent>) {
        Self::with_limits(MAX_QUEUE_EVENTS, MAX_QUEUE_BYTES)
    }

    fn with_limits(max_events: usize, max_bytes: usize) -> (Self, mpsc::Receiver<QueuedEvent>) {
        let (sender, receiver) = mpsc::channel(max_events);
        (
            Self {
                sender,
                queued_bytes: Arc::new(AtomicUsize::new(0)),
                max_bytes,
            },
            receiver,
        )
    }

    pub fn enqueue(&self, body: Bytes) -> bool {
        let size = body.len();
        if !self.reserve_bytes(size) {
            return false;
        }
        let event = QueuedEvent {
            body,
            size,
            queued_bytes: Arc::clone(&self.queued_bytes),
        };
        if self.sender.try_send(event).is_err() {
            self.queued_bytes.fetch_sub(size, Ordering::AcqRel);
            return false;
        }
        true
    }

    fn reserve_bytes(&self, size: usize) -> bool {
        self.queued_bytes
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                current
                    .checked_add(size)
                    .filter(|next| *next <= self.max_bytes)
            })
            .is_ok()
    }
}

pub(super) fn build_http_client() -> Result<reqwest::Client, reqwest::Error> {
    build_http_client_with_timeouts(CONNECT_TIMEOUT, REQUEST_TIMEOUT)
}

fn build_http_client_with_timeouts(
    connect_timeout: Duration,
    request_timeout: Duration,
) -> Result<reqwest::Client, reqwest::Error> {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .connect_timeout(connect_timeout)
        .timeout(request_timeout)
        .build()
}

pub(super) async fn run_worker(
    worker: PeerWorker,
    mut receiver: mpsc::Receiver<QueuedEvent>,
    mut shutdown: watch::Receiver<bool>,
) {
    loop {
        let event = tokio::select! {
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    return;
                }
                continue;
            }
            event = receiver.recv() => match event {
                Some(event) => event,
                None => return,
            }
        };
        event.queued_bytes.fetch_sub(event.size, Ordering::AcqRel);
        if let Err(error) = send_with_retry(&worker, event.body, &mut shutdown).await {
            log::warn!(
                "clipboard sync send to peer {} failed: {error}",
                worker.peer_id
            );
        }
    }
}

async fn send_with_retry(
    worker: &PeerWorker,
    body: Bytes,
    shutdown: &mut watch::Receiver<bool>,
) -> Result<(), &'static str> {
    let timestamp = Utc::now().timestamp();
    let headers = sign_headers(&worker.key, &worker.local_device_id, timestamp, &body)
        .map_err(|_| "authentication unavailable")?;
    let endpoint = endpoint(worker.address, worker.port);
    let mut retry_delays = RETRY_DELAYS.into_iter();
    loop {
        let response = worker
            .client
            .post(&endpoint)
            .headers(headers.clone())
            .body(body.clone())
            .send()
            .await;
        match response {
            Ok(response) if response.status().is_success() => return Ok(()),
            Ok(response) if !response.status().is_server_error() => {
                return Err("peer rejected event");
            }
            Ok(_) | Err(_) => {
                let Some(retry_delay) = retry_delays.next() else {
                    return Err("peer unavailable");
                };
                tokio::select! {
                    changed = shutdown.changed() => {
                        if changed.is_err() || *shutdown.borrow() {
                            return Err("shutdown");
                        }
                    }
                    () = tokio::time::sleep(retry_delay) => {}
                }
            }
        }
    }
}

fn endpoint(address: IpAddr, port: u16) -> String {
    match address {
        IpAddr::V4(address) => format!("http://{address}:{port}{EVENT_PATH}"),
        IpAddr::V6(address) => format!("http://[{address}]:{port}{EVENT_PATH}"),
    }
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
