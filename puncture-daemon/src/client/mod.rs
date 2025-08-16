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

use puncture_client_core::{
    AppEvent, Balance, ClientRpcRequest, ENDPOINT_BOLT11_RECEIVE, ENDPOINT_BOLT11_SEND,
    ENDPOINT_BOLT12_RECEIVE, ENDPOINT_BOLT12_SEND, ENDPOINT_ONCHAIN_SEND, ENDPOINT_RECOVER,
    ENDPOINT_REGISTER, ENDPOINT_SET_RECOVERY_NAME,
};

use crate::AppState;

macro_rules! client_method {
    ($func:ident, $state:expr, $user_id:expr, $params:expr, $auth:expr) => {{
        async move {
            let mut conn = $state.db.get_connection().await;

            if $auth && !db::user_exists(&mut conn, $user_id.clone()).await {
                return Err("Method requires a registered user".to_string());
            }

            drop(conn);

            match serde_json::from_value($params) {
                Ok(request) => rpc::$func($state, $user_id, request)
                    .await
                    .map(|response| serde_json::to_value(response).unwrap()),
                Err(e) => Err(format!("Failed to deserialize request: {}", e)),
            }
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

    let response = match request.method.as_str() {
        ENDPOINT_REGISTER => client_method!(register, state, user_id, request.request, false).await,
        ENDPOINT_BOLT11_RECEIVE => {
            client_method!(bolt11_receive, state, user_id, request.request, true).await
        }
        ENDPOINT_BOLT12_RECEIVE => {
            client_method!(bolt12_receive, state, user_id, request.request, true).await
        }
        ENDPOINT_BOLT11_SEND => {
            client_method!(bolt11_send, state, user_id, request.request, true).await
        }
        ENDPOINT_BOLT12_SEND => {
            client_method!(bolt12_send, state, user_id, request.request, true).await
        }
        ENDPOINT_ONCHAIN_SEND => {
            client_method!(onchain_send, state, user_id, request.request, true).await
        }
        ENDPOINT_SET_RECOVERY_NAME => {
            client_method!(set_recovery_name, state, user_id, request.request, true).await
        }
        ENDPOINT_RECOVER => client_method!(recover, state, user_id, request.request, true).await,
        _ => Err(format!("Method '{}' not found", request.method)),
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

    let balance = Ok(AppEvent::Balance(Balance { amount_msat }));

    let payments = db::user_payments(&mut conn, user_pk.clone())
        .await
        .into_iter()
        .map(AppEvent::Payment)
        .map(Ok);

    stream::once(future::ready(balance))
        .chain(stream::iter(payments))
        .chain(stream)
}
