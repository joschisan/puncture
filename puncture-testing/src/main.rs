mod cli;

use std::net::{Ipv4Addr, SocketAddrV4};
use std::process::{Child, Command};
use std::str::FromStr;
use std::sync::Arc;
use std::thread::sleep;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, ensure};
use bitcoin::Network;
use bitcoincore_rpc::bitcoin::{Address, address::NetworkUnchecked};
use bitcoincore_rpc::{Auth, Client, RpcApi};
use lightning::offers::offer::Offer;
use lightning_invoice::{Bolt11InvoiceDescription, Description};
use lightning_types::payment::PaymentHash;

use puncture_client::PunctureClient;
use puncture_client_core::{AppEvent, Balance, Payment, Update};
use puncture_core::{InviteCode, PunctureCode};

fn main() -> Result<()> {
    let rpc = Client::new(
        "http://127.0.0.1:18443",
        Auth::UserPass("bitcoin".to_string(), "bitcoin".to_string()),
    )
    .context("Failed to connect to Bitcoin RPC")?;

    let ldk_node_port = 9735;

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

    start_daemon()?;

    retry(cli::balances, "wait for daemon to start its API")?;

    let address = cli::onchain_receive()?;

    rpc.generate_to_address(101, &address)?;

    retry(
        || {
            let balances = cli::balances()?;

            ensure!(
                balances.total_onchain_balance_sats > 10_000_000,
                "Balance below 10M"
            );

            Ok(())
        },
        "await onchain balance",
    )?;

    retry(
        || cli::open_channel(node.node_id(), ldk_node_port),
        "wait for channel to be opened",
    )?;

    let funding_txo = retry(
        || {
            cli::list_channels()?
                .pop()
                .and_then(|c| c.funding_txo)
                .context("No funding txo yet")
        },
        "wait for funding tx to be negotiated",
    )?;

    retry(
        || rpc.get_mempool_entry(&funding_txo.txid),
        "wait for funding tx to enter the mempool",
    )?;

    rpc.generate_to_address(6, &dummy_address())?;

    retry(
        || {
            let balances = cli::balances()?;

            ensure!(
                balances.total_outbound_capacity_msat > 1_000_000_000,
                "Outbound capacity not reached",
            );

            ensure!(
                balances.total_inbound_capacity_msat > 1_000_000_000,
                "Channel capacity not reached"
            );

            Ok(())
        },
        "await channel capacity",
    )?;

    let response = cli::invite()?;

    println!("Daemon invite: {}", response.invite);

    let invite = PunctureCode::decode(&response.invite)
        .unwrap()
        .to_invite()
        .unwrap();

    runtime.block_on(run_test(node, invite))
}

