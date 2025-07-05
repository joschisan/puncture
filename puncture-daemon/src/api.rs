use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{future, sync::Arc};

use anyhow::{Context, Result, anyhow, ensure};
use bitcoin::hashes::Hash;
use futures::FutureExt;
use futures::stream;
use iroh::endpoint::Connection;
use iroh::{Endpoint, endpoint::Incoming};
use ldk_node::payment::SendingParameters;
use lightning::offers::offer::Offer;
use lightning_invoice::{Bolt11InvoiceDescription, Description};
use rand::Rng;
use serde::Deserialize;
use serde_json::Value;
use tokio_stream::{Stream, StreamExt};
use tracing::{error, info, warn};

use puncture_api_core::{
    AppEvent, Balance, Bolt11ReceiveRequest, Bolt11ReceiveResponse, Bolt11SendRequest,
    Bolt12ReceiveResponse, Bolt12SendRequest, FeesResponse, Payment, RegisterRequest,
    RegisterResponse,
};

use crate::{AppState, db};

macro_rules! method {
    ($func:ident, $state:expr, $user_id:expr, $params:expr) => {{
        match serde_json::from_value($params) {
            Ok(request) => $func($state, $user_id, request).await.map(|response| {
                serde_json::to_value(response).expect("Failed to serialize response")
            }),
            Err(_) => Err("Failed to deserialize request".to_string()),
        }
    }};
}

#[derive(Deserialize)]
struct JsonRpcRequest {
    method: String,
    request: Value,
}

pub async fn run_iroh_api(endpoint: Endpoint, app_state: AppState) -> anyhow::Result<()> {
    tracing::info!(
        "Starting Iroh API server with node_id: {}",
        endpoint.node_id()
    );

    let app_state = Arc::new(app_state);

    while let Some(incoming) = endpoint.accept().await {
        tokio::spawn(
            handle_connection(app_state.clone(), incoming).then(|result| async {
                if let Err(e) = result {
                    warn!(?e, "Error handling connection");
                }
            }),
        );
    }

    Ok(())
}

async fn handle_connection(app_state: Arc<AppState>, incoming: Incoming) -> anyhow::Result<()> {
    let connection = incoming.accept()?.await?;

    match std::str::from_utf8(&connection.alpn().unwrap())? {
        "puncture-register" => handle_register_connection(app_state.clone(), connection).await,
        "puncture-api" => handle_api_connection(app_state.clone(), connection).await,
        alpn => Err(anyhow!("Unknown ALPN: {:?}", alpn)),
    }
}

async fn handle_register_connection(
    app_state: Arc<AppState>,
    connection: Connection,
) -> anyhow::Result<()> {
    let node_id = connection.remote_node_id()?.to_string();

    let (mut send_stream, mut recv_stream) = connection.accept_bi().await?;

    let request = recv_stream.read_to_end(100_000).await?;

    let request: RegisterRequest = serde_json::from_slice(&request)?;

    let response = register_with_invite(app_state.clone(), node_id.clone(), request.invite_id)
        .await
        .map_err(|e| e.to_string());

    let response = serde_json::to_vec(&response).expect("Failed to serialize response");

    send_stream.write_all(&response).await?;

    send_stream.finish()?;

    connection.closed().await;

    Ok(())
}

async fn register_with_invite(
    app_state: Arc<AppState>,
    node_id: String,
    invite_id: String,
) -> anyhow::Result<RegisterResponse> {
    let invite = db::get_invite(&app_state.db, &invite_id)
        .await
        .context("Invite not found")?;

    ensure!(invite.expires_at > db::unix_time(), "Invite expired");

    ensure!(
        invite.user_limit > db::count_invite_users(&app_state.db, &invite_id).await,
        "Invite user limit reached"
    );

    db::register_user_with_invite(&app_state.db, node_id.clone(), invite_id.clone()).await;

    info!(?node_id, ?invite_id, "New user registered");

    Ok(RegisterResponse {
        network: app_state.args.bitcoin_network,
        name: app_state.args.daemon_name.clone(),
    })
}

