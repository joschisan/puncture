mod api;
mod cli;
mod db;
mod events;
mod models;
mod schema;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result, ensure};
use std::sync::atomic::AtomicUsize;

use clap::{ArgGroup, Parser};
use dashmap::DashMap;
use iroh::Endpoint;
use ldk_node::bitcoin::Network;
use ldk_node::{Builder, Event, Node};
use tokio::net::TcpListener;
use tracing::{info, warn};
use url::Url;

use puncture_core::db::{DbConnection, setup_database};
use puncture_core::{invite, secret};

use crate::db::unix_time;
use crate::events::EventBus;
use crate::models::Bolt11Receive;

#[derive(Parser, Debug, Clone)]
#[command(group(
    ArgGroup::new("chain_source")
        .required(true)
        .multiple(false)
        .args(["bitcoind_rpc_url", "esplora_rpc_url"])
))]
struct Args {
    /// Bearer token for admin API access, used to authenticate administrative operations.
    #[arg(long, env = "ADMIN_AUTH")]
    admin_auth: String,

    /// Directory path for storing user account data in a SQLite database.
    #[arg(long, env = "PUNCTURE_DATA_DIR")]
    puncture_data_dir: PathBuf,

    /// Directory path for storing LDK node data in a SQLite database.
    #[arg(long, env = "LDK_DATA_DIR")]
    ldk_data_dir: PathBuf,

    /// Bitcoin network to operate on, determines address formats and chain validation rules.
    #[arg(long, env = "BITCOIN_NETWORK")]
    bitcoin_network: Network,

    /// Bitcoin Core RPC URL for chain data access. Mutually exclusive with --esplora-rpc-url.
    #[arg(long, env = "BITCOIN_RPC_URL")]
    bitcoind_rpc_url: Option<Url>,

    /// Esplora API URL for chain data access. Mutually exclusive with --bitcoind-rpc-url.
    #[arg(long, env = "ESPLORA_RPC_URL")]
    esplora_rpc_url: Option<Url>,

    /// Name of the puncture instance as displayed to the user
    #[arg(long, env = "INSTANCE_NAME")]
    instance_name: String,

    /// Fee rate in parts per million (PPM) applied to outgoing Lightning payments.
    #[arg(long, env = "FEE_PPM", default_value = "10000")]
    fee_ppm: u32,

    /// Fixed base fee in millisatoshis added to all outgoing Lightning payments.
    #[arg(long, env = "BASE_FEE_MSAT", default_value = "50000")]
    base_fee_msat: u32,

    /// Expiration time in seconds for all generated Lightning invoices.
    #[arg(long, env = "INVOICE_EXPIRY_SECS", default_value = "3600")]
    invoice_expiry_secs: u32,

    /// Network address and port for the HTTP API server to bind to.
    #[arg(long, env = "API_BIND", default_value = "0.0.0.0:8080")]
    api_bind: SocketAddr,

    /// Network address and port for the Lightning node to listen for peer connections.
    #[arg(long, env = "LDK_BIND", default_value = "0.0.0.0:9735")]
    ldk_bind: SocketAddr,

    /// Minimum amount in satoshis enforced across all incoming and outgoing payments.
    #[arg(long, env = "MIN_AMOUNT_SATS", default_value = "1")]
    min_amount_sats: u32,

    /// Maximum amount in satoshis enforced across all incoming and outgoing payments.
    #[arg(long, env = "MAX_AMOUNT_SATS", default_value = "100000")]
    max_amount_sats: u32,

    /// Maximum number of users allowed to register.
    #[arg(long, env = "MAX_USERS", default_value = "1000")]
    max_users: u32,

    /// Maximum number of pending invoices and outgoing payments each user can have simultaneously.
    #[arg(long, env = "MAX_PENDING_PAYMENTS_PER_USER", default_value = "10")]
    max_pending_payments_per_user: u32,
}

#[derive(Clone)]
struct AppState {
    args: Args,
    db: DbConnection,
    node: Arc<Node>,
    event_bus: EventBus,
    send_lock: Arc<tokio::sync::Mutex<()>>,
    endpoint: Endpoint,
    semaphore: Arc<DashMap<String, AtomicUsize>>,
}

impl AppState {
    fn get_fee_msat(&self, amount_msat: i64) -> i64 {
        (amount_msat / self.args.fee_ppm as i64) + self.args.base_fee_msat as i64
    }
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to install CTRL+C handler");