async fn run_test(node: Arc<ldk_node::Node>, invite: InviteCode) -> Result<()> {
    let client_a = PunctureClient::new("./data-dir-testing/client-a".to_string()).await;
    let client_b = PunctureClient::new("./data-dir-testing/client-b".to_string()).await;

    let connection_a = client_a.register(invite.clone()).await.unwrap();
    let connection_b = client_b.register(invite.clone()).await.unwrap();

    assert_eq!(
        connection_a.next_event().await,
        AppEvent::Balance(Balance { amount_msat: 0 })
    );

    assert_eq!(
        connection_b.next_event().await,
        AppEvent::Balance(Balance { amount_msat: 0 })
    );

    let invoice = connection_a
        .bolt11_receive(1_000_000, String::new())
        .await
        .unwrap();

    node.bolt11_payment().send(&invoice, None).unwrap();

    assert_eq!(
        connection_a.next_event().await,
        AppEvent::Balance(Balance {
            amount_msat: 1_000_000
        })
    );

    assert_payment(connection_a.next_event().await, 1_000_000, 0, "successful").await;

    let invoice = connection_b
        .bolt11_receive(500_000, String::new())
        .await
        .unwrap();

    connection_a
        .bolt11_send(invoice.clone(), 500_000, None)
        .await
        .unwrap();

    assert_eq!(
        connection_a.next_event().await,
        AppEvent::Balance(Balance {
            amount_msat: 499_000
        })
    );

    assert_payment(connection_a.next_event().await, 500_000, 1000, "successful").await;

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

    while connection_b
        .bolt11_send(invoice.clone(), 100_000, None)
        .await
        .is_err()
    {
        println!("Waiting for payment to be sent");

        sleep(Duration::from_secs(1));
    }

    assert_eq!(
        connection_b.next_event().await,
        AppEvent::Balance(Balance {
            amount_msat: 389_500
        })
    );

    let payment = assert_payment(connection_b.next_event().await, 100_000, 10_500, "pending").await;

    assert_eq!(
        connection_b.next_event().await,
        AppEvent::Balance(Balance {
            amount_msat: 400_000
        })
    );

    assert_eq!(
        connection_b.next_event().await,
        AppEvent::Update(Update {
            id: payment.id,
            status: "successful".to_string(),
            fee_msat: 0
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
        .bolt11_send(invoice, 100_000, None)
        .await
        .unwrap();

    assert_eq!(
        connection_b.next_event().await,
        AppEvent::Balance(Balance {
            amount_msat: 289_500
        })
    );

    let payment = assert_payment(connection_b.next_event().await, 100_000, 10_500, "pending").await;

    sleep(Duration::from_secs(1));

    node.bolt11_payment().fail_for_hash(PaymentHash([0; 32]))?;

    assert_eq!(
        connection_b.next_event().await,
        AppEvent::Balance(Balance {
            amount_msat: 400_000
        })
    );

    assert_eq!(
        connection_b.next_event().await,
        AppEvent::Update(Update {
            id: payment.id,
            status: "failed".to_string(),
            fee_msat: 0
        })
    );

    let connection_a = client_a.list_daemons().await.pop().unwrap().connect();
    let connection_b = client_b.list_daemons().await.pop().unwrap().connect();

    assert_eq!(
        connection_a.next_event().await,
        AppEvent::Balance(Balance {
            amount_msat: 499_000
        })
    );

    assert_payment(connection_a.next_event().await, 1_000_000, 0, "successful").await;

    assert_payment(connection_a.next_event().await, 500_000, 1000, "successful").await;

    assert_eq!(
        connection_b.next_event().await,
        AppEvent::Balance(Balance {
            amount_msat: 400_000
        })
    );

    assert_payment(connection_b.next_event().await, 500_000, 0, "successful").await;

    assert_payment(connection_b.next_event().await, 100_000, 0, "successful").await;

    assert_payment(connection_b.next_event().await, 100_000, 0, "failed").await;

    let invoice = connection_a
        .bolt11_receive(100_000, String::new())
        .await
        .unwrap();

    connection_b
        .bolt11_send(invoice, 100_000, None)
        .await
        .unwrap();

    assert_eq!(
        connection_a.next_event().await,
        AppEvent::Balance(Balance {
            amount_msat: 599_000
        })
    );

    assert_payment(connection_a.next_event().await, 100_000, 0, "successful").await;

    assert_eq!(
        connection_b.next_event().await,
        AppEvent::Balance(Balance {
            amount_msat: 299_000
        })
    );

    assert_payment(connection_b.next_event().await, 100_000, 1000, "successful").await;

    println!("Testing Bolt11 was successful!");

    let offer = connection_a.bolt12_receive_variable_amount().await.unwrap();

    node.bolt12_payment()
        .send_using_amount(&Offer::from_str(&offer).unwrap(), 100_000, None, None)
        .unwrap();

    node.bolt12_payment()
        .send_using_amount(&Offer::from_str(&offer).unwrap(), 100_000, None, None)
        .unwrap();

    assert_eq!(
        connection_a.next_event().await,
        AppEvent::Balance(Balance {
            amount_msat: 699_000
        })
    );

    assert_payment(connection_a.next_event().await, 100_000, 0, "successful").await;

    assert_eq!(
        connection_a.next_event().await,
        AppEvent::Balance(Balance {
            amount_msat: 799_000
        })
    );

    assert_payment(connection_a.next_event().await, 100_000, 0, "successful").await;

    let offer = connection_b.bolt12_receive_variable_amount().await.unwrap();

    connection_a
        .bolt12_send(Offer::from_str(&offer).unwrap(), 100_000)
        .await
        .unwrap();

    assert_eq!(
        connection_a.next_event().await,
        AppEvent::Balance(Balance {
            amount_msat: 698_000
        })
    );

    assert_payment(connection_a.next_event().await, 100_000, 1000, "successful").await;

    assert_eq!(
        connection_b.next_event().await,
        AppEvent::Balance(Balance {
            amount_msat: 399_000
        })
    );

    assert_payment(connection_b.next_event().await, 100_000, 0, "successful").await;

    let offer = node
        .bolt12_payment()
        .receive_variable_amount("", None)
        .unwrap();

    connection_b.bolt12_send(offer, 100_000).await.unwrap();

    assert_eq!(
        connection_b.next_event().await,
        AppEvent::Balance(Balance {
            amount_msat: 288_500
        })
    );

    let payment = assert_payment(connection_b.next_event().await, 100_000, 10_500, "pending").await;

    assert_eq!(
        connection_b.next_event().await,
        AppEvent::Balance(Balance {
            amount_msat: 299_000
        })
    );

    assert_eq!(
        connection_b.next_event().await,
        AppEvent::Update(Update {
            id: payment.id,
            status: "successful".to_string(),
            fee_msat: 0
        })
    );

    let offer = node
        .bolt12_payment()
        .receive(50_000, "", Some(3600), None)
        .unwrap();

    connection_b.bolt12_send(offer, 50_000).await.unwrap();

    assert_eq!(
        connection_b.next_event().await,
        AppEvent::Balance(Balance {
            amount_msat: 238_750
        })
    );

    let payment = assert_payment(connection_b.next_event().await, 50_000, 10_250, "pending").await;

    assert_eq!(
        connection_b.next_event().await,
        AppEvent::Balance(Balance {
            amount_msat: 249_000
        })
    );

    assert_eq!(
        connection_b.next_event().await,
        AppEvent::Update(Update {
            id: payment.id,
            status: "successful".to_string(),
            fee_msat: 0
        })
    );

    println!("Testing Bolt12 was successful!");

    let daemon_a = client_a.list_daemons().await.pop().unwrap();
    let daemon_b = client_b.list_daemons().await.pop().unwrap();

    client_a.delete_daemon(daemon_a).await;
    client_b.delete_daemon(daemon_b).await;

    assert!(client_a.list_daemons().await.is_empty());
    assert!(client_b.list_daemons().await.is_empty());

    client_a.register(invite.clone()).await.unwrap();
    client_b.register(invite.clone()).await.unwrap();

    assert_eq!(client_a.list_daemons().await.len(), 1);
    assert_eq!(client_b.list_daemons().await.len(), 1);

    println!("Testing daemon deletion and re-registration was successful!");

    let client_c = PunctureClient::new("./data-dir-testing/client-c".to_string()).await;

    client_c.register(invite.clone()).await.unwrap();

    let response = cli::recover(client_a.user_pk().await).unwrap();

    let recovery = PunctureCode::decode(&response.recovery)
        .unwrap()
        .to_recovery()
        .unwrap();

    let connection_c = client_c.register(invite).await.unwrap();

    assert_eq!(
        connection_c.next_event().await,
        AppEvent::Balance(Balance { amount_msat: 0 })
    );

    assert_eq!(connection_c.recover(recovery).await.unwrap(), 698_000);

    assert_eq!(
        connection_c.next_event().await,
        AppEvent::Balance(Balance {
            amount_msat: 698_000
        })
    );

    connection_c
        .set_recovery_name(Some("joschisan".to_string()))
        .await
        .unwrap();

    assert_eq!(cli::list_users().unwrap().len(), 3);

    println!("Testing user recovery was successful!");

    Ok(())
}

pub fn start_daemon() -> Result<Child> {
    Command::new("target/debug/puncture-daemon")
        .arg("--puncture-data-dir")
        .arg("./data-dir-testing/daemon/puncture")
        .arg("--ldk-data-dir")
        .arg("./data-dir-testing/daemon/ldk")
        .arg("--bitcoin-network")
        .arg("regtest")
        .arg("--bitcoind-rpc-url")
        .arg("http://bitcoin:bitcoin@127.0.0.1:18443")
        .arg("--daemon-name")
        .arg("testing daemon")
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
        .require_network(Network::Regtest)
        .expect("Dummy address should be valid for regtest network")
}

fn retry<T, E, F>(action: F, description: &str) -> Result<T>
where
    F: Fn() -> Result<T, E>,
{
    for _ in 0..30 {
        match action() {
            Ok(result) => return Ok(result),
            Err(_) => {
                sleep(Duration::from_secs(1));
            }
        }
    }

    Err(anyhow!("Failed to {} after 30 attempts", description))
}
