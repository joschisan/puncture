mod db;

use std::fs;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

use anyhow::Context;

use iroh::Endpoint;
use iroh::endpoint::{Connection, RelayMode};
use lightning::offers::offer::Offer;
use lightning_invoice::Bolt11Invoice;
use serde::{Serialize, de::DeserializeOwned};
use tokio::{sync::watch, task::AbortHandle};
use tracing::warn;

use puncture_client_core::{
    AppEvent, Bolt11ReceiveRequest, Bolt11ReceiveResponse, Bolt11SendRequest,
    Bolt12ReceiveResponse, Bolt12SendRequest, ClientRpcRequest, FeesResponse, RecoverRequest,
    RecoverResponse, RegisterRequest, RegisterResponse, SetRecoveryNameRequest,
};
use puncture_core::db::Database;
use puncture_core::{InviteCode, RecoveryCode, secret};

pub struct PunctureClient {
    endpoint: Endpoint,
    db: Database,
}

impl PunctureClient {
    /// Create a new puncture client
    pub async fn new(data_dir: String) -> Self {
        let data_dir = PathBuf::from(data_dir);

        fs::create_dir_all(&data_dir).expect("Failed to create data directory");

        let secret_key = secret::read_or_generate(&data_dir);

        let endpoint = Endpoint::builder()
            .secret_key(secret_key)
            .discovery_n0()
            .discovery_dht()
            .relay_mode(RelayMode::Disabled)
            .bind()
            .await
            .expect("Failed to bind iroh endpoint");

        let db = Database::new(&data_dir, puncture_client_db::MIGRATIONS, 3)
            .expect("Failed to setup database");

        Self { endpoint, db }
    }

    pub async fn register(&self, invite: InviteCode) -> Result<PunctureConnection, String> {
        let connection = self
            .endpoint
            .connect(invite.node_id(), b"puncture-api")
            .await
            .map_err(|_| "Failed to connect".to_string())?;

        let response: RegisterResponse = request_json(
            connection,
            "register",
            RegisterRequest {
                invite_id: invite.id(),
            },
        )
        .await
        .map_err(|_| "Failed to register".to_string())??;

        db::save_daemon(
            &mut *self.db.get_connection().await,
            invite.node_id(),
            response,
        )
        .await;

        Ok(PunctureConnection::new(
            self.endpoint.clone(),
            invite.node_id(),
        ))
    }

    pub async fn list_daemons(&self) -> Vec<Daemon> {
        db::list_daemons(&mut *self.db.get_connection().await)
            .await
            .into_iter()
            .map(|daemon| Daemon {
                endpoint: self.endpoint.clone(),
                node_id: iroh::NodeId::from_str(&daemon.node_id).unwrap(),
                name: daemon.name,
            })
            .collect()
    }

    pub async fn delete_daemon(&self, daemon: Daemon) {
        db::delete_daemon(&mut *self.db.get_connection().await, daemon.node_id).await;
    }

    pub async fn user_pk(&self) -> String {
        self.endpoint.secret_key().public().to_string()
    }
}

pub struct Daemon {
    endpoint: Endpoint,
    node_id: iroh::NodeId,
    name: String,
}

impl Daemon {
    pub fn name(&self) -> String {
        self.name.clone()
    }

    pub fn connect(&self) -> PunctureConnection {
        PunctureConnection::new(self.endpoint.clone(), self.node_id)
    }
}

/// The main client for communicating with the puncture daemon
#[derive(Clone, Debug)]
pub struct PunctureConnection {
    /// A channel to obtain a connection managed by the background task
    receiver: watch::Receiver<Option<Connection>>,
    /// A handle to the background task managing the connection
    handle: AbortHandle,
}

impl Drop for PunctureConnection {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

impl PunctureConnection {
    /// Create a new puncture connection
    pub fn new(endpoint: Endpoint, node_id: iroh::NodeId) -> Self {
        let (sender, receiver) = watch::channel(None);

        let handle = tokio::spawn(reconnect(endpoint, node_id, sender)).abort_handle();

        Self { receiver, handle }
    }

