use std::{future, sync::Arc};

use anyhow::{Result, anyhow, ensure};
use bitcoin::hashes::Hash;
use futures::stream;
use iroh::endpoint::Connection;
use iroh::{Endpoint, endpoint::Incoming};
use ldk_node::payment::SendingParameters;
use lightning_invoice::{Bolt11InvoiceDescription, Description};
use serde::Deserialize;
use serde_json::Value;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio_stream::{Stream, StreamExt};
use tracing::{error, info, warn};

use puncture_api_core::{
    AppEvent, Balance, Bolt11QuoteResponse, Bolt11ReceiveResponse, ConfigResponse,
    UserBolt11QuoteRequest, UserBolt11ReceiveRequest, UserBolt11SendRequest,
};

use crate::{AppState, Bolt11Receive, db};

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
    let app_state = Arc::new(app_state);

    tracing::info!(
        "Starting Iroh API server with node_id: {}",
        endpoint.node_id()
    );

    while let Some(incoming) = endpoint.accept().await {
        if let Err(e) = handle_connection(app_state.clone(), incoming).await {
            warn!(?e, "Error handling connection");
        }
    }

    Ok(())
}

async fn handle_connection(app_state: Arc<AppState>, incoming: Incoming) -> anyhow::Result<()> {
    let connection = incoming.accept()?.await?;

    let node_id = connection.remote_node_id()?.to_string();

    if !db::user_exists(&app_state.db, node_id.clone()).await {
        ensure!(
            app_state.args.max_users as i64 > db::user_count(&app_state.db).await,
            "Max users reached, no more users can register"
        );

        db::register_user(&app_state.db, node_id.clone()).await;

        info!(?node_id, "New user registered");
    }

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

    tokio::spawn(async move {
        if let Err(e) = drive_connection(app_state.clone(), connection, node_id.clone()).await {
            warn!(?e, "Error while driving connection");
        }

        app_state
            .semaphore
            .get(&node_id)
            .expect("Counter not found")
            .fetch_sub(1, Ordering::Relaxed);
    });

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
        "config" => method!(config, state, user_id, request.request),
        "bolt11_receive" => {
            method!(bolt11_receive, state, user_id, request.request)
        }
        "bolt11_send" => method!(bolt11_send, state, user_id, request.request),
        "bolt11_quote" => method!(bolt11_quote, state, user_id, request.request),
        _ => Err(format!("Method '{}' not found", request.method)),
    };

    let response = serde_json::to_vec(&response).expect("Failed to serialize response");

    send_stream.write_all(&response).await?;

    send_stream.finish()?;

    Ok(())
}

pub async fn config(
    state: Arc<AppState>,
    _user_pk: String,
    _request: (),
) -> Result<ConfigResponse, String> {
    Ok(ConfigResponse {
        name: state.args.instance_name.clone(),
    })
}

pub async fn bolt11_receive(
    state: Arc<AppState>,
    user_pk: String,
    request: UserBolt11ReceiveRequest,
) -> Result<Bolt11ReceiveResponse, String> {
    info!(?request, "bolt11 receive");

    let pending = db::count_pending_invoices(&state.db, user_pk.clone()).await;

    if pending >= state.args.max_pending_payments_per_user as i64 {
        return Err("Too many pending invoices".to_string());
    }

    if request.amount_msat < state.args.min_amount_sats * 1000 {
        return Err(format!(
            "The minimum amount is {} sats",
            state.args.min_amount_sats
        ));
    }

    if request.amount_msat > state.args.max_amount_sats * 1000 {
        return Err(format!(
            "The maximum amount is {} sats",
            state.args.max_amount_sats
        ));
    }

    let invoice = state
        .node
        .bolt11_payment()
        .receive(
            request.amount_msat.into(),
            &Description::new(request.description.clone().unwrap_or_default())
                .map(Bolt11InvoiceDescription::Direct)
                .map_err(|e| e.to_string())?,
            state.args.invoice_expiry_secs,
        )
        .inspect_err(|error| error!(?error, "ldk node bolt11 receive error"))
        .map_err(|e| e.to_string())?;

    db::create_bolt11_invoice(
        &state.db,
        user_pk,
        invoice.clone(),
        request.amount_msat.into(),
        request.description.unwrap_or_default(),
        state.args.invoice_expiry_secs,
    )
    .await;

    Ok(Bolt11ReceiveResponse { invoice })
}

