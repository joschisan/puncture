use std::fmt::Display;

use axum::{
    Router,
    extract::{Json, State},
    http::StatusCode,
    response::IntoResponse,
    routing::post,
};
use bitcoin::hex::{DisplayHex, FromHex};
use ldk_node::UserChannelId;
use rand::Rng;
use serde_json::Value;
use tracing::info;

use puncture_cli_core::{
    BalancesResponse, ChannelInfo, CloseChannelRequest, ConnectPeerRequest, DisconnectPeerRequest,
    InviteRequest, InviteResponse, ListChannelsResponse, ListPeersResponse, ListUsersResponse,
    NewAddressResponse, NodeIdResponse, OnchainSendRequest, OpenChannelRequest,
    OpenChannelResponse, PeerInfo,
};
use puncture_core::invite::Invite;

use crate::{AppState, db};

pub struct ApiError {
    pub code: StatusCode,
    pub error: String,
}

impl ApiError {
    fn bad_request(error: impl Display) -> Self {
        Self {
            code: StatusCode::BAD_REQUEST,
            error: error.to_string(),
        }
    }

    fn internal(error: impl Display) -> Self {
        Self {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            error: error.to_string(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        (self.code, self.error).into_response()
    }
}

#[axum::debug_handler]
pub async fn ldk_node_id(State(state): State<AppState>) -> Json<NodeIdResponse> {
    Json(NodeIdResponse {
        node_id: state.node.node_id(),
    })
}

#[axum::debug_handler]
pub async fn ldk_onchain_receive(
    State(state): State<AppState>,
) -> Result<Json<NewAddressResponse>, ApiError> {
    let address = state
        .node
        .onchain_payment()
        .new_address()
        .map_err(ApiError::internal)?;

    info!(?address, "generated new onchain address");

    Ok(Json(NewAddressResponse {
        address: address.into_unchecked(),
    }))
}

#[axum::debug_handler]
pub async fn ldk_onchain_send(
    State(state): State<AppState>,
    Json(request): Json<OnchainSendRequest>,
) -> Result<Json<()>, ApiError> {
    state
        .node
        .onchain_payment()
        .send_to_address(
            &request.address.clone().assume_checked(),
            request.amount_sats,
            request.fee_rate,
        )
        .map_err(ApiError::internal)?;

    info!(?request, "sent onchain payment");

    Ok(Json(()))
}

#[axum::debug_handler]
pub async fn ldk_balances(
    State(state): State<AppState>,
) -> Result<Json<BalancesResponse>, ApiError> {
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
pub async fn ldk_channel_open(
    State(state): State<AppState>,
    Json(request): Json<OpenChannelRequest>,
) -> Result<Json<OpenChannelResponse>, ApiError> {
    let channel_id = state
        .node
        .open_announced_channel(
            request.node_id,
            request.address.into(),
            request.channel_amount_sats,
            request.push_to_counterparty_msat,
            None,
        )
        .map_err(ApiError::internal)?;

    info!(?request, ?channel_id, "opened channel");

    Ok(Json(OpenChannelResponse {
        channel_id: channel_id.0.to_string(),
    }))
}

#[axum::debug_handler]
pub async fn ldk_channel_close(
    State(state): State<AppState>,
    Json(request): Json<CloseChannelRequest>,
) -> Result<Json<()>, ApiError> {
    let channel_id = <[u8; 16]>::from_hex(&request.user_channel_id)
        .map(u128::from_be_bytes)
        .map(UserChannelId)
        .map_err(ApiError::bad_request)?;

    match request.force {
        true => {
            state
                .node
                .force_close_channel(&channel_id, request.counterparty_node_id, None)
                .map_err(ApiError::internal)?;
        }
        false => {
            state
                .node
                .close_channel(&channel_id, request.counterparty_node_id)
                .map_err(ApiError::internal)?;
        }
    }

    info!(?request, "closed channel");

    Ok(Json(()))
}

pub async fn ldk_channel_list(
    State(state): State<AppState>,
    Json(_request): Json<Value>,
) -> Result<Json<ListChannelsResponse>, ApiError> {
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
            confirmations: channel.confirmations,
            confirmations_required: channel.confirmations_required,
        })
        .collect();

    Ok(Json(ListChannelsResponse { channels }))
}

pub async fn user_list(State(state): State<AppState>) -> Result<Json<ListUsersResponse>, ApiError> {
    Ok(Json(ListUsersResponse {
        users: db::list_users(&state.db).await,
    }))
}

#[axum::debug_handler]
pub async fn ldk_peer_connect(
    State(state): State<AppState>,
    Json(request): Json<ConnectPeerRequest>,
) -> Result<Json<()>, ApiError> {
    state
        .node
        .connect(request.node_id, request.address.into(), request.persist)
        .map_err(ApiError::internal)?;

    info!(?request, "connected to peer");

    Ok(Json(()))
}

#[axum::debug_handler]
pub async fn ldk_peer_disconnect(
    State(state): State<AppState>,
    Json(request): Json<DisconnectPeerRequest>,
) -> Result<Json<()>, ApiError> {
    state
        .node
        .disconnect(request.counterparty_node_id)
        .map_err(ApiError::internal)?;

    info!(?request, "disconnected from peer");

    Ok(Json(()))
}

#[axum::debug_handler]
pub async fn ldk_peer_list(
    State(state): State<AppState>,
    Json(_request): Json<Value>,
) -> Result<Json<ListPeersResponse>, ApiError> {
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
pub async fn invite(
    State(state): State<AppState>,
    Json(request): Json<InviteRequest>,
) -> Result<Json<InviteResponse>, ApiError> {
    let invite_id = rand::rng().random();

    db::create_invite(
        &state.db,
        &invite_id,
        request.user_limit,
        request.expiry_days * 60 * 60 * 24,
    )
    .await;

    Ok(Json(InviteResponse {
        invite: Invite::new(invite_id, state.endpoint.node_id()).encode(),
    }))
}

pub fn router() -> Router<AppState> {
    let onchain_router = Router::new()
        .route("/receive", post(ldk_onchain_receive))
        .route("/send", post(ldk_onchain_send));

    let channel_router = Router::new()
        .route("/open", post(ldk_channel_open))
        .route("/close", post(ldk_channel_close))
        .route("/list", post(ldk_channel_list));

    let peer_router = Router::new()
        .route("/connect", post(ldk_peer_connect))
        .route("/disconnect", post(ldk_peer_disconnect))
        .route("/list", post(ldk_peer_list));

    let ldk_router = Router::new()
        .route("/node-id", post(ldk_node_id))
        .route("/balances", post(ldk_balances))
        .nest("/onchain", onchain_router)
        .nest("/channel", channel_router)
        .nest("/peer", peer_router);

    let user_router = Router::new().route("/list", post(user_list));

    Router::new()
        .nest("/ldk", ldk_router)
        .nest("/user", user_router)
        .route("/invite", post(invite))
}