    /// Make a request to the daemon
    async fn request<R, T>(&self, method: &str, request: R) -> Result<T, String>
    where
        R: Serialize,
        T: DeserializeOwned,
    {
        let connection = self.receiver.borrow().clone().ok_or("Disconnected")?;

        request_json(connection, method, request)
            .await
            .map_err(|_| "Request failed".to_string())?
    }

    /// Create a bolt11 invoice for receiving payments
    pub async fn bolt11_receive(
        &self,
        amount_msat: u32,
        description: String,
    ) -> Result<Bolt11Invoice, String> {
        self.request(
            "bolt11_receive",
            Bolt11ReceiveRequest {
                amount_msat,
                description,
            },
        )
        .await
        .map(|response: Bolt11ReceiveResponse| response.invoice)
    }

    /// Send a bolt11 payment
    pub async fn bolt11_send(
        &self,
        invoice: Bolt11Invoice,
        amount_msat: u64,
        ln_address: Option<String>,
    ) -> Result<(), String> {
        self.request(
            "bolt11_send",
            Bolt11SendRequest {
                invoice: invoice.clone(),
                amount_msat,
                ln_address,
            },
        )
        .await
    }

    /// Create a amountless bolt12 offer for receiving payments
    pub async fn bolt12_receive_variable_amount(&self) -> Result<String, String> {
        self.request("bolt12_receive_variable_amount", ())
            .await
            .map(|response: Bolt12ReceiveResponse| response.offer)
    }

    /// Send a bolt12 payment
    pub async fn bolt12_send(&self, offer: Offer, amount_msat: u64) -> Result<(), String> {
        self.request(
            "bolt12_send",
            Bolt12SendRequest {
                offer: offer.to_string(),
                amount_msat,
            },
        )
        .await
    }

    /// Get the fees for a bolt11 payment
    pub async fn fees(&self) -> Result<FeesResponse, String> {
        self.request("fees", ()).await
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
        let connection = self.receiver.borrow().clone().context("Disconnected")?;

        let mut stream = connection.accept_uni().await?;

        let event = stream.read_to_end(100_000).await?;

        Ok(serde_json::from_slice(&event)?)
    }

    /// Set or clear the recovery name for this user
    pub async fn set_recovery_name(&self, recovery_name: Option<String>) -> Result<(), String> {
        self.request(
            "set_recovery_name",
            SetRecoveryNameRequest { recovery_name },
        )
        .await
    }

    /// Recover a balance from a recovery code
    pub async fn recover(&self, recovery_code: RecoveryCode) -> Result<u64, String> {
        self.request(
            "recover",
            RecoverRequest {
                recovery_id: recovery_code.id(),
            },
        )
        .await
        .map(|response: RecoverResponse| response.balance_msat)
    }
}

/// Background task that maintains a single connection to the daemon
async fn reconnect(
    endpoint: Endpoint,
    node_id: iroh::NodeId,
    sender: watch::Sender<Option<Connection>>,
) {
    let mut backoff = backoff_durations();

    loop {
        match endpoint.connect(node_id, b"puncture-api").await {
            Ok(connection) => {
                sender.send(Some(connection.clone())).ok();

                connection.closed().await;

                sender.send(None).ok();

                backoff = backoff_durations();
            }
            Err(e) => {
                warn!("Failed to connect to daemon: {}", e);
            }
        }

        tokio::time::sleep(backoff.next().unwrap()).await;
    }
}

fn backoff_durations() -> impl Iterator<Item = Duration> {
    (1..).map(|i| Duration::from_millis(std::cmp::min(i * i * 100, 10_000)))
}

async fn request_json<R, T>(
    connection: Connection,
    method: &str,
    request: R,
) -> anyhow::Result<Result<T, String>>
where
    R: Serialize,
    T: DeserializeOwned,
{
    let request = serde_json::to_vec(&ClientRpcRequest {
        method: method.to_string(),
        request,
    })
    .expect("Failed to serialize request");

    let (mut send_stream, mut recv_stream) = connection.open_bi().await?;

    send_stream.write_all(&request).await?;

    send_stream.finish()?;

    let response = recv_stream.read_to_end(1_000_000).await?;

    Ok(serde_json::from_slice(&response)?)
}
