mod cli;
mod client;
mod convert;
mod db;
mod events;
mod ui;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::{Context, Result, ensure};
use bitcoin::secp256k1::PublicKey;
use clap::{ArgGroup, Parser};
use iroh::Endpoint;
use ldk_node::bitcoin::Network;
use ldk_node::payment::PaymentKind;
use ldk_node::{Builder, Event, Node};
use lightning::ln::msgs::SocketAddress;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;
use url::Url;

use puncture_core::db::Database;
use puncture_core::{secret, unix_time};

use crate::{
    convert::{IntoPayment, IntoReceiveRecord},
    events::EventBus,
};

#[derive(Parser, Debug, Clone)]
#[command(group(
    ArgGroup::new("chain_source")
        .required(true)
        .multiple(false)
        .args(["bitcoind_rpc_url", "esplora_rpc_url"])
), group(
    ArgGroup::new("lsp1_config")
        .required(false)
        .multiple(false)
        .requires_all(["lsp1_node_id", "lsp1_socket_address"])
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

    /// Liquidity source LSPs1 node ID
    #[arg(long, env = "LSP1_NODE_ID", hide = true)]
    lsp1_node_id: Option<PublicKey>,

    /// Liquidity source LSPs1 node address (IP:PORT, HOSTNAME:PORT or Onion address)
    #[arg(long, env = "LSP1_SOCKET_ADDRESS", hide = true)]
    lsp1_socket_address: Option<String>,

    /// Liquidity source LSPs1 node token
    #[arg(long, env = "LSP1_TOKEN", hide = true)]
    lsp1_token: Option<String>,

    /// Fee rate in parts per million (PPM) applied to outgoing Lightning payments.
    #[arg(long, env = "FEE_PPM", default_value = "5000")]
    fee_ppm: u64,

    /// Fixed base fee in millisatoshis added to all outgoing Lightning payments.
    #[arg(long, env = "BASE_FEE_MSAT", default_value = "10000")]
    base_fee_msat: u64,

    /// Expiration time in seconds for all generated Lightning invoices.
    #[arg(long, env = "INVOICE_EXPIRY_SECS", default_value = "3600")]
    invoice_expiry_secs: u32,

    /// Network address and port for the client interface to be served on.
    #[arg(long, env = "CLIENT_BIND", default_value = "0.0.0.0:8080")]
    client_bind: SocketAddr,

    /// Network address and port for the lightning p2p interface to be served on.
    #[arg(long, env = "LDK_BIND", default_value = "0.0.0.0:8081")]
    ldk_bind: SocketAddr,

    /// Network address and port for the CLI interface to be served on. Never expose this to the public.
    #[arg(long, env = "CLI_BIND", default_value = "0.0.0.0:8082")]
    cli_bind: SocketAddr,

    /// Network address and port for the UI interface to be served on. Never expose this to the public.
    #[arg(long, env = "UI_BIND", default_value = "0.0.0.0:8083")]
    ui_bind: SocketAddr,

    /// Minimum amount in satoshis enforced across all incoming and outgoing payments.
    #[arg(long, env = "MIN_AMOUNT_SATS", default_value = "1")]
    min_amount_sats: u32,

    /// Maximum amount in satoshis enforced across all incoming and outgoing payments.
    #[arg(long, env = "MAX_AMOUNT_SATS", default_value = "100000")]
    max_amount_sats: u32,

    /// Maximum number of pending invoices and outgoing payments each user can have simultaneously.
    #[arg(long, env = "MAX_PENDING_PAYMENTS_PER_USER", default_value = "10")]
    max_pending_payments_per_user: u32,

    /// The log level for the puncture daemon
    #[arg(long, env = "LOG_LEVEL", default_value = "info")]
    log_level: String,
}

#[derive(Clone)]
struct AppState {
    args: Args,
    db: Database,
    node: Arc<Node>,
    event_bus: EventBus,
    send_lock: Arc<tokio::sync::Mutex<()>>,
    node_id: iroh::NodeId,
}

async fn shutdown_signal() {
    tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .expect("Failed to install SIGTERM handler")
        .recv()
        .await;
}

