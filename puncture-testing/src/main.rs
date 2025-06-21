mod cli;

use std::net::{Ipv4Addr, SocketAddrV4};
use std::process::{Child, Command};
use std::sync::Arc;
use std::thread::sleep;
use std::time::Duration;

use anyhow::{Context, Result};
use bitcoin::Network;
use bitcoincore_rpc::bitcoin::{Address, address::NetworkUnchecked};
use bitcoincore_rpc::{Auth, Client, RpcApi};
use lightning_invoice::{Bolt11InvoiceDescription, Description};
use lightning_types::payment::PaymentHash;

use puncture_api_core::{AppEvent, Balance, Payment, Update};
use puncture_client::PunctureClient;

fn main() -> Result<()> {
    let rpc = Client::new(
        "http://127.0.0.1:18443",
        Auth::UserPass("bitcoin".to_string(), "bitcoin".to_string()),
    )
    .context("Failed to connect to Bitcoin RPC")?;

    let (api_port, ldk_port, ldk_node_port) = (8080, 8081, 8082);

    // Build freestanding ldk node for testing

    let mut builder = ldk_node::Builder::new();

    builder.set_node_alias("puncture-testing-node".to_string())?;

    builder.set_storage_dir_path("./data-dir-testing/ldk-node".to_string());

    builder.set_network(Network::Regtest);

    builder.set_chain_source_bitcoind_rpc(
        "127.0.0.1".to_string(),
        18443,
        "bitcoin".to_string(),
        "bitcoin".to_string(),
    );

    builder
        .set_listening_addresses(vec![
            SocketAddrV4::new(Ipv4Addr::LOCALHOST, ldk_node_port).into(),
        ])
        .context("Failed to set listening address")?;

    let node = Arc::new(builder.build().context("Failed to build LDK Node")?);

    let runtime = Arc::new(tokio::runtime::Runtime::new()?);

    node.start_with_runtime(runtime.clone())
        .context("Failed to start LDK Node")?;

    start_daemon(api_port, ldk_port)?;

    sleep(Duration::from_secs(1)); // Wait for daemon to start its API

    fund_daemon(&rpc, api_port)?;

    sleep(Duration::from_secs(1)); // Wait for balance to be updated

    cli::open_channel(api_port, node.node_id(), ldk_node_port)?;

    sleep(Duration::from_secs(1)); // Wait for funding tx to enter the mempool

    rpc.generate_to_address(6, &dummy_address())?;

    await_channel_capacity(api_port)?;

    let response = cli::invite(api_port)?;

    println!("Daemon invite: {}", response.invite);

    runtime.block_on(run_test(node, response.invite))
}

