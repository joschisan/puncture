use std::str::FromStr;

use axum::extract::{Json, State};
use bitcoin::hex::{DisplayHex, FromHex};
use bitcoin::{FeeRate, Txid};
use ldk_node::UserChannelId;
use lightning::ln::msgs::SocketAddress;
use rand::Rng;
use serde_json::Value;
use tracing::info;

use puncture_cli_core::{
    BalancesResponse, ChannelInfo, CloseChannelRequest, ConnectPeerRequest, DisconnectPeerRequest,
    InviteRequest, InviteResponse, ListChannelsResponse, ListPeersResponse, ListUsersResponse,
    NodeIdResponse, OnchainDrainRequest, OnchainReceiveResponse, OnchainSendRequest,
    OpenChannelRequest, OpenChannelResponse, PeerInfo, RecoverRequest, RecoverResponse,
    RequestChannelRequest, RequestChannelResponse,
};
use puncture_core::PunctureCode;

use crate::AppState;

use super::{CliError, db};

#[axum::debug_handler]
pub async fn ldk_node_id(State(state): State<AppState>) -> Json<NodeIdResponse> {
    Json(NodeIdResponse {
        node_id: state.node.node_id(),
    })
}

#[axum::debug_handler]
pub async fn ldk_balances(
    State(state): State<AppState>,
) -> Result<Json<BalancesResponse>, CliError> {
    let total_onchain_balance_sats = state.node.list_balances().total_onchain_balance_sats;

    let total_inbound_capacity_msat = state
        .node
        .list_channels()
        .into_iter()
        .filter(|c| c.is_usable)
        .map(|c| c.inbound_capacity_msat)
        .sum::<u64>();

    let total_outbound_capacity_msat = state
        .node
        .list_channels()
        .into_iter()
        .filter(|c| c.is_usable)
        .map(|c| c.outbound_capacity_msat)
        .sum::<u64>();

    Ok(Json(BalancesResponse {
        total_onchain_balance_sats,
        total_inbound_capacity_msat,
        total_outbound_capacity_msat,
    }))
}

#[axum::debug_handler]
pub async fn ldk_onchain_receive(
    State(state): State<AppState>,
) -> Result<Json<OnchainReceiveResponse>, CliError> {
    let address = state
        .node
        .onchain_payment()
        .new_address()
        .map_err(CliError::internal)?;

    info!(?address, "generated new onchain address");

    Ok(Json(OnchainReceiveResponse {
        address: address.into_unchecked(),
    }))
}

#[axum::debug_handler]
pub async fn ldk_onchain_send(
    State(state): State<AppState>,
    Json(request): Json<OnchainSendRequest>,
) -> Result<Json<Txid>, CliError> {
    let address = request
        .address
        .require_network(state.args.bitcoin_network)
        .map_err(|_| CliError::bad_request("Address is for a different network"))?;

    state
        .node
        .onchain_payment()
        .send_to_address(
            &address,
            request.amount_sats,
            request
                .sats_per_vbyte
                .map(FeeRate::from_sat_per_vb_unchecked),
        )
        .map(Json)
        .map_err(CliError::internal)
}

#[axum::debug_handler]
pub async fn ldk_onchain_drain(
    State(state): State<AppState>,
    Json(request): Json<OnchainDrainRequest>,
) -> Result<Json<Txid>, CliError> {
    if !state.node.list_channels().is_empty() {
        return Err(CliError::bad_request("You still have channels open"));
    }

    let address = request
        .address
        .require_network(state.args.bitcoin_network)
        .map_err(|_| CliError::bad_request("Address is for a different network"))?;

    state
        .node
        .onchain_payment()
        .send_all_to_address(
            &address,
            false,
            request
                .sats_per_vbyte
                .map(FeeRate::from_sat_per_vb_unchecked),
        )
        .map(Json)
        .map_err(CliError::internal)
}

#[axum::debug_handler]
pub async fn ldk_channel_open(
    State(state): State<AppState>,
    Json(request): Json<OpenChannelRequest>,
) -> Result<Json<OpenChannelResponse>, CliError> {
    let channel_id = match request.public {
        true => state.node.open_announced_channel(
            request.node_id,
            SocketAddress::from_str(&request.socket_address).map_err(CliError::bad_request)?,
            request.channel_amount_sats,
            request.push_to_counterparty_msat,
            None,
        ),
        false => state.node.open_channel(
            request.node_id,
            SocketAddress::from_str(&request.socket_address).map_err(CliError::bad_request)?,
            request.channel_amount_sats,
            request.push_to_counterparty_msat,
            None,
        ),
    }
    .map_err(CliError::internal)?;

    info!(?request, ?channel_id, "opened channel");

    Ok(Json(OpenChannelResponse {
        channel_id: channel_id.0.to_string(),
    }))
}

#[axum::debug_handler]
pub async fn ldk_channel_close(
    State(state): State<AppState>,
    Json(request): Json<CloseChannelRequest>,
) -> Result<Json<()>, CliError> {
    let channel_id = <[u8; 16]>::from_hex(&request.user_channel_id)
        .map(u128::from_be_bytes)
        .map(UserChannelId)
        .map_err(CliError::bad_request)?;

    match request.force {
        true => {
            state
                .node
                .force_close_channel(&channel_id, request.counterparty_node_id, None)
                .map_err(CliError::internal)?;
        }
        false => {
            state
                .node
                .close_channel(&channel_id, request.counterparty_node_id)
                .map_err(CliError::internal)?;
        }
    }

    info!(?request, "closed channel");

    Ok(Json(()))
}

