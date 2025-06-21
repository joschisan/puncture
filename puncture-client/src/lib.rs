mod db;
mod models;
mod schema;

use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

use anyhow::Context;
use iroh::Endpoint;
use iroh::endpoint::{Connection, RelayMode};
use serde::{Serialize, de::DeserializeOwned};
use tokio::sync::{mpsc, oneshot};
use tracing::{info, warn};

use puncture_api_core::{
    AppEvent, Bolt11QuoteResponse, Bolt11ReceiveResponse, ConfigResponse, UserBolt11QuoteRequest,
    UserBolt11ReceiveRequest, UserBolt11SendRequest,
};
use puncture_core::db::{DbConnection, setup_database};
use puncture_core::{invite, secret};

/// A helper struct for our JSON-RPC requests
#[derive(Serialize, Debug)]
struct IrohApiRequest<R> {
    method: String,
    request: R,
}

pub struct PunctureClient {
    endpoint: Endpoint,
    db: DbConnection,
}

impl PunctureClient {
    /// Create a new puncture client
    pub async fn new(data_dir: String) -> Self {
        let data_dir = PathBuf::from(data_dir);

        assert!(data_dir.is_dir(), "Puncture data dir is not a directory");

        assert!(data_dir.exists(), "Puncture data dir does not exist");

        let secret_key = secret::read_or_generate(&data_dir);

        let endpoint = Endpoint::builder()
            .secret_key(secret_key)
            .discovery_n0()
            .relay_mode(RelayMode::Disabled)
            .bind()
            .await
            .expect("Failed to bind iroh endpoint");

        let db = setup_database(&data_dir, db::MIGRATIONS).expect("Failed to setup database");

        Self { endpoint, db }
    }

    pub async fn add_instance(&self, invite: String) -> Result<PunctureConnection, String> {
        let node_id = invite::decode(&invite).map_err(|_| "Invalid invite".to_string())?;

        let connection = PunctureConnection::new(self.endpoint.clone(), node_id);

        db::save_instance_config(&self.db, node_id.to_string(), connection.config().await?);

        Ok(connection)
    }

    pub fn get_instances(&self) -> Vec<Instance> {
        db::get_instances(&self.db)
            .into_iter()
            .map(|instance| Instance {
                endpoint: self.endpoint.clone(),
                node_id: iroh::NodeId::from_str(&instance.node_id).unwrap(),
                name: instance.name,
            })
            .collect()
    }
}

pub struct Instance {
    endpoint: Endpoint,
    node_id: iroh::NodeId,
    name: String,
}

impl Instance {
    pub fn name(&self) -> String {
        self.name.clone()
    }

    pub fn invite(&self) -> String {
        invite::encode(&self.node_id)
    }

    pub fn connect(&self) -> PunctureConnection {
        PunctureConnection::new(self.endpoint.clone(), self.node_id)
    }
}

/// The main client for communicating with the puncture daemon
#[derive(Clone, Debug)]
pub struct PunctureConnection {
    /// A channel to ask the background task for a connection
    connection_tx: mpsc::Sender<oneshot::Sender<Connection>>,
}

impl PunctureConnection {
    /// Create a new puncture connection
    pub fn new(endpoint: Endpoint, node_id: iroh::NodeId) -> Self {
        let (connection_tx, connection_rx) = mpsc::channel(8);

        tokio::spawn(manage_connection(endpoint, node_id, connection_rx));

        Self { connection_tx }
    }

    /// Get a connection from the background task
    async fn get_connection(&self) -> anyhow::Result<iroh::endpoint::Connection> {
        let (tx, rx) = oneshot::channel();

        self.connection_tx
            .send(tx)
            .await
            .context("Connection manager task has shut down")?;

        rx.await.context("Failed to receive connection")
    }

