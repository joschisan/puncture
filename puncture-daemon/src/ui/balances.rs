use axum::{
    Form,
    extract::State,
    response::{Html, IntoResponse},
};
use maud::{Markup, html};
use serde::Deserialize;

use super::shared::{base_template, copyable_hex_input, inline_error};
use crate::AppState;
use bitcoin::{Address, FeeRate, address::NetworkUnchecked};

pub fn balances_template(
    node_id: &str,
    onchain_balance: u64,
    total_inbound_capacity_msat: u64,
    total_outbound_capacity_msat: u64,
) -> Markup {
    let content = html! {
        div class="row g-4" {
            div class="col-12" {
                div class="card h-100 overflow-hidden" {
                    div class="card-body" {
                        h5 class="card-title" { "Node ID" }
                        (copyable_hex_input(node_id, None))
                    }
                }
            }

            div class="col-12" {
                div class="card h-100 overflow-hidden" {
                    div class="card-body" {
                        h5 class="card-title" { "Onchain Balance" }
                        p class="card-text display-6" { (onchain_balance) " sats" }
                    }
                }
            }

            div class="col-12" {
                div class="card h-100 overflow-hidden" {
                    div class="card-body" {
                        h5 class="card-title" { "Inbound Capacity" }
                        p class="card-text display-6" { (total_inbound_capacity_msat / 1000) " sats" }
                    }
                }
            }

            div class="col-12" {
                div class="card h-100 overflow-hidden" {
                    div class="card-body" {
                        h5 class="card-title" { "Outbound Capacity" }
                        p class="card-text display-6" { (total_outbound_capacity_msat / 1000) " sats" }
                    }
                }
            }
        }
    };

    let action_sidebar = html! {
        div class="card mb-4" {
            div class="card-header" {
                h5 class="card-title mb-0" { "Receive Bitcoin" }
            }
            div class="card-body" {
                form hx-post="/onchain/receive"
                     hx-target="#receive-results"
                     hx-swap="innerHTML" {
                    button type="submit" class="btn btn-outline-primary w-100" { "Generate Address" }
                }

                div id="receive-results" {}
            }
        }

        div class="card mb-4" {
            div class="card-header" {
                h5 class="card-title mb-0" { "Send Bitcoin" }
            }
            div class="card-body" {
                form hx-post="/onchain/send"
                     hx-target="#send-results"
                     hx-swap="innerHTML" {
                    div class="mb-3" {
                        label for="address" class="form-label" { "Address" }
                        input type="text" class="form-control font-monospace" id="address" name="address" required placeholder="bc1..." {}
                    }
                    div class="mb-3" {
                        label for="amount_sats" class="form-label" { "Amount (sats)" }
                        input type="text" class="form-control" id="amount_sats" name="amount_sats" required placeholder="100000" {}
                    }
                    div class="mb-3" {
                        label for="sats_per_vbyte" class="form-label" { "Fee Rate (sats/vB)" }
                        input type="text" class="form-control" id="sats_per_vbyte" name="sats_per_vbyte" placeholder="Optional" {}
                        div class="form-text" { "Leave empty for automatic fee estimation" }
                    }
                    button type="submit" class="btn btn-outline-primary w-100" { "Create Transaction" }
                }

                div id="send-results" {}
            }
        }

        div class="card" {
            div class="card-header" {
                h5 class="card-title mb-0" { "Drain Wallet" }
            }
            div class="card-body" {
                form hx-post="/onchain/drain"
                     hx-target="#drain-results"
                     hx-swap="innerHTML" {
                    div class="mb-3" {
                        label for="drain_address" class="form-label" { "Destination Address" }
                        input type="text" class="form-control font-monospace" id="drain_address" name="address" required placeholder="bc1q..." {}
                    }
                    div class="mb-3" {
                        label for="drain_sats_per_vbyte" class="form-label" { "Fee Rate (sats/vB)" }
                        input type="text" class="form-control" id="drain_sats_per_vbyte" name="sats_per_vbyte" placeholder="Optional" {}
                        div class="form-text" { "Leave empty for automatic fee estimation" }
                    }
                    button type="submit" class="btn btn-outline-primary w-100" { "Create Transaction" }
                }

                div id="drain-results" {}
            }
        }
    };

    base_template("Balances", "/", content, action_sidebar)
}