#[tracing::instrument(skip(state))]
pub async fn bolt11_send(
    state: Arc<AppState>,
    user_pk: String,
    request: UserBolt11SendRequest,
) -> Result<(), String> {
    let pending_payments = db::count_pending_bolt11_sends(&state.db, user_pk.clone()).await;

    if pending_payments >= state.args.max_pending_payments_per_user as i64 {
        return Err("Too many pending payments".to_string());
    }

    let amount_msat = request
        .invoice
        .amount_milli_satoshis()
        .ok_or("Invoice is missing amount".to_string())?
        .try_into()
        .map_err(|_| "Invalid invoice amount".to_string())?;

    if amount_msat < state.args.min_amount_sats as i64 * 1000 {
        return Err(format!(
            "The minimum amount is {} sats",
            state.args.min_amount_sats
        ));
    }

    if amount_msat > state.args.max_amount_sats as i64 * 1000 {
        return Err(format!(
            "The maximum amount is {} sats",
            state.args.max_amount_sats
        ));
    }

    let fee_msat = state.get_fee_msat(amount_msat);

    let send_lock = state.send_lock.lock().await;

    let balance_msat = db::user_balance(&state.db, user_pk.clone()).await as i64;

    if balance_msat < amount_msat + fee_msat {
        return Err("Insufficient balance".to_string());
    }

    let payment_hash = request.invoice.payment_hash().to_byte_array();

    let invoice_opt = db::bolt11_invoice(&state.db, payment_hash).await;

    let send_status = if let Some(invoice) = invoice_opt {
        if invoice.user_pk == user_pk {
            return Err("This is your own invoice".to_string());
        }

        db::create_bolt11_receive_payment(&state.db, invoice.clone().into()).await;

        let balance_msat = db::user_balance(&state.db, invoice.user_pk.clone()).await;

        state
            .event_bus
            .send_balance_event(invoice.user_pk.clone(), balance_msat);

        state.event_bus.send_payment_event(
            invoice.user_pk.clone(),
            Into::<Bolt11Receive>::into(invoice.clone()).into(),
        );

        "successful".to_string()
    } else {
        state
            .node
            .bolt11_payment()
            .send(&request.invoice, Some(sending_parameters(fee_msat)))
            .inspect_err(|error| error!(?error, "ldk node bolt11 send error"))
            .map_err(|e| e.to_string())?;

        "pending".to_string()
    };

    let send_record = db::create_bolt11_send_payment(
        &state.db,
        user_pk.clone(),
        request.invoice.clone(),
        amount_msat,
        fee_msat,
        request.ln_address.clone(),
        send_status,
    )
    .await;

    drop(send_lock);

    let balance_msat = db::user_balance(&state.db, user_pk.clone()).await;

    state
        .event_bus
        .send_balance_event(user_pk.clone(), balance_msat);

    state
        .event_bus
        .send_payment_event(user_pk.clone(), send_record.into());

    Ok(())
}

fn sending_parameters(amount_msat: i64) -> SendingParameters {
    SendingParameters {
        max_total_routing_fee_msat: Some(Some(amount_msat as u64)),
        max_total_cltv_expiry_delta: None,
        max_path_count: None,
        max_channel_saturation_power_of_half: None,
    }
}

pub async fn bolt11_quote(
    state: Arc<AppState>,
    _user_pk: String,
    request: UserBolt11QuoteRequest,
) -> Result<Bolt11QuoteResponse, String> {
    let amount_msat: i64 = request
        .invoice
        .amount_milli_satoshis()
        .ok_or("Invoice is missing amount".to_string())?
        .try_into()
        .map_err(|_| "Invalid invoice amount".to_string())?;

    let response = Bolt11QuoteResponse {
        amount_msat: amount_msat as u64,
        fee_msat: state.get_fee_msat(amount_msat) as u64,
        description: request.invoice.description().to_string(),
        expiry_secs: request.invoice.expiry_time().as_secs(),
    };

    Ok(response)
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
