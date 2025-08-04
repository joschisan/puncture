mod db;
mod rpc;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::{future, sync::Arc};

use anyhow::{Result, anyhow, ensure};
use dashmap::DashMap;
use futures::stream;
use iroh::endpoint::Connection;
use iroh::{Endpoint, endpoint::Incoming};
use serde_json::Value;
use tokio_stream::{Stream, StreamExt};
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use puncture_client_core::{AppEvent, Balance, ClientRpcRequest};

use crate::AppState;

macro_rules! method {
    ($func:ident, $state:expr, $user_id:expr, $params:expr) => {{
        match serde_json::from_value($params) {
            Ok(request) => rpc::$func($state, $user_id, request).await.map(|response| {
                serde_json::to_value(response).expect("Failed to serialize response")
            }),
            Err(_) => Err("Failed to deserialize request".to_string()),
        }
    }};
}

pub async fn run_api(endpoint: Endpoint, app_state: AppState, ct: CancellationToken) {
    info!(
        "Starting Iroh API server with node_id: {}",
        endpoint.node_id()
    );

    let app_state = Arc::new(app_state);

    let semaphore = Arc::new(DashMap::new());

    loop {
        tokio::select! {
            incoming = endpoint.accept() => {
                tokio::spawn(handle_connection(app_state.clone(), semaphore.clone(), incoming.unwrap(), ct.clone()));
            }
            _ = ct.cancelled() => {
                break;
            }
        }
    }

    endpoint.close().await;
}

async fn handle_connection(
    app_state: Arc<AppState>,
    semaphore: Arc<DashMap<String, AtomicUsize>>,
    incoming: Incoming,
    ct: CancellationToken,
) {
    if let Err(e) = handle_connection_inner(app_state, semaphore, incoming, ct).await {
        warn!(?e, "Error handling connection");
    }
}

async fn handle_connection_inner(
    app_state: Arc<AppState>,
    semaphore: Arc<DashMap<String, AtomicUsize>>,
    incoming: Incoming,
    ct: CancellationToken,
) -> anyhow::Result<()> {
    let connection = incoming.accept()?.await?;

    let node_id = connection.remote_node_id()?.to_string();

    let counter = semaphore
        .entry(node_id.clone())
        .or_insert_with(|| AtomicUsize::new(0));

    ensure!(
        counter.load(Ordering::Relaxed) < 10,
        "User has reached maximum of 10 simultaneous connections"
    );

    counter.fetch_add(1, Ordering::Relaxed);

    drop(counter);

    drive_connection(
        app_state.clone(),
        connection.clone(),
        node_id.clone(),
        ct.clone(),
    )
    .await
    .inspect_err(|e| warn!(?e, "Error while driving connection"))
    .ok();

    connection.closed().await;

    semaphore
        .get(&node_id)
        .expect("Counter not found")
        .fetch_sub(1, Ordering::Relaxed);

    Ok(())
}

async fn drive_connection(
    app_state: Arc<AppState>,
    connection: Connection,
    node_id: String,
    ct: CancellationToken,
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
            _ = ct.cancelled() => {
                return Ok(());
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

    let request: ClientRpcRequest<Value> = serde_json::from_slice(&request)?;

    let mut conn = state.db.get_connection().await;

    let user_exists = db::user_exists(&mut conn, user_id.clone()).await;

    drop(conn);

    let response = if user_exists {
        match request.method.as_str() {
            "register" => method!(register, state, user_id, request.request),
            "fees" => method!(fees, state, user_id, request.request),
            "bolt11_receive" => {
                method!(bolt11_receive, state, user_id, request.request)
            }
            "bolt12_receive_variable_amount" => method!(
                bolt12_receive_variable_amount,
                state,
                user_id,
                request.request
            ),
            "bolt11_send" => method!(bolt11_send, state, user_id, request.request),
            "bolt12_send" => method!(bolt12_send, state, user_id, request.request),
            "set_recovery_name" => method!(set_recovery_name, state, user_id, request.request),
            "recover" => method!(recover, state, user_id, request.request),
            _ => Err(format!("Method '{}' not found", request.method)),
        }
    } else {
        match request.method.as_str() {
            "register" => method!(register, state, user_id, request.request),
            _ => Err(format!("Method '{}' not found", request.method)),
        }
    };

    let response = serde_json::to_vec(&response).expect("Failed to serialize response");

    send_stream.write_all(&response).await?;

    send_stream.finish()?;

    Ok(())
}

/// Event stream for a user
pub async fn events(
    state: Arc<AppState>,
    user_pk: String,
) -> impl Stream<Item = Result<AppEvent, String>> + Send + 'static {
    let stream = state.event_bus.clone().subscribe_to_events(user_pk.clone());

    let mut conn = state.db.get_connection().await;

    let amount_msat = crate::db::user_balance(&mut conn, user_pk.clone()).await;

    let balance_event = AppEvent::Balance(Balance { amount_msat });

    let payments = db::user_payments(&mut conn, user_pk.clone()).await;

    let payment_events = payments
        .into_iter()
        .rev()
        .take(50)
        .rev()
        .map(AppEvent::Payment);

    stream::once(future::ready(Ok(balance_event)))
        .chain(stream::iter(payment_events.map(Ok)))
        .chain(stream)
}