    /// Make a request to the daemon
    async fn request<R, T>(&self, method: &str, request: R) -> anyhow::Result<Result<T, String>>
    where
        R: Serialize,
        T: DeserializeOwned,
    {
        let connection = self.get_connection().await?;

        let request = IrohApiRequest {
            method: method.to_string(),
            request,
        };

        let request = serde_json::to_vec(&request).expect("Failed to serialize request");

        let (mut send_stream, mut recv_stream) = connection
            .open_bi()
            .await
            .context("Failed to open bidirectional stream")?;

        send_stream
            .write_all(&request)
            .await
            .context("Failed to write request")?;

        send_stream
            .finish()
            .context("Failed to finish send stream")?;

        let response = recv_stream
            .read_to_end(1_000_000)
            .await
            .context("Failed to read response")?;

        serde_json::from_slice(&response).context("Failed to deserialize response")
    }

    /// Get the name of the puncture instance
    async fn config(&self) -> Result<ConfigResponse, String> {
        self.request("config", ())
            .await
            .map_err(|e| format!("Transport error: {}", e))?
    }

    /// Create a bolt11 invoice for receiving payments
    pub async fn bolt11_receive(
        &self,
        amount_msat: u32,
        description: Option<String>,
    ) -> Result<String, String> {
        let response: Bolt11ReceiveResponse = self
            .request(
                "bolt11_receive",
                UserBolt11ReceiveRequest {
                    amount_msat,
                    description,
                },
            )
            .await
            .map_err(|e| format!("Transport error: {}", e))??;

        Ok(response.invoice.to_string())
    }

    /// Send a bolt11 payment
    pub async fn bolt11_send(
        &self,
        invoice: String,
        ln_address: Option<String>,
    ) -> Result<(), String> {
        let invoice = invoice.parse().map_err(|_| "Invalid invoice".to_string())?;

        self.request(
            "bolt11_send",
            UserBolt11SendRequest {
                invoice,
                ln_address,
            },
        )
        .await
        .map_err(|e| format!("Transport error: {}", e))?
    }

    /// Quote a bolt11 payment (get fees and details)
    pub async fn bolt11_quote(&self, invoice: String) -> Result<Bolt11QuoteResponse, String> {
        let invoice = invoice.parse().map_err(|_| "Invalid invoice".to_string())?;

        self.request("bolt11_quote", UserBolt11QuoteRequest { invoice })
            .await
            .map_err(|e| format!("Transport error: {}", e))?
    }

    /// Awaits the next event from the daemon
    pub async fn next_event(&self) -> AppEvent {
        loop {
            match self.accept_event().await {
                Ok(event) => return event,
                Err(e) => warn!("Failed to accept event: {}", e),
            }

            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }

    async fn accept_event(&self) -> anyhow::Result<AppEvent> {
        let connection = self.get_connection().await?;

        let mut stream = connection.accept_uni().await?;

        let event = stream.read_to_end(100_000).await?;

        Ok(serde_json::from_slice(&event)?)
    }
}

/// Background task that maintains a single connection to the daemon
async fn manage_connection(
    endpoint: Endpoint,
    node_id: iroh::NodeId,
    mut rx: mpsc::Receiver<oneshot::Sender<Connection>>,
) {
    info!("Starting connection manager task");

    let mut senders: Vec<oneshot::Sender<Connection>> = Vec::new();

    let mut backoff = backoff_durations();

    loop {
        info!("Attempting to connect to daemon");

        if let Ok(connection) = endpoint.connect(node_id, b"puncture-api").await {
            info!("Connection established with daemon");

            for sender in senders.drain(..) {
                sender.send(connection.clone()).ok();
            }

            loop {
                tokio::select! {
                    sender = rx.recv() => {
                        match sender {
                            Some(sender) => sender.send(connection.clone()).ok(),
                            None => return, // We are shutting down
                        };
                    }
                    _ = connection.closed() => {
                        break;
                    }
                }
            }

            backoff = backoff_durations();
        } else {
            warn!("Failed to connect to daemon");
        }

        tokio::select! {
            sender = rx.recv() => {
                match sender {
                    Some(sender) => senders.push(sender),
                    None => return, // We are shutting down
                };
            },
            _ = tokio::time::sleep(backoff.next().unwrap()) => {},
        }
    }
}

fn backoff_durations() -> impl Iterator<Item = Duration> {
    (1..).map(|i| Duration::from_millis(std::cmp::min(i * i * 100, 10_000)))
}
