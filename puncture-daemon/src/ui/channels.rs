use axum::{
    Form,
    extract::State,
    response::{Html, IntoResponse},
};
use bitcoin::hex::{DisplayHex, FromHex};
use bitcoin::secp256k1::PublicKey;
use ldk_node::UserChannelId;
use lightning::ln::msgs::SocketAddress;
use maud::{Markup, html};
use serde::Deserialize;
use std::str::FromStr;

use super::shared::{base_template, copyable_hex_input, inline_error};
use crate::AppState;

pub fn channels_template(channels: &[ldk_node::ChannelDetails]) -> Markup {
    let content = html! {
        div class="row g-4" {
                @for channel in channels {
                    div class="col-12" {
                        div class="card h-100 overflow-hidden" {
                            div class="card-header" {
                                h5 class="card-title mb-0" { "Channel" }
                            }
                            div class="card-body" {
                                table class="table table-sm table-borderless mb-0" {
                                    tbody {
                                        tr {
                                            td class="fw-bold" style="width: 1px; white-space: nowrap;" { "user_channel_id:" }
                                            td style="width: 100%; min-width: 0;" {
                                                (copyable_hex_input(&channel.user_channel_id.0.to_be_bytes().as_hex().to_string(), None))
                                            }
                                        }
                                        tr {
                                            td class="fw-bold" style="width: 1px; white-space: nowrap;" { "counterparty_node_id:" }
                                            td style="width: 100%; min-width: 0;" {
                                                (copyable_hex_input(&channel.counterparty_node_id.to_string(), None))
                                            }
                                        }
                                        tr {
                                            td class="fw-bold" style="width: 1px; white-space: nowrap;" { "channel_value_sats:" }
                                            td style="width: 100%; min-width: 0;" {
                                                (channel.channel_value_sats)
                                            }
                                        }
                                        tr {
                                            td class="fw-bold" style="width: 1px; white-space: nowrap;" { "is_outbound:" }
                                            td style="width: 100%; min-width: 0;" {
                                                (channel.is_outbound)
                                            }
                                        }
                                        tr {
                                            td class="fw-bold" style="width: 1px; white-space: nowrap;" { "outbound_capacity_msat:" }
                                            td style="width: 100%; min-width: 0;" {
                                                (channel.outbound_capacity_msat)
                                            }
                                        }
                                        tr {
                                            td class="fw-bold" style="width: 1px; white-space: nowrap;" { "inbound_capacity_msat:" }
                                            td style="width: 100%; min-width: 0;" {
                                                (channel.inbound_capacity_msat)
                                            }
                                        }
                                        tr {
                                            td class="fw-bold" style="width: 1px; white-space: nowrap;" { "is_channel_ready:" }
                                            td style="width: 100%; min-width: 0;" {
                                                (channel.is_channel_ready)
                                            }
                                        }
                                        tr {
                                            td class="fw-bold" style="width: 1px; white-space: nowrap;" { "is_usable:" }
                                            td style="width: 100%; min-width: 0;" {
                                                (channel.is_usable)
                                            }
                                        }
                                        @if let Some(confirmations) = channel.confirmations {
                                            tr {
                                                td class="fw-bold" style="width: 1px; white-space: nowrap;" { "confirmations:" }
                                                td style="width: 100%; min-width: 0;" {
                                                    (confirmations)
                                                }
                                            }
                                        }
                                        @if let Some(confirmations_required) = channel.confirmations_required {
                                            tr {
                                                td class="fw-bold" style="width: 1px; white-space: nowrap;" { "confirmations_required:" }
                                                td style="width: 100%; min-width: 0;" {
                                                    (confirmations_required)
                                                }
                                            }
                                        }
                                        @if let Some(funding_txo) = &channel.funding_txo {
                                            tr {
                                                td class="fw-bold" style="width: 1px; white-space: nowrap;" { "funding_txid:" }
                                                td style="width: 100%; min-width: 0;" {
                                                    (copyable_hex_input(&funding_txo.txid.to_string(), None))
                                                }
                                            }
                                            tr {
                                                td class="fw-bold" style="width: 1px; white-space: nowrap;" { "funding_vout:" }
                                                td style="width: 100%; min-width: 0;" {
                                                    (funding_txo.vout)
                                                }
                                            }
                                        }
                                    }
                                }
                                form hx-post="/channels/close"
                                     hx-target="this"
                                     hx-swap="outerHTML"
                                     class="mt-3" {
                                    input type="hidden" name="user_channel_id" value=(channel.user_channel_id.0.to_be_bytes().as_hex().to_string()) {}
                                    input type="hidden" name="counterparty_node_id" value=(channel.counterparty_node_id.to_string()) {}
                                    button type="submit" class="btn btn-outline-danger w-100" {
                                        "Close Channel"
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
                h5 class="card-title mb-0" { "Open Channel" }
            }
            div class="card-body" {
                form hx-post="/channels/open"
                     hx-target="#open-channel-results"
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
                        label for="channel_amount_sats" class="form-label" { "Channel Amount (sats)" }
                        input type="number" class="form-control" id="channel_amount_sats" name="channel_amount_sats" required placeholder="1000000" {}
                    }
                    button type="submit" class="btn btn-outline-primary w-100" { "Open" }
                }

                div id="open-channel-results" {}
            }
        }
    };

    base_template("Channels", "/channels", content, action_sidebar)
}

#[derive(Deserialize)]
pub struct OpenChannelForm {
    pub node_id: String,
    pub socket_address: String,
    pub channel_amount_sats: u64,
}

#[derive(Deserialize)]
pub struct CloseChannelForm {
    pub user_channel_id: String,
    pub counterparty_node_id: String,
}

pub async fn channels_page(State(state): State<AppState>) -> impl IntoResponse {
    Html(channels_template(&state.node.list_channels()).into_string())
}

pub async fn open_channel_submit(
    State(state): State<AppState>,
    Form(form): Form<OpenChannelForm>,
) -> impl IntoResponse {
    let node_id = match form.node_id.parse::<PublicKey>() {
        Ok(id) => id,
        Err(_) => return Html(inline_error("Invalid node ID format").into_string()),
    };

    let socket_address = match SocketAddress::from_str(&form.socket_address) {
        Ok(addr) => addr,
        Err(_) => return Html(inline_error("Invalid socket address format").into_string()),
    };

    match state.node.open_announced_channel(
        node_id,
        socket_address,
        form.channel_amount_sats,
        None,
        None,
    ) {
        Ok(_) => Html("".to_string()),
        Err(e) => Html(inline_error(&format!("Failed to open channel: {}", e)).into_string()),
    }
}

pub async fn close_channel_submit(
    State(state): State<AppState>,
    Form(form): Form<CloseChannelForm>,
) -> impl IntoResponse {
    let counterparty_node_id = match form.counterparty_node_id.parse::<PublicKey>() {
        Ok(id) => id,
        Err(_) => return Html(inline_error("Invalid node ID format").into_string()),
    };

    let user_channel_id = match <[u8; 16]>::from_hex(&form.user_channel_id)
        .map(u128::from_be_bytes)
        .map(UserChannelId)
    {
        Ok(id) => id,
        Err(_) => return Html(inline_error("Invalid channel ID format").into_string()),
    };

    match state
        .node
        .close_channel(&user_channel_id, counterparty_node_id)
    {
        Ok(_) => Html(
            html! {
                form class="mt-3" {
                    button class="btn btn-outline-danger w-100" disabled {
                        "This channel has been closed"
                    }
                }
            }
            .into_string(),
        ),
        Err(e) => Html(inline_error(&format!("Failed to close channel: {}", e)).into_string()),
    }
}