#[axum::debug_handler]
pub async fn ldk_channel_list(
    State(state): State<AppState>,
    Json(_request): Json<Value>,
) -> Result<Json<ListChannelsResponse>, CliError> {
    let channels = state
        .node
        .list_channels()
        .into_iter()
        .map(|channel| ChannelInfo {
            user_channel_id: channel.user_channel_id.0.to_be_bytes().as_hex().to_string(),
            counterparty_node_id: channel.counterparty_node_id,
            channel_value_sats: channel.channel_value_sats,
            outbound_capacity_msat: channel.outbound_capacity_msat,
            inbound_capacity_msat: channel.inbound_capacity_msat,
            is_channel_ready: channel.is_channel_ready,
            is_usable: channel.is_usable,
            is_outbound: channel.is_outbound,
            funding_txo: channel.funding_txo,
            confirmations: channel.confirmations,
            confirmations_required: channel.confirmations_required,
        })
        .collect();

    Ok(Json(ListChannelsResponse { channels }))
}

#[axum::debug_handler]
pub async fn ldk_channel_request(
    State(state): State<AppState>,
    Json(request): Json<RequestChannelRequest>,
) -> Result<Json<RequestChannelResponse>, CliError> {
    // Connect to Megalith LSP
    state
        .node
        .connect(
            "038a9e56512ec98da2b5789761f7af8f280baf98a09282360cd6ff1381b5e889bf"
                .parse()
                .unwrap(),
            "64.23.162.51:9735".parse().unwrap(),
            true,
        )
        .map_err(|_| CliError::internal("Failed to connect to Megalith LSP node"))?;

    info!("Ensured connection to Megalith LSP node");

    let client = reqwest::Client::new();

    // Create request payload for Megalith LSPS1 API
    let payload = serde_json::json!({
        "lsp_balance_sat": request.lsp_balance_sat.to_string(),
        "client_balance_sat": request.client_balance_sat.to_string(),
        "required_channel_confirmations": 0,
        "funding_confirms_within_blocks": 6,
        "channel_expiry_blocks": request.channel_expiry_blocks,
        "token": "",
        "refund_on_chain_address": null,
        "announce_channel": request.public,
        "public_key": state.node.node_id().to_string()
    });

    // Make HTTP request to Megalith LSPS1 API
    let response = client
        .post("https://megalithic.me/api/lsps1/v1/create_order")
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
        .map_err(|e| CliError::internal(format!("Failed to call Megalith API: {e}")))?;

    if !response.status().is_success() {
        return Err(CliError::bad_request(format!(
            "Megalith API error: {}",
            response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string())
        )));
    }

    // Parse response to get the BOLT11 invoice
    let api_response: Value = response
        .json()
        .await
        .map_err(|e| CliError::internal(format!("Failed to parse Megalith response: {e}")))?;

    let invoice = api_response
        .get("payment")
        .and_then(|v| v.get("bolt11"))
        .and_then(|v| v.get("invoice"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| CliError::internal("Missing invoice in Megalith response"))?;

    info!(?request, ?invoice, "requested channel from Megalith LSP");

    Ok(Json(RequestChannelResponse {
        invoice: invoice.to_string(),
    }))
}

#[axum::debug_handler]
pub async fn ldk_peer_connect(
    State(state): State<AppState>,
    Json(request): Json<ConnectPeerRequest>,
) -> Result<Json<()>, CliError> {
    state
        .node
        .connect(
            request.node_id,
            SocketAddress::from_str(&request.socket_address).map_err(CliError::bad_request)?,
            request.persist,
        )
        .map_err(CliError::internal)?;

    info!(?request, "connected to peer");

    Ok(Json(()))
}

#[axum::debug_handler]
pub async fn ldk_peer_disconnect(
    State(state): State<AppState>,
    Json(request): Json<DisconnectPeerRequest>,
) -> Result<Json<()>, CliError> {
    state
        .node
        .disconnect(request.counterparty_node_id)
        .map_err(CliError::internal)?;

    info!(?request, "disconnected from peer");

    Ok(Json(()))
}

#[axum::debug_handler]
pub async fn ldk_peer_list(
    State(state): State<AppState>,
    Json(_request): Json<Value>,
) -> Result<Json<ListPeersResponse>, CliError> {
    let peers = state
        .node
        .list_peers()
        .into_iter()
        .map(|peer| PeerInfo {
            node_id: peer.node_id,
            address: peer.address.to_string(),
            is_persisted: peer.is_persisted,
            is_connected: peer.is_connected,
        })
        .collect();

    Ok(Json(ListPeersResponse { peers }))
}

#[tracing::instrument(skip(state))]
pub async fn user_invite(
    State(state): State<AppState>,
    Json(request): Json<InviteRequest>,
) -> Result<Json<InviteResponse>, CliError> {
    let invite_id = rand::rng().random();

    db::create_invite(
        &state.db,
        &invite_id,
        request.user_limit,
        request.expiry_days * 60 * 60 * 24,
    )
    .await;

    Ok(Json(InviteResponse {
        invite: PunctureCode::invite(invite_id, state.node_id).encode(),
    }))
}

#[tracing::instrument(skip(state))]
pub async fn user_recover(
    State(state): State<AppState>,
    Json(request): Json<RecoverRequest>,
) -> Result<Json<RecoverResponse>, CliError> {
    if !db::user_exists(&state.db, request.user_pk.clone()).await {
        return Err(CliError::bad_request("User does not exist"));
    }

    let recovery_id = rand::rng().random();

    db::create_recovery(&state.db, &recovery_id, &request.user_pk, 60 * 60 * 24).await;

    Ok(Json(RecoverResponse {
        recovery: PunctureCode::recovery(recovery_id).encode(),
    }))
}

pub async fn user_list(State(state): State<AppState>) -> Result<Json<ListUsersResponse>, CliError> {
    Ok(Json(ListUsersResponse {
        users: db::list_users(&state.db).await,
    }))
}
