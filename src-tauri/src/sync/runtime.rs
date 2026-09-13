use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use anyhow::{anyhow, Context};
use axum::body::Bytes;
use sqlx::SqlitePool;
use tokio::sync::{watch, Mutex};
use tokio::task::{JoinHandle, JoinSet};

use super::auth::PeerKey;
use super::client::{build_http_client, run_worker, OutboundQueue, PeerWorker};
use super::protocol::SyncEvent;
use super::receiver::{EventSink, Receiver};
use super::secrets::SharedSecretStore;
use super::server::{self, IncomingPeer, ServerState};
use super::{parse_sync_address, validate_configuration, validate_configuration_with_loopback};
use crate::clipboard::{ClipboardPayload, ImageStore};
use crate::core::{AppError, Result};
use crate::settings::SyncSettings;

#[derive(Clone)]
pub struct SyncRuntime {
    state: Arc<RuntimeState>,
}

struct RuntimeState {
    running: Mutex<Option<RunningRuntime>>,
    publisher: RwLock<Option<Publisher>>,
    secrets: SharedSecretStore,
}

struct RunningRuntime {
    shutdown: watch::Sender<bool>,
    supervisor: JoinHandle<()>,
    #[cfg(test)]
    local_addr: std::net::SocketAddr,
}

struct Publisher {
    device_id: String,
    queues: Vec<OutboundQueue>,
}

impl SyncRuntime {
    pub fn new(secrets: SharedSecretStore) -> Self {
        Self {
            state: Arc::new(RuntimeState {
                running: Mutex::new(None),
                publisher: RwLock::new(None),
                secrets,
            }),
        }
    }

    pub fn publish_local(&self, payload: &ClipboardPayload) -> usize {
        let publisher = self
            .state
            .publisher
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let Some(publisher) = publisher.as_ref() else {
            return 0;
        };
        let Some(event) = SyncEvent::from_local(&publisher.device_id, payload) else {
            return 0;
        };
        let Ok(body) = event.encode() else {
            return 0;
        };
        let body = Bytes::from(body);
        publisher
            .queues
            .iter()
            .filter(|queue| queue.enqueue(body.clone()))
            .count()
    }

    pub fn set_peer_secret(&self, reference: &str, secret: &str) -> Result<()> {
        if secret.len() < 16 || secret.len() > 1_024 {
            return Err(secret_error());
        }
        self.state
            .secrets
            .set(reference, secret)
            .map_err(|_| secret_error())
    }

    pub fn delete_peer_secret(&self, reference: &str) -> Result<()> {
        self.state
            .secrets
            .delete(reference)
            .map_err(|_| secret_error())
    }

    pub fn test_peer_secret(&self, reference: &str) -> Result<()> {
        let secret = self
            .state
            .secrets
            .get(reference)
            .map_err(|_| secret_error())?;
        let _key = PeerKey::derive(secret.expose());
        Ok(())
    }

    pub(super) async fn configure(&self, input: RuntimeInput, allow_loopback: bool) -> Result<()> {
        if allow_loopback {
            validate_configuration_with_loopback(&input.settings, true)?;
        } else {
            validate_configuration(&input.settings)?;
        }
        self.stop().await;
        if !input.settings.enabled {
            return Ok(());
        }

        let bind_ip = parse_sync_address(&input.settings.bind_address, allow_loopback)?;
        let listener =
            tokio::net::TcpListener::bind(SocketAddr::new(bind_ip, input.settings.listen_port))
                .await
                .context("failed to bind configured sync address")?;
        #[cfg(test)]
        let local_addr = listener
            .local_addr()
            .context("failed to read sync listener address")?;
        let client = build_http_client().context("failed to create sync HTTP client")?;

        let mut incoming = Vec::with_capacity(input.settings.peers.len());
        let mut workers = Vec::with_capacity(input.settings.peers.len());
        let mut queues = Vec::with_capacity(input.settings.peers.len());
        for peer in &input.settings.peers {
            let secret = self
                .state
                .secrets
                .get(&peer.secret_reference)
                .map_err(|_| secret_error())?;
            let key = PeerKey::derive(secret.expose());
            let address = parse_sync_address(&peer.address, allow_loopback)?;
            incoming.push(IncomingPeer {
                peer_id: peer.id.clone(),
                address,
                key: key.clone(),
            });
            let (queue, receiver) = OutboundQueue::new();
            queues.push(queue);
            workers.push((
                PeerWorker {
                    peer_id: peer.id.clone(),
                    address,
                    port: peer.port,
                    local_device_id: input.settings.device_id.clone(),
                    key,
                    client: client.clone(),
                },
                receiver,
            ));
        }

        let receiver = Receiver::new(input.pool, input.image_store, input.sensitive, input.sink);
        let server_state = ServerState::new(incoming, receiver);
        let (shutdown, shutdown_receiver) = watch::channel(false);
        let supervisor_shutdown = shutdown.clone();
        let supervisor = tokio::spawn(async move {
            let mut tasks = JoinSet::new();
            tasks.spawn(async move {
                server::serve(listener, server_state, shutdown_receiver)
                    .await
                    .map_err(|error| error.to_string())
            });
            for (worker, receiver) in workers {
                let shutdown = supervisor_shutdown.subscribe();
                tasks.spawn(async move {
                    run_worker(worker, receiver, shutdown).await;
                    Ok::<(), String>(())
                });
            }
            while let Some(result) = tasks.join_next().await {
                match result {
                    Ok(Ok(())) => {}
                    Ok(Err(error)) => log::warn!("clipboard sync task stopped: {error}"),
                    Err(error) => log::warn!("clipboard sync task failed: {error}"),
                }
                if *supervisor_shutdown.borrow() {
                    break;
                }
            }
            tasks.abort_all();
        });

        *self
            .state
            .publisher
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(Publisher {
            device_id: input.settings.device_id,
            queues,
        });
        *self.state.running.lock().await = Some(RunningRuntime {
            shutdown,
            supervisor,
            #[cfg(test)]
            local_addr,
        });
        Ok(())
    }

    pub(crate) async fn stop(&self) {
        *self
            .state
            .publisher
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
        let Some(mut running) = self.state.running.lock().await.take() else {
            return;
        };
        let _ = running.shutdown.send(true);
        if tokio::time::timeout(Duration::from_secs(2), &mut running.supervisor)
            .await
            .is_err()
        {
            running.supervisor.abort();
        }
    }

    #[cfg(test)]
    pub async fn configure_for_test(&self, input: RuntimeInput) -> Result<std::net::SocketAddr> {
        self.configure(input, true).await?;
        self.state
            .running
            .lock()
            .await
            .as_ref()
            .map(|running| running.local_addr)
            .ok_or_else(|| AppError::Other(anyhow!("test sync runtime did not start")))
    }
}

pub struct RuntimeInput {
    pub settings: SyncSettings,
    pub pool: SqlitePool,
    pub image_store: ImageStore,
    pub sensitive: crate::settings::Sensitive,
    pub sink: Arc<dyn EventSink>,
}

fn secret_error() -> AppError {
    AppError::Other(anyhow!("sync peer credential operation failed"))
}
