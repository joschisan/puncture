mod cli;
mod client;
mod convert;
mod db;
mod events;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

use anyhow::{Context, Result, ensure};
use clap::{ArgGroup, Parser};
use dashmap::DashMap;
use iroh::Endpoint;
use ldk_node::bitcoin::Network;
use ldk_node::payment::PaymentKind;
use ldk_node::{Builder, Event, Node};
use tokio::net::TcpListener;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;
use url::Url;

use puncture_cli_core::CLI_BIND_ADDR;
use puncture_core::db::Database;
use puncture_core::{secret, unix_time};

use crate::{convert::ToPayment, events::EventBus};

#[derive(Parser, Debug, Clone)]
#[command(group(
    ArgGroup::new("chain_source")
        .required(true)
        .multiple(false)
        .args(["bitcoind_rpc_url", "esplora_rpc_url"])
))]
struct Args {
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
    #[arg(long, env = "DAEMON_NAME")]
    daemon_name: String,

    /// The log level for the puncture daemon
    #[arg(long, env = "LOG_LEVEL", default_value = "info")]
    log_level: String,

    /// Fee rate in parts per million (PPM) applied to outgoing Lightning payments.
    #[arg(long, env = "FEE_PPM", default_value = "10000")]
    fee_ppm: u64,

    /// Fixed base fee in millisatoshis added to all outgoing Lightning payments.
    #[arg(long, env = "BASE_FEE_MSAT", default_value = "50000")]
    base_fee_msat: u64,

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

    /// Maximum number of pending invoices and outgoing payments each user can have simultaneously.
    #[arg(long, env = "MAX_PENDING_PAYMENTS_PER_USER", default_value = "10")]
    max_pending_payments_per_user: u32,
}

#[derive(Clone)]
struct AppState {
    args: Args,
    db: Database,
    node: Arc<Node>,
    event_bus: EventBus,
    send_lock: Arc<tokio::sync::Mutex<()>>,
    endpoint: Endpoint,
    semaphore: Arc<DashMap<String, AtomicUsize>>,
}

impl AppState {
    fn get_fee_msat(&self, amount_msat: u64) -> u64 {
        (amount_msat * self.args.fee_ppm) / 1_000_000 + self.args.base_fee_msat
    }
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to install CTRL+C handler");

    info!("Signal received, shutting down gracefully...");
}

fn main() -> Result<()> {
    std::panic::set_hook(Box::new(|info| {
        error!("FATAL PANIC: {}", info);

        std::thread::sleep(std::time::Duration::from_millis(500));

        std::process::exit(1);
    }));

    let args = Args::parse();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(args.log_level.clone()))
        .init();

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

    builder.set_log_facade_logger();

    builder.set_node_alias("puncture-daemon".to_string())?;

    builder.set_storage_dir_path(args.ldk_data_dir.to_string_lossy().to_string());

    builder.set_network(args.bitcoin_network);

    if args.bitcoin_network == Network::Bitcoin {
        builder.set_gossip_source_rgs("https://rapidsync.lightningdevkit.org/snapshot".to_string());
    }

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

    let db = Database::new(&args.puncture_data_dir, puncture_daemon_db::MIGRATIONS, 100)?;

    let event_bus = EventBus::new(1000);

    let secret_key = secret::read_or_generate(&args.puncture_data_dir);

    let builder = Endpoint::builder()
        .secret_key(secret_key)
        .discovery_n0()
        .alpns(vec![b"puncture-api".to_vec()]);

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

    runtime.spawn(client::run_api(endpoint, app_state.clone()));

    runtime.block_on(async {
        let listener = TcpListener::bind(CLI_BIND_ADDR)
            .await
            .context("Failed to bind to API address")?;

        axum::serve(listener, cli::router().with_state(app_state))
            .with_graceful_shutdown(shutdown_signal())
            .await
            .context("Failed to start HTTP server")
    })?;

    node.stop().context("Failed to stop LDK Node")?;

    info!("Graceful shutdown complete");

    Ok(())
}

async fn process_events(node: Arc<Node>, db: Database, event_bus: EventBus) {
    loop {
        let event = node.next_event_async().await;

        info!("Processing LDK Event: {:?}", event);

        match event {
            Event::PaymentReceived {
                payment_id,
                amount_msat,
                ..
            } => {
                let record = match node
                    .payment(&payment_id.unwrap())
                    .expect("Payment not found")
                    .kind
                {
                    PaymentKind::Bolt11 { hash, .. } => db::get_invoice(&db, hash.0)
                        .await
                        .expect("Invoice not found")
                        .into_receive_record(payment_id.unwrap().0, amount_msat),
                    PaymentKind::Bolt12Offer { offer_id, .. } => db::get_offer(&db, offer_id.0)
                        .await
                        .expect("Offer not found")
                        .into_receive_record(payment_id.unwrap().0, amount_msat),
                    _ => panic!("Unexpected payment kind"),
                };

                info!(?amount_msat, ?record.user_pk, "payment received");

                assert_eq!(record.amount_msat as u64, amount_msat);

                db::create_receive_payment(&db, record.clone()).await;

                let balance_msat = db::user_balance(&db, record.user_pk.clone()).await;

                event_bus.send_balance_event(record.user_pk.clone(), balance_msat);

                event_bus.send_payment_event(record.user_pk.clone(), record.to_payment(true));
            }
            Event::PaymentSuccessful { payment_id, .. } => {
                let record = db::update_send_status(&db, payment_id.unwrap().0, "successful").await;

                let latency_ms = unix_time().saturating_sub(record.created_at);

                info!(?record.user_pk, ?latency_ms, "payment successful");

                event_bus.send_update_event(record.user_pk, record.id, "successful");
            }
            Event::PaymentFailed { payment_id, .. } => {
                let record = db::update_send_status(&db, payment_id.unwrap().0, "failed").await;

                let latency_ms = unix_time().saturating_sub(record.created_at);

                warn!(?record.user_pk, ?latency_ms, "payment failed");

                let balance_msat = db::user_balance(&db, record.user_pk.clone()).await;

                event_bus.send_balance_event(record.user_pk.clone(), balance_msat);

                event_bus.send_update_event(record.user_pk, record.id, "failed");
            }
            _ => {}
        }

        node.event_handled().expect("Failed to handle event");
    }
}
