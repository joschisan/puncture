use axum::{
    Form,
    extract::State,
    response::{Html, IntoResponse},
};
use maud::{Markup, html};
use serde::Deserialize;

use super::shared::{base_template, format_sats, qr_code_with_copy, success_replacement};
use crate::AppState;
use bitcoin::{Address, address::NetworkUnchecked};

pub fn onchain_template(onchain_balance: u64, user_count: usize) -> Markup {
    let content = html! {
        div class="row g-4" {
            div class="col-12" {
                div class="card h-100 overflow-hidden" {
                    div class="card-body" {
                        h5 class="card-title" { "Onchain Balance" }
                        p class="card-text display-6" { (format_sats(onchain_balance)) " ₿" }
                    }
                }
            }
        }
    };

    let action_sidebar = html! {
        div class="accordion" id="actionsAccordion" {
            div class="accordion-item" {
                h2 class="accordion-header" {
                    button class="accordion-button collapsed" type="button" data-bs-toggle="collapse" data-bs-target="#receiveCollapse" aria-expanded="false" aria-controls="receiveCollapse" {
                        "Receive Bitcoin"
                    }
                }
                div id="receiveCollapse" class="accordion-collapse collapse" data-bs-parent="#actionsAccordion" {
                    div class="accordion-body" {
                        (receive_bitcoin_form())
                    }
                }
            }

            div class="accordion-item" {
                h2 class="accordion-header" {
                    button class="accordion-button collapsed" type="button" data-bs-toggle="collapse" data-bs-target="#sendCollapse" aria-expanded="false" aria-controls="sendCollapse" {
                        "Send Bitcoin"
                    }
                }
                div id="sendCollapse" class="accordion-collapse collapse" data-bs-parent="#actionsAccordion" {
                    div class="accordion-body" {
                        (send_bitcoin_form(None))
                    }
                }
            }

            div class="accordion-item" {
                h2 class="accordion-header" {
                    button class="accordion-button collapsed" type="button" data-bs-toggle="collapse" data-bs-target="#drainCollapse" aria-expanded="false" aria-controls="drainCollapse" {
                        "Drain Wallet"
                    }
                }
                div id="drainCollapse" class="accordion-collapse collapse" data-bs-parent="#actionsAccordion" {
                    div class="accordion-body" {
                        (drain_wallet_form(None))
                    }
                }
            }
        }
    };

    base_template("Onchain", "/onchain", content, action_sidebar, user_count)
}

fn receive_bitcoin_form() -> Markup {
    html! {
        form hx-post="/onchain/receive"
             hx-target="this"
             hx-swap="outerHTML" {
            button type="submit" class="btn btn-outline-primary w-100" { "Generate Address" }
        }
    }
}

fn send_bitcoin_form(error: Option<&str>) -> Markup {
    html! {
        form hx-post="/onchain/send"
             hx-target="this"
             hx-swap="outerHTML" {

            @if let Some(err) = error {
                div class="alert alert-danger" { (err) }
            }

            div class="mb-3" {
                label for="accordion-address" class="form-label" { "Bitcoin Address" }
                input type="text" class="form-control font-monospace" id="accordion-address" name="address" required placeholder="bc1qxxx..." {}
                div class="form-text" { "Enter the destination Bitcoin address" }
            }
            div class="mb-3" {
                label for="accordion-amount" class="form-label" { "Amount (sats)" }
                input type="number" class="form-control" id="accordion-amount" name="amount_sats" required placeholder="100000" min="1" {}
                div class="form-text" { "Amount to send in satoshis" }
            }

            button type="submit" class="btn btn-outline-primary w-100" { "Send Bitcoin" }
        }
    }
}

