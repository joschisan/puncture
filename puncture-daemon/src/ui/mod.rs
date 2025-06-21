mod db;
mod lightning;
mod onchain;
mod shared;

use axum::{
    Router,
    routing::{get, post},
};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

use crate::AppState;

pub async fn run_ui(app_state: AppState, ct: CancellationToken) {
    let listener = TcpListener::bind(app_state.args.ui_bind)
        .await
        .expect("Failed to bind UI server");

    axum::serve(listener, create_router().with_state(app_state))
        .with_graceful_shutdown(ct.cancelled_owned())
        .await
        .expect("Failed to start UI server");
}

fn create_router() -> Router<AppState> {
    Router::new()
        .route("/", get(lightning::lightning_page))
        .route("/lightning", get(lightning::lightning_page))
        .route(
            "/lightning/channel/open",
            post(lightning::open_channel_submit),
        )
        .route(
            "/lightning/channel/request",
            post(lightning::request_channel_submit),
        )
        .route(
            "/lightning/channel/close",
            post(lightning::close_channel_submit),
        )
        .route(
            "/lightning/peer/connect",
            post(lightning::connect_peer_submit),
        )
        .route(
            "/lightning/peer/disconnect",
            post(lightning::disconnect_peer_submit),
        )
        .route("/onchain", get(onchain::onchain_page))
        .route("/onchain/receive", post(onchain::onchain_receive_submit))
        .route("/onchain/send", post(onchain::onchain_send_submit))
        .route("/onchain/drain", post(onchain::onchain_drain_submit))
        .route("/lightning/users/invite", post(lightning::invite_submit))
}