async fn handle_api_connection(
    app_state: Arc<AppState>,
    connection: Connection,
) -> anyhow::Result<()> {
    let node_id = connection.remote_node_id()?.to_string();

    ensure!(
        db::user_exists(&app_state.db, node_id.clone()).await,
        "User not found"
    );

    let counter = app_state
        .semaphore
        .entry(node_id.clone())
        .or_insert_with(|| AtomicUsize::new(0));

    ensure!(
        counter.load(Ordering::Relaxed) < 3,
        "User has reached maximum of 3 connections"
    );

    counter.fetch_add(1, Ordering::Relaxed);

    drop(counter);

    if let Err(e) = drive_connection(app_state.clone(), connection.clone(), node_id.clone()).await {
        warn!(?e, "Error while driving connection");
    }

    connection.closed().await;

    app_state
        .semaphore
        .get(&node_id)
        .expect("Counter not found")
        .fetch_sub(1, Ordering::Relaxed);

    Ok(())
}

async fn drive_connection(
    app_state: Arc<AppState>,
    connection: Connection,
    node_id: String,
) -> anyhow::Result<()> {
    let mut event_stream = Box::pin(events(app_state.clone(), node_id.clone()).await);

    loop {
        tokio::select! {
            stream = connection.accept_bi() => {
                match stream {
                    Ok((send, recv)) => {
                        handle_request(app_state.clone(), node_id.clone(), send, recv).await?;
                    }
                    Err(..) => {
                        return Ok(());
                    }
                }
            }
            event = event_stream.next() => {
                let event = event.unwrap().map_err(|e| anyhow!(e))?;

                let event = serde_json::to_vec(&event).expect("Failed to serialize event");

                let mut send = connection.open_uni().await?;

                send.write_all(&event).await?;

                send.finish()?;
            }
        }
    }
}

async fn handle_request(
    state: Arc<AppState>,
    user_id: String,
    mut send_stream: iroh::endpoint::SendStream,
    mut recv_stream: iroh::endpoint::RecvStream,
) -> anyhow::Result<()> {
    let request = recv_stream.read_to_end(100_000).await?;

    let request: JsonRpcRequest = serde_json::from_slice(&request)?;

    let response = match request.method.as_str() {
        "fees" => method!(fees, state, user_id, request.request),
        "bolt11_receive" => {
            method!(bolt11_receive, state, user_id, request.request)
        }
        "bolt11_send" => method!(bolt11_send, state, user_id, request.request),
        "bolt12_send" => method!(bolt12_send, state, user_id, request.request),
        "bolt12_receive_variable_amount" => method!(
            bolt12_receive_variable_amount,
            state,
            user_id,
            request.request
        ),
        _ => Err(format!("Method '{}' not found", request.method)),
    };

    let response = serde_json::to_vec(&response).expect("Failed to serialize response");

    send_stream.write_all(&response).await?;

    send_stream.finish()?;

    Ok(())
}

pub async fn bolt11_receive(
    state: Arc<AppState>,
    user_pk: String,
    request: Bolt11ReceiveRequest,
) -> Result<Bolt11ReceiveResponse, String> {
    info!(?request, "bolt11 receive");

    let pending = db::count_pending_invoices(&state.db, user_pk.clone()).await;

    if pending >= state.args.max_pending_payments_per_user as i64 {
        return Err("Too many pending invoices".to_string());
    }

    check_amount_bounds(state.clone(), request.amount_msat as u64)?;

    let invoice = state
        .node
        .bolt11_payment()
        .receive(
            request.amount_msat.into(),
            &Description::new(request.description.clone())
                .map(Bolt11InvoiceDescription::Direct)
                .map_err(|e| e.to_string())?,
            state.args.invoice_expiry_secs,
        )
        .inspect_err(|error| error!(?error, "ldk node bolt11 receive error"))
        .map_err(|e| e.to_string())?;

    db::create_invoice(
        &state.db,
        user_pk,
        invoice.clone(),
        request.amount_msat.into(),
        request.description,
        state.args.invoice_expiry_secs,
    )
    .await;

    Ok(Bolt11ReceiveResponse { invoice })
}