fn drain_wallet_form(error: Option<&str>) -> Markup {
    html! {
        form hx-post="/onchain/drain"
             hx-target="this"
             hx-swap="outerHTML" {

            @if let Some(err) = error {
                div class="alert alert-danger" { (err) }
            }

            div class="mb-3" {
                label for="accordion-drain-address" class="form-label" { "Destination Address" }
                input type="text" class="form-control font-monospace" id="accordion-drain-address" name="address" required placeholder="bc1q..." {}
            }

            button type="submit" class="btn btn-outline-primary w-100" { "Drain Wallet" }
        }
    }
}

#[derive(Deserialize)]
pub struct OnchainSendForm {
    pub address: String,
    pub amount_sats: u64,
}

#[derive(Deserialize)]
pub struct OnchainDrainForm {
    pub address: String,
}

pub async fn onchain_page(State(state): State<AppState>) -> impl IntoResponse {
    let user_count = super::db::list_users(&state.db).await.len();

    let html = onchain_template(
        state.node.list_balances().total_onchain_balance_sats,
        user_count,
    );

    Html(html.into_string())
}

async fn try_send_bitcoin(state: &AppState, form: &OnchainSendForm) -> Result<String, String> {
    let unchecked_address = form
        .address
        .parse::<Address<NetworkUnchecked>>()
        .map_err(|_| "Invalid address format".to_string())?;

    let address = unchecked_address
        .require_network(state.node.config().network)
        .map_err(|_| "Invalid address for network".to_string())?;

    let txid = state
        .node
        .onchain_payment()
        .send_to_address(&address, form.amount_sats, None)
        .map_err(|e| format!("Failed to send: {e}"))?;

    Ok(txid.to_string())
}

async fn try_drain_wallet(state: &AppState, form: &OnchainDrainForm) -> Result<String, String> {
    let unchecked_address = form
        .address
        .parse::<Address<NetworkUnchecked>>()
        .map_err(|_| "Invalid address format".to_string())?;

    let address = unchecked_address
        .require_network(state.node.config().network)
        .map_err(|_| "Invalid address for network".to_string())?;

    if !state.node.list_channels().is_empty() {
        return Err(
            "Cannot drain wallet while channels are open. Close all channels first.".to_string(),
        );
    }

    let txid = state
        .node
        .onchain_payment()
        .send_all_to_address(&address, false, None)
        .map_err(|e| format!("Failed to drain: {e}"))?;

    Ok(txid.to_string())
}

pub async fn onchain_receive_submit(State(state): State<AppState>) -> Html<String> {
    let address = state
        .node
        .onchain_payment()
        .new_address()
        .expect("Failed to generate new address");

    let html = success_replacement(
        "Address Generated",
        "Send Bitcoin to this address:",
        qr_code_with_copy(&address.to_string()),
    );

    Html(html.into_string())
}

pub async fn onchain_send_submit(
    State(state): State<AppState>,
    Form(form): Form<OnchainSendForm>,
) -> Html<String> {
    match try_send_bitcoin(&state, &form).await {
        Ok(txid) => {
            let html = success_replacement(
                "Transaction Created",
                "You can monitor the confirmation of the transaction on mempool.space:",
                html! {
                    a href={(format!("https://mempool.space/tx/{}", txid))} target="_blank" class="btn btn-outline-primary" {
                        "mempool.space"
                    }
                },
            );
            Html(html.into_string())
        }
        Err(error) => Html(send_bitcoin_form(Some(&error)).into_string()),
    }
}

pub async fn onchain_drain_submit(
    State(state): State<AppState>,
    Form(form): Form<OnchainDrainForm>,
) -> Html<String> {
    match try_drain_wallet(&state, &form).await {
        Ok(txid) => {
            let html = success_replacement(
                "Transaction Created",
                "You can monitor the confirmation of the transaction on mempool.space:",
                html! {
                    a href={(format!("https://mempool.space/tx/{}", txid))} target="_blank" class="btn btn-outline-primary" {
                        "mempool.space"
                    }
                },
            );
            Html(html.into_string())
        }
        Err(error) => Html(drain_wallet_form(Some(&error)).into_string()),
    }
}
