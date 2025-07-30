mod db;
mod rpc;

use std::fmt::Display;

use axum::{Router, http::StatusCode, response::IntoResponse, routing::post};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

use puncture_cli_core::{
    ROUTE_LDK_BALANCES, ROUTE_LDK_CHANNEL_CLOSE, ROUTE_LDK_CHANNEL_LIST, ROUTE_LDK_CHANNEL_OPEN,
    ROUTE_LDK_CHANNEL_REQUEST, ROUTE_LDK_NODE_ID, ROUTE_LDK_ONCHAIN_DRAIN,
    ROUTE_LDK_ONCHAIN_RECEIVE, ROUTE_LDK_ONCHAIN_SEND, ROUTE_LDK_PEER_CONNECT,
    ROUTE_LDK_PEER_DISCONNECT, ROUTE_LDK_PEER_LIST, ROUTE_USER_INVITE, ROUTE_USER_LIST,
    ROUTE_USER_RECOVER,
};

use crate::AppState;

pub async fn run_cli(app_state: AppState, ct: CancellationToken) {
    let listener = TcpListener::bind(app_state.args.cli_bind)
        .await
        .expect("Failed to bind CLI server");

    axum::serve(listener, router().with_state(app_state))
        .with_graceful_shutdown(ct.cancelled_owned())
        .await
        .expect("Failed to start CLI server");
}

pub struct CliError {
    pub code: StatusCode,
    pub error: String,
}

impl CliError {
    pub fn bad_request(error: impl Display) -> Self {
        Self {
            code: StatusCode::BAD_REQUEST,
            error: error.to_string(),
        }
    }

    pub fn internal(error: impl Display) -> Self {
        Self {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            error: error.to_string(),
        }
    }
}

impl IntoResponse for CliError {
    fn into_response(self) -> axum::response::Response {
        (self.code, self.error).into_response()
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(ROUTE_LDK_NODE_ID, post(rpc::ldk_node_id))
        .route(ROUTE_LDK_BALANCES, post(rpc::ldk_balances))
        .route(ROUTE_LDK_ONCHAIN_RECEIVE, post(rpc::ldk_onchain_receive))
        .route(ROUTE_LDK_ONCHAIN_SEND, post(rpc::ldk_onchain_send))
        .route(ROUTE_LDK_ONCHAIN_DRAIN, post(rpc::ldk_onchain_drain))
        .route(ROUTE_LDK_CHANNEL_OPEN, post(rpc::ldk_channel_open))
        .route(ROUTE_LDK_CHANNEL_CLOSE, post(rpc::ldk_channel_close))
        .route(ROUTE_LDK_CHANNEL_LIST, post(rpc::ldk_channel_list))
        .route(ROUTE_LDK_CHANNEL_REQUEST, post(rpc::ldk_channel_request))
        .route(ROUTE_LDK_PEER_CONNECT, post(rpc::ldk_peer_connect))
        .route(ROUTE_LDK_PEER_DISCONNECT, post(rpc::ldk_peer_disconnect))
        .route(ROUTE_LDK_PEER_LIST, post(rpc::ldk_peer_list))
        .route(ROUTE_USER_INVITE, post(rpc::user_invite))
        .route(ROUTE_USER_RECOVER, post(rpc::user_recover))
        .route(ROUTE_USER_LIST, post(rpc::user_list))
}