async fn run_test(node: Arc<ldk_node::Node>, invite: String) -> Result<()> {
    let client_a = PunctureClient::new("./data-dir-testing/client-a".to_string()).await;
    let client_b = PunctureClient::new("./data-dir-testing/client-b".to_string()).await;

    let connection_a = client_a.add_instance(invite.clone()).await.unwrap();
    let connection_b = client_b.add_instance(invite.clone()).await.unwrap();

    assert_eq!(
        connection_a.next_event().await,
        AppEvent::Balance(Balance { amount_msat: 0 })
    );

    assert_eq!(
        connection_b.next_event().await,
        AppEvent::Balance(Balance { amount_msat: 0 })
    );

    let invoice = connection_a.bolt11_receive(1_000_000, None).await.unwrap();

    node.bolt11_payment()
        .send(&invoice.parse().unwrap(), None)
        .unwrap();

    assert_eq!(
        connection_a.next_event().await,
        AppEvent::Balance(Balance {
            amount_msat: 1_000_000
        })
    );

    assert_payment(connection_a.next_event().await, 1_000_000, 0, "successful").await;

    let invoice = connection_b.bolt11_receive(500_000, None).await.unwrap();

    connection_a.bolt11_send(invoice, None).await.unwrap();

    assert_eq!(
        connection_a.next_event().await,
        AppEvent::Balance(Balance {
            amount_msat: 449_950
        })
    );

    assert_payment(
        connection_a.next_event().await,
        500_000,
        50_050,
        "successful",
    )
    .await;

    assert_eq!(
        connection_b.next_event().await,
        AppEvent::Balance(Balance {
            amount_msat: 500_000
        })
    );

    assert_payment(connection_b.next_event().await, 500_000, 0, "successful").await;

    let invoice = node
        .bolt11_payment()
        .receive(
            100_000,
            &Bolt11InvoiceDescription::Direct(Description::new(String::new())?),
            3600,
        )
        .unwrap();

    connection_b
        .bolt11_send(invoice.to_string(), None)
        .await
        .unwrap();

    assert_eq!(
        connection_b.next_event().await,
        AppEvent::Balance(Balance {
            amount_msat: 349_990
        })
    );

    let payment = assert_payment(connection_b.next_event().await, 100_000, 50_010, "pending").await;

    assert_eq!(
        connection_b.next_event().await,
        AppEvent::Update(Update {
            id: payment.id,
            status: "successful".to_string()
        })
    );

    let invoice = node
        .bolt11_payment()
        .receive_for_hash(
            100_000,
            &Bolt11InvoiceDescription::Direct(Description::new(String::new())?),
            3600,
            PaymentHash([0; 32]),
        )
        .unwrap();

    connection_b
        .bolt11_send(invoice.to_string(), None)
        .await
        .unwrap();

    assert_eq!(
        connection_b.next_event().await,
        AppEvent::Balance(Balance {
            amount_msat: 199_980
        })
    );

    let payment = assert_payment(connection_b.next_event().await, 100_000, 50_010, "pending").await;

    sleep(Duration::from_secs(1));

    node.bolt11_payment().fail_for_hash(PaymentHash([0; 32]))?;

    assert_eq!(
        connection_b.next_event().await,
        AppEvent::Balance(Balance {
            amount_msat: 349_990
        })
    );

    assert_eq!(
        connection_b.next_event().await,
        AppEvent::Update(Update {
            id: payment.id,
            status: "failed".to_string()
        })
    );

    let connection_a = client_a.get_instances().pop().unwrap().connect();
    let connection_b = client_b.get_instances().pop().unwrap().connect();

    assert_eq!(
        connection_a.next_event().await,
        AppEvent::Balance(Balance {
            amount_msat: 449_950
        })
    );

    assert_payment(connection_a.next_event().await, 1_000_000, 0, "successful").await;

    assert_payment(
        connection_a.next_event().await,
        500_000,
        50_050,
        "successful",
    )
    .await;

    assert_eq!(
        connection_b.next_event().await,
        AppEvent::Balance(Balance {
            amount_msat: 349_990
        })
    );

    assert_payment(connection_b.next_event().await, 500_000, 0, "successful").await;

    assert_payment(
        connection_b.next_event().await,
        100_000,
        50_010,
        "successful",
    )
    .await;

    assert_payment(connection_b.next_event().await, 100_000, 50_010, "failed").await;

    let invoice = connection_a.bolt11_receive(100_000, None).await.unwrap();

    connection_b.bolt11_send(invoice, None).await.unwrap();

    assert_eq!(
        connection_a.next_event().await,
        AppEvent::Balance(Balance {
            amount_msat: 549_950
        })
    );

    assert_payment(connection_a.next_event().await, 100_000, 0, "successful").await;

    assert_eq!(
        connection_b.next_event().await,
        AppEvent::Balance(Balance {
            amount_msat: 199_980
        })
    );

    assert_payment(
        connection_b.next_event().await,
        100_000,
        50_010,
        "successful",
    )
    .await;

    println!("Test successful!");

    Ok(())
}

pub fn start_daemon(api_port: u16, ldk_port: u16) -> Result<Child> {
    Command::new("target/debug/puncture-daemon")
        .arg("--admin-auth")
        .arg("testing-auth")
        .arg("--puncture-data-dir")
        .arg("./data-dir-testing/daemon/puncture")
        .arg("--ldk-data-dir")
        .arg("./data-dir-testing/daemon/ldk")
        .arg("--bitcoin-network")
        .arg("regtest")
        .arg("--bitcoind-rpc-url")
        .arg("http://bitcoin:bitcoin@127.0.0.1:18443")
        .arg("--instance-name")
        .arg("testing instance")
        .arg("--api-bind")
        .arg(format!("127.0.0.1:{}", api_port))
        .arg("--ldk-bind")
        .arg(format!("127.0.0.1:{}", ldk_port))
        .spawn()
        .context("Failed to start daemon")
}

async fn assert_payment(event: AppEvent, amount_msat: i64, fee_msat: i64, status: &str) -> Payment {
    match event {
        AppEvent::Payment(payment) => {
            assert_eq!(payment.amount_msat, amount_msat);
            assert_eq!(payment.fee_msat, fee_msat);
            assert_eq!(payment.status, status);

            payment
        }
        _ => panic!("Expected payment event"),
    }
}

fn dummy_address() -> Address {
    "bcrt1qsurq86f2kdlce0tflgznehpzx275d93wvvxsml"
        .parse::<Address<NetworkUnchecked>>()
        .unwrap()
        .assume_checked()
}

fn fund_daemon(rpc: &Client, api_port: u16) -> Result<()> {
    let address = cli::onchain_receive(api_port)?;

    rpc.generate_to_address(101, &address)?;

    loop {
        let balances = cli::balances(api_port)?;

        if balances.total_onchain_balance_sats > 10_000_000 {
            break;
        }

        sleep(Duration::from_secs(1));
    }

    Ok(())
}

fn await_channel_capacity(api_port: u16) -> Result<()> {
    loop {
        let balances = cli::balances(api_port)?;

        if balances.total_outbound_capacity_msat > 1_000_000_000
            && balances.total_inbound_capacity_msat > 1_000_000_000
        {
            break;
        }

        sleep(Duration::from_secs(1));
    }

    Ok(())
}