fn main() -> Result<()> {
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
                bitcoind_url.password().unwrap_or_default().to_string(),
            );
        }
        (None, Some(esplora_url)) => {
            builder.set_chain_source_esplora(esplora_url.to_string(), None);
        }
        _ => panic!("XOR relation is enforced by argument group"),
    }

    if let (Some(node_id), Some(socket_address)) = (args.lsp1_node_id, &args.lsp1_socket_address) {
        builder.set_liquidity_source_lsps1(
            node_id,
            SocketAddress::from_str(socket_address)
                .ok()
                .context("Invalid LSP1 socket address")?,
            args.lsp1_token.clone(),
        );
    }

    builder.set_listening_addresses(vec![args.ldk_bind.into()])?;

    let node = Arc::new(builder.build()?);

    let runtime = Arc::new(tokio::runtime::Runtime::new()?);

    node.start_with_runtime(runtime.clone())?;

    // On first startup, connect to public nodes that support Bolt12
    if !secret::exists(&args.puncture_data_dir) && args.bitcoin_network == Network::Bitcoin {
        if let Err(e) = node.connect(
            "03864ef025fde8fb587d989186ce6a4a186895ee44a926bfc370e2c366597a3f8f"
                .parse()
                .unwrap(),
            "3.33.236.230:9735".parse().unwrap(),
            true,
        ) {
            warn!(?e, "Failed to connect to Acinq's public node");
        }

        if let Err(e) = node.connect(
            "027100442c3b79f606f80f322d98d499eefcb060599efc5d4ecb00209c2cb54190"
                .parse()
                .unwrap(),
            "3.230.33.224:9735".parse().unwrap(),
            true,
        ) {
            warn!(?e, "Failed to connect to Block's public node");
        }

        if let Err(e) = node.connect(
            "03c8e5f583585cac1de2b7503a6ccd3c12ba477cfd139cd4905be504c2f48e86bd"
                .parse()
                .unwrap(),
            "34.73.189.183:9735".parse().unwrap(),
            true,
        ) {
            warn!(?e, "Failed to connect to Strike's public node");
        }
    }

    let db = Database::new(&args.puncture_data_dir, puncture_daemon_db::MIGRATIONS, 100)?;

    let event_bus = EventBus::new(1000);

    let secret_key = secret::read_or_generate(&args.puncture_data_dir);

    let builder = Endpoint::builder()
        .secret_key(secret_key)
        .discovery_n0()
        .discovery_dht()
        .alpns(vec![b"puncture-api".to_vec()]);

    let builder = match args.client_bind {
        SocketAddr::V4(addr_v4) => builder.bind_addr_v4(addr_v4),
        SocketAddr::V6(addr_v6) => builder.bind_addr_v6(addr_v6),
    };

    let endpoint = runtime.block_on(builder.bind())?;

    let app_state = AppState {
        args: args.clone(),
        db: db.clone(),
        node: node.clone(),
        event_bus: event_bus.clone(),
        send_lock: Arc::new(tokio::sync::Mutex::new(())),
        node_id: endpoint.node_id(),
    };

    let ct = tokio_util::sync::CancellationToken::new();

    let client_task = runtime.spawn(client::run_api(
        endpoint.clone(),
        app_state.clone(),
        ct.clone(),
    ));

    let cli_task = runtime.spawn(cli::run_cli(app_state.clone(), ct.clone()));

    let ui_task = runtime.spawn(ui::run_ui(app_state.clone(), ct.clone()));

    let events_task = runtime.spawn(process_ldk_events(
        node.clone(),
        db.clone(),
        event_bus.clone(),
        ct.clone(),
    ));

    runtime.block_on(shutdown_signal());

    node.stop()?;

    ct.cancel();

    if let Err(e) = runtime.block_on(client_task) {
        warn!(?e, "Failed to join client API task");
    }

    if let Err(e) = runtime.block_on(cli_task) {
        warn!(?e, "Failed to join CLI API task");
    }

    if let Err(e) = runtime.block_on(events_task) {
        warn!(?e, "Failed to join LDK events task");
    }

    if let Err(e) = runtime.block_on(ui_task) {
        warn!(?e, "Failed to join UI task");
    }

    info!("Graceful shutdown complete");

    Ok(())
}

async fn process_ldk_events(
    node: Arc<Node>,
    db: Database,
    event_bus: EventBus,
    ct: CancellationToken,
) {
    loop {
        tokio::select! {
            event = node.next_event_async() => {
                info!("Processing LDK Event: {:?}", event);

                process_ldk_event(node.clone(), db.clone(), event_bus.clone(), event).await;

                node.event_handled().expect("Failed to handle event");
            },
            _ = ct.cancelled() => {
                break;
            }
        };
    }
}

async fn process_ldk_event(node: Arc<Node>, db: Database, event_bus: EventBus, event: Event) {
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

            event_bus.send_payment_event(record.user_pk.clone(), record.into_payment(true));
        }
        Event::PaymentSuccessful { payment_id, .. } => {
            let record = db::update_send_status(&db, payment_id.unwrap().0, "successful")
                .await
                .expect("successful payment not found");

            let latency_ms = unix_time().saturating_sub(record.created_at);

            info!(?record.user_pk, ?latency_ms, "payment successful");

            event_bus.send_update_event(record.user_pk, record.id, "successful");
        }
        Event::PaymentFailed {
            payment_id, reason, ..
        } => {
            match db::update_send_status(&db, payment_id.unwrap().0, "failed").await {
                Some(record) => {
                    let latency_ms = unix_time().saturating_sub(record.created_at);

                    warn!(?record.user_pk, ?latency_ms, "payment failed");

                    let balance_msat = db::user_balance(&db, record.user_pk.clone()).await;

                    event_bus.send_balance_event(record.user_pk.clone(), balance_msat);

                    event_bus.send_update_event(record.user_pk, record.id, "failed");
                }
                None => {
                    warn!(?payment_id, ?reason, "failed payment not found");
                }
            };
        }
        _ => {}
    }
}
