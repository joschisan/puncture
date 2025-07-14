mod balances;
mod channels;
mod db;
mod peers;
mod shared;
mod users;

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
        .route("/", get(balances::balances_page))
        .route("/channels", get(channels::channels_page))
        .route("/channels/open", post(channels::open_channel_submit))
        .route("/channels/close", post(channels::close_channel_submit))
        .route("/peers", get(peers::peers_page))
        .route("/peers/connect", post(peers::connect_peer_submit))
        .route("/peers/disconnect", post(peers::disconnect_peer_submit))
        .route("/onchain/receive", post(balances::onchain_receive_submit))
        .route("/onchain/send", post(balances::onchain_send_submit))
        .route("/onchain/drain", post(balances::onchain_drain_submit))
        .route("/users", get(users::users_page))
        .route("/users/invite", post(users::invite_submit))
}