#[tracing::instrument(skip(state))]
pub async fn bolt11_send(
    state: Arc<AppState>,
    user_pk: String,
    request: Bolt11SendRequest,
) -> Result<(), String> {
    let send_lock = state.send_lock.lock().await;

    let fee_msat = check_send_request(state.clone(), user_pk.clone(), request.amount_msat).await?;

    let payment_hash = request.invoice.payment_hash().to_byte_array();

    let send_status = match db::get_invoice(&state.db, payment_hash).await {
        Some(invoice) => {
            if invoice.user_pk == user_pk {
                return Err("This is your own invoice".to_string());
            }

            if let Some(amount_msat) = invoice.amount_msat {
                if amount_msat as u64 > request.amount_msat {
                    return Err("Amount is lower than the invoice's minimum amount".to_string());
                }
            }

            let record = invoice
                .clone()
                .into_receive_record(payment_hash, request.amount_msat);

            db::create_receive_payment(&state.db, record.clone()).await;

            push_events(state.clone(), invoice.user_pk.clone(), record).await;

            "successful".to_string()
        }
        None => {
            state
                .node
                .bolt11_payment()
                .send_using_amount(
                    &request.invoice,
                    request.amount_msat,
                    Some(sending_parameters(fee_msat)),
                )
                .inspect_err(|error| error!(?error, "ldk node bolt11 send error"))
                .map_err(|e| e.to_string())?;

            "pending".to_string()
        }
    };

    let send_record = db::create_send_payment(
        &state.db,
        payment_hash,
        user_pk.clone(),
        request.amount_msat as i64,
        fee_msat as i64,
        request.invoice.description().to_string(),
        request.invoice.to_string(),
        send_status,
        request.ln_address.clone(),
    )
    .await;

    drop(send_lock);

    push_events(state, user_pk, send_record).await;

    Ok(())
}

#[tracing::instrument(skip(state))]
pub async fn bolt12_send(
    state: Arc<AppState>,
    user_pk: String,
    request: Bolt12SendRequest,
) -> Result<(), String> {
    let send_lock = state.send_lock.lock().await;

    let fee_msat = check_send_request(state.clone(), user_pk.clone(), request.amount_msat).await?;

    let offer = Offer::from_str(&request.offer).map_err(|_| "Invalid offer".to_string())?;

    let (payment_id, status) = match db::get_offer(&state.db, offer.id().0).await {
        Some(offer) => {
            if offer.user_pk == user_pk {
                return Err("This is your own payment request".to_string());
            }

            if let Some(amount_msat) = offer.amount_msat {
                if amount_msat as u64 > request.amount_msat {
                    return Err("Amount is lower than the offer's minimum amount".to_string());
                }
            }

            let payment_id: [u8; 32] = rand::rng().random();

            let record = offer
                .clone()
                .into_receive_record(payment_id, request.amount_msat);

            db::create_receive_payment(&state.db, record.clone()).await;

            push_events(state.clone(), offer.user_pk.clone(), record).await;

            (payment_id, "successful")
        }
        None => {
            let payment_id = state
                .node
                .bolt12_payment()
                .send_using_amount(&offer, request.amount_msat, None, None)
                .inspect_err(|error| error!(?error, "ldk node bolt12 send error"))
                .map_err(|e| e.to_string())?;

            (payment_id.0, "pending")
        }
    };

    let send_record = db::create_send_payment(
        &state.db,
        payment_id,
        user_pk.clone(),
        request.amount_msat as i64,
        fee_msat as i64,
        offer.description().unwrap().to_string(),
        offer.to_string(),
        status.to_string(),
        None,
    )
    .await;

    drop(send_lock);

    push_events(state, user_pk, send_record).await;

    Ok(())
}

