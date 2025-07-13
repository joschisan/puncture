use axum::{
    Form,
    extract::State,
    response::{Html, IntoResponse},
};
use bitcoin::secp256k1::PublicKey;
use lightning::ln::msgs::SocketAddress;
use maud::{Markup, html};
use serde::Deserialize;
use std::str::FromStr;

use super::shared::{base_template, copyable_hex_input, inline_error};
use crate::AppState;

pub fn peers_template(peers: &[ldk_node::PeerDetails]) -> Markup {
    let content = html! {
        div class="row g-4" {
                @for peer in peers {
                    div class="col-12" {
                        div class="card h-100 overflow-hidden" {
                            div class="card-header" {
                                h5 class="card-title mb-0" { "Peer" }
                            }
                            div class="card-body" {
                                table class="table table-sm table-borderless mb-0" {
                                    tbody {
                                        tr {
                                            td class="fw-bold" { "node_id:" }
                                            td {
                                                (copyable_hex_input(&peer.node_id.to_string(), None))
                                            }
                                        }
                                        tr {
                                            td class="fw-bold" { "address:" }
                                            td {
                                                span class="font-monospace small" {
                                                    (peer.address.to_string())
                                                }
                                            }
                                        }
                                        tr {
                                            td class="fw-bold" { "is_persisted:" }
                                            td {
                                                (peer.is_persisted)
                                            }
                                        }
                                        tr {
                                            td class="fw-bold" { "is_connected:" }
                                            td {
                                                (peer.is_connected)
                                            }
                                        }
                                    }
                                }
                                form hx-post="/peers/disconnect"
                                     hx-target="this"
                                     hx-swap="outerHTML"
                                     class="mt-3" {
                                    input type="hidden" name="counterparty_node_id" value=(peer.node_id.to_string()) {}
                                    button type="submit" class="btn btn-outline-danger w-100" {
                                        "Disconnect"
                                    }
                                }
                            }
                        }
                    }
                }
            }
    };

    let action_sidebar = html! {
        div class="card" {
            div class="card-header" {
                h5 class="card-title mb-0" { "Connect Peer" }
            }
            div class="card-body" {
                form hx-post="/peers/connect"
                     hx-target="#connect-peer-results"
                     hx-swap="innerHTML" {
                    div class="mb-3" {
                        label for="node_id" class="form-label" { "Node ID" }
                        input type="text" class="form-control font-monospace" id="node_id" name="node_id" required placeholder="03abc..." {}
                    }
                    div class="mb-3" {
                        label for="socket_address" class="form-label" { "Socket Address" }
                        input type="text" class="form-control" id="socket_address" name="socket_address" required placeholder="127.0.0.1:9735" {}
                    }
                    div class="mb-3" {
                        div class="form-check" {
                            input class="form-check-input" type="checkbox" id="persist" name="persist" {}
                            label class="form-check-label" for="persist" { "Persist connection" }
                        }
                    }
                    button type="submit" class="btn btn-outline-primary w-100" { "Connect" }
                }

                div id="connect-peer-results" {}
            }
        }
    };

    base_template("Peers", "/peers", content, action_sidebar)
}

#[derive(Deserialize)]
pub struct ConnectPeerForm {
    pub node_id: String,
    pub socket_address: String,
    #[serde(default)]
    pub persist: bool,
}

#[derive(Deserialize)]
pub struct DisconnectPeerForm {
    pub counterparty_node_id: String,
}

pub async fn peers_page(State(state): State<AppState>) -> impl IntoResponse {
    Html(peers_template(&state.node.list_peers()).into_string())
}

pub async fn connect_peer_submit(
    State(state): State<AppState>,
    Form(form): Form<ConnectPeerForm>,
) -> impl IntoResponse {
    let node_id = match form.node_id.parse::<PublicKey>() {
        Ok(id) => id,
        Err(_) => return Html(inline_error("Invalid node ID format").into_string()),
    };

    let socket_address = match SocketAddress::from_str(&form.socket_address) {
        Ok(addr) => addr,
        Err(_) => return Html(inline_error("Invalid socket address format").into_string()),
    };

    match state.node.connect(node_id, socket_address, form.persist) {
        Ok(_) => Html("".to_string()), // Success - just clear form and stay on page
        Err(e) => Html(inline_error(&format!("Failed to connect to peer: {}", e)).into_string()),
    }
}

pub async fn disconnect_peer_submit(
    State(state): State<AppState>,
    Form(form): Form<DisconnectPeerForm>,
) -> impl IntoResponse {
    let counterparty_node_id = match form.counterparty_node_id.parse::<PublicKey>() {
        Ok(id) => id,
        Err(_) => return Html(inline_error("Invalid node ID format").into_string()),
    };

    match state.node.disconnect(counterparty_node_id) {
        Ok(_) => Html(
            html! {
                form class="mt-3" {
                    button class="btn btn-outline-danger w-100" disabled {
                        "This peer has been disconnected"
                    }
                }
            }
            .into_string(),
        ),
        Err(e) => {
            Html(inline_error(&format!("Failed to disconnect from peer: {}", e)).into_string())
        }
    }
}