    info!("Signal received, shutting down gracefully...");
}

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    ensure!(
        args.puncture_data_dir.is_dir(),
        "Puncture data dir is not a directory"
    );

    ensure!(
        args.puncture_data_dir.exists(),
        "Puncture data dir does not exist"
    );

    info!("Starting Puncture Daemon...");

    let mut builder = Builder::new();

    builder.set_node_alias("puncture-daemon".to_string())?;

    builder.set_storage_dir_path(args.ldk_data_dir.to_string_lossy().to_string());

    builder.set_network(args.bitcoin_network);

    // Set chain source based on which URL was provided
    match (args.bitcoind_rpc_url.clone(), args.esplora_rpc_url.clone()) {
        (Some(bitcoind_url), None) => {
            builder.set_chain_source_bitcoind_rpc(
                bitcoind_url
                    .host_str()
                    .context("Invalid bitcoind RPC URL: missing host")?
                    .to_string(),
                bitcoind_url
                    .port()
                    .context("Invalid bitcoind RPC URL: missing port")?,
                bitcoind_url.username().to_string(),
                bitcoind_url
                    .password()
                    .context("Invalid bitcoind RPC URL: missing password")?
                    .to_string(),
            );
        }
        (None, Some(esplora_url)) => {
            builder.set_chain_source_esplora(esplora_url.to_string(), None);
        }
        _ => panic!("XOR relation is enforced by argument group"),
    }

    builder
        .set_listening_addresses(vec![args.ldk_bind.into()])
        .context("Failed to set listening address")?;

    let node = Arc::new(builder.build().context("Failed to build LDK Node")?);

    let runtime = Arc::new(tokio::runtime::Runtime::new()?);

    node.start_with_runtime(runtime.clone())
        .context("Failed to start LDK Node")?;

    let db = setup_database(&args.puncture_data_dir, db::MIGRATIONS)?;

    let event_bus = EventBus::new(1000);

    let secret_key = secret::read_or_generate(&args.puncture_data_dir);

    let builder = Endpoint::builder()
        .secret_key(secret_key)
        .discovery_n0()
        .alpns(vec![b"puncture-api".to_vec()]);

    // Use same bind address as HTTP API since iroh uses UDP
    let builder = match args.api_bind {
        SocketAddr::V4(addr_v4) => builder.bind_addr_v4(addr_v4),
        SocketAddr::V6(addr_v6) => builder.bind_addr_v6(addr_v6),
    };

    let endpoint = runtime.block_on(async {
        builder
            .bind()
            .await
            .context("Failed to create iroh endpoint")
    })?;

    info!("Invite: {}", invite::encode(&endpoint.node_id()));

    runtime.spawn(process_events(node.clone(), db.clone(), event_bus.clone()));

    let app_state = AppState {
        args: args.clone(),
        db: db.clone(),
        node: node.clone(),
        event_bus,
        send_lock: Arc::new(tokio::sync::Mutex::new(())),
        endpoint: endpoint.clone(),
        semaphore: Arc::new(DashMap::new()),
    };

    runtime.spawn(api::run_iroh_api(endpoint, app_state.clone()));

    info!("Starting API server at {}", args.api_bind);

    runtime.block_on(async {
        let listener = TcpListener::bind(args.api_bind)
            .await
            .context("Failed to bind to API address")?;

        axum::serve(listener, cli::router(app_state))
            .with_graceful_shutdown(shutdown_signal())
            .await
            .context("Failed to start HTTP server")
    })?;

    node.stop().context("Failed to stop LDK Node")?;

    info!("Graceful shutdown complete");

    Ok(())
}

async fn process_events(node: Arc<Node>, db: DbConnection, event_bus: EventBus) {
    loop {
        match node.next_event_async().await {
            Event::PaymentReceived {
                payment_hash,
                amount_msat,
                ..
            } => {
                let receive_record: Bolt11Receive = db::bolt11_invoice(&db, payment_hash.0)
                    .await
                    .expect("Invoice not found")
                    .into();

                info!(?payment_hash, ?amount_msat, ?receive_record.user_pk, "payment received");

                assert_eq!(receive_record.amount_msat as u64, amount_msat);

                db::create_bolt11_receive_payment(&db, receive_record.clone()).await;

                let balance_msat = db::user_balance(&db, receive_record.user_pk.clone()).await;

                event_bus.send_balance_event(receive_record.user_pk.clone(), balance_msat);

                event_bus.send_payment_event(
                    receive_record.user_pk.clone(),
                    receive_record.clone().into(),
                );
            }
            Event::PaymentSuccessful { payment_hash, .. } => {
                let send_record = db::update_bolt11_send_payment_status(
                    &db,
                    payment_hash.0,
                    "successful".to_string(),
                )
                .await;

                let latency_ms = unix_time().saturating_sub(send_record.created_at);

                info!(?payment_hash, ?send_record.user_pk, ?latency_ms, "payment successful");

                event_bus.send_update_event(
                    send_record.user_pk.clone(),
                    send_record.payment_hash.clone(),
                    "successful".to_string(),
                );
            }
            Event::PaymentFailed { payment_hash, .. } => {
                let send_record = db::update_bolt11_send_payment_status(
                    &db,
                    payment_hash.unwrap().0,
                    "failed".to_string(),
                )
                .await;

                let latency_ms = unix_time().saturating_sub(send_record.created_at);

                warn!(?payment_hash, ?send_record.user_pk, ?latency_ms, "payment failed");

                let balance_msat = db::user_balance(&db, send_record.user_pk.clone()).await;

                event_bus.send_balance_event(send_record.user_pk.clone(), balance_msat);

                event_bus.send_update_event(
                    send_record.user_pk.clone(),
                    send_record.payment_hash.clone(),
                    "failed".to_string(),
                );
            }
            _ => {}
        }

        node.event_handled().expect("Failed to handle event");
    }
}