async fn check_send_request(
    state: Arc<AppState>,
    user_pk: String,
    amount_msat: u64,
) -> Result<u64, String> {
    let pending_payments = db::count_pending_sends(&state.db, user_pk.clone()).await;

    if pending_payments >= state.args.max_pending_payments_per_user as i64 {
        return Err("Too many pending payments".to_string());
    }

    check_amount_bounds(state.clone(), amount_msat)?;

    let fee_msat = state.get_fee_msat(amount_msat);

    let balance_msat = db::user_balance(&state.db, user_pk.clone()).await;

    if balance_msat < amount_msat + fee_msat {
        return Err("Insufficient balance".to_string());
    }

    Ok(fee_msat)
}

fn check_amount_bounds(state: Arc<AppState>, amount_msat: u64) -> Result<(), String> {
    if amount_msat < state.args.min_amount_sats as u64 * 1000 {
        return Err(format!(
            "The minimum amount is {} sats",
            state.args.min_amount_sats
        ));
    }

    if amount_msat > state.args.max_amount_sats as u64 * 1000 {
        return Err(format!(
            "The maximum amount is {} sats",
            state.args.max_amount_sats
        ));
    }

    Ok(())
}

async fn push_events<R: Into<Payment>>(state: Arc<AppState>, user_pk: String, record: R) {
    let balance_msat = db::user_balance(&state.db, user_pk.clone()).await;

    state
        .event_bus
        .send_balance_event(user_pk.clone(), balance_msat);

    state
        .event_bus
        .send_payment_event(user_pk.clone(), record.into());
}

fn sending_parameters(fee_msat: u64) -> SendingParameters {
    SendingParameters {
        max_total_routing_fee_msat: Some(Some(fee_msat)),
        max_total_cltv_expiry_delta: None,
        max_path_count: None,
        max_channel_saturation_power_of_half: None,
    }
}

pub async fn fees(
    state: Arc<AppState>,
    _user_pk: String,
    _request: (),
) -> Result<FeesResponse, String> {
    Ok(FeesResponse {
        fee_ppm: state.args.fee_ppm,
        base_fee_msat: state.args.base_fee_msat,
    })
}

pub async fn bolt12_receive_variable_amount(
    state: Arc<AppState>,
    user_pk: String,
    _request: (),
) -> Result<Bolt12ReceiveResponse, String> {
    if let Some(offer_record) = db::get_offer_by_user_pk(&state.db, user_pk.clone()).await {
        return Ok(Bolt12ReceiveResponse {
            offer: offer_record.pr.to_string(),
        });
    }

    let offer = state
        .node
        .bolt12_payment()
        .receive_variable_amount("", None)
        .expect("Failed to create offer with variable amount");

    db::create_offer(
        &state.db,
        user_pk.clone(),
        offer.clone(),
        None,
        String::new(),
        None,
    )
    .await;

    Ok(Bolt12ReceiveResponse {
        offer: offer.to_string(),
    })
}

/// Event stream for a user
pub async fn events(
    state: Arc<AppState>,
    user_pk: String,
) -> impl Stream<Item = Result<AppEvent, String>> + Send + 'static {
    let stream = state.event_bus.clone().subscribe_to_events(user_pk.clone());

    let balance = Balance {
        amount_msat: db::user_balance(&state.db, user_pk.clone()).await,
    };

    let balance_event = AppEvent::Balance(balance.clone());

    let payments = db::user_payments(&state.db, user_pk.clone()).await;

    let payment_events = payments.into_iter().map(AppEvent::Payment);

    stream::once(future::ready(Ok(balance_event)))
        .chain(stream::iter(payment_events.map(Ok)))
        .chain(stream)
}