#[derive(Deserialize)]
pub struct OnchainSendForm {
    pub address: String,
    pub amount_sats: u64,
    pub sats_per_vbyte: Option<u64>,
}

#[derive(Deserialize)]
pub struct OnchainDrainForm {
    pub address: String,
    pub sats_per_vbyte: Option<u64>,
}

pub async fn balances_page(State(state): State<AppState>) -> impl IntoResponse {
    let total_inbound_capacity_msat = state
        .node
        .list_channels()
        .iter()
        .map(|c| c.inbound_capacity_msat)
        .sum();

    let total_outbound_capacity_msat = state
        .node
        .list_channels()
        .iter()
        .map(|c| c.outbound_capacity_msat)
        .sum();

    Html(
        balances_template(
            &state.node.node_id().to_string(),
            state.node.list_balances().total_onchain_balance_sats,
            total_inbound_capacity_msat,
            total_outbound_capacity_msat,
        )
        .into_string(),
    )
}

pub async fn onchain_receive_submit(State(state): State<AppState>) -> impl IntoResponse {
    let address = state
        .node
        .onchain_payment()
        .new_address()
        .expect("Failed to generate new address");

    let html = html! {
        div class="alert alert-success fade show mt-3 mb-0" {
            h6 class="mb-2" { "Address Generated" }
            (copyable_hex_input(&address.to_string(), None))
        }
    };

    Html(html.into_string())
}

pub async fn onchain_send_submit(
    State(state): State<AppState>,
    Form(form): Form<OnchainSendForm>,
) -> impl IntoResponse {
    let unchecked_address = match form.address.parse::<Address<NetworkUnchecked>>() {
        Ok(addr) => addr,
        Err(_) => return Html(inline_error("Invalid address format").into_string()),
    };

    let address = match unchecked_address.require_network(state.node.config().network) {
        Ok(addr) => addr,
        Err(_) => return Html(inline_error("Invalid address for network").into_string()),
    };

    let fee_rate = form.sats_per_vbyte.map(FeeRate::from_sat_per_vb_unchecked);

    match state
        .node
        .onchain_payment()
        .send_to_address(&address, form.amount_sats, fee_rate)
    {
        Ok(_) => Html("".to_string()), // Success - just clear the form and stay on page
        Err(e) => Html(inline_error(&format!("Failed to send: {}", e)).into_string()),
    }
}

pub async fn onchain_drain_submit(
    State(state): State<AppState>,
    Form(form): Form<OnchainDrainForm>,
) -> impl IntoResponse {
    let unchecked_address = match form.address.parse::<Address<NetworkUnchecked>>() {
        Ok(addr) => addr,
        Err(_) => return Html(inline_error("Invalid address format").into_string()),
    };

    let address = match unchecked_address.require_network(state.node.config().network) {
        Ok(addr) => addr,
        Err(_) => return Html(inline_error("Invalid address for network").into_string()),
    };

    if !state.node.list_channels().is_empty() {
        return Html(
            inline_error("Cannot drain wallet while channels are open. Close all channels first.")
                .into_string(),
        );
    }

    let fee_rate = form.sats_per_vbyte.map(FeeRate::from_sat_per_vb_unchecked);

    match state
        .node
        .onchain_payment()
        .send_all_to_address(&address, false, fee_rate)
    {
        Ok(_) => Html("".to_string()), // Success - just clear the form and stay on page
        Err(e) => Html(inline_error(&format!("Failed to drain: {}", e)).into_string()),
    }
}
