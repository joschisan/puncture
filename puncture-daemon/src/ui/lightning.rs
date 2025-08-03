use axum::{
    Form,
    extract::State,
    response::{Html, IntoResponse},
};
use bitcoin::hex::{DisplayHex, FromHex};
use ldk_node::UserChannelId;
use maud::{Markup, html};
use serde::Deserialize;

use super::shared::{
    base_template, copyable_hex_input, format_sats, parse_node_id, parse_socket_address,
    qr_code_with_copy, success_message, success_replacement,
};
use crate::AppState;

pub fn lightning_template(
    node_id: &str,
    total_inbound_capacity_msat: u64,
    total_outbound_capacity_msat: u64,
    channels: &[ldk_node::ChannelDetails],
    peers: &[ldk_node::PeerDetails],
) -> Markup {
    let content = html! {
        // Overview Cards
        div class="row g-4 mb-4" {
            div class="col-6" {
                div class="card h-100 overflow-hidden" {
                    div class="card-body" {
                        h5 class="card-title" { "Inbound Capacity" }
                        p class="card-text display-6" { (format_sats(total_inbound_capacity_msat / 1000)) " ₿" }
                    }
                }
            }

            div class="col-6" {
                div class="card h-100 overflow-hidden" {
                    div class="card-body" {
                        h5 class="card-title" { "Outbound Capacity" }
                        p class="card-text display-6" { (format_sats(total_outbound_capacity_msat / 1000)) " ₿" }
                    }
                }
            }
        }

        // Lightning Channels
        div class="card h-100 overflow-hidden mb-4" {
            div class="card-body" {
                h5 class="card-title" { "Lightning Channels" }
                @if channels.is_empty() {
                    p class="text-muted" { "No channels open yet. Use the sidebar to open your first channel." }
                } @else {
                    div class="accordion" id="channelsAccordion" {
                        @for (i, channel) in channels.iter().enumerate() {
                            div class="accordion-item" {
                                h2 class="accordion-header" {
                                    button class="accordion-button collapsed" type="button" data-bs-toggle="collapse"
                                            data-bs-target={(format!("#channel-{}", i))} aria-expanded="false"
                                            aria-controls={(format!("channel-{}", i))} {
                                        div class="d-flex align-items-center w-100 me-3" {
                                            div class="me-3" { (format_sats(channel.outbound_capacity_msat / 1000)) " ₿" }
                                                @let total_capacity = channel.outbound_capacity_msat + channel.inbound_capacity_msat;
                                                @let local_percentage = (100 * channel.outbound_capacity_msat) / total_capacity;
                                                div class="progress flex-grow-1 me-3" style="height: 8px;" {
                                                    div class="progress-bar bg-primary" role="progressbar"
                                                         style={(format!("width: {}%", local_percentage))}
                                                         aria-valuenow=(local_percentage) aria-valuemin="0" aria-valuemax="100" {}
                                                }
                                            div { (format_sats(channel.inbound_capacity_msat / 1000)) " ₿" }
                                        }
                                    }
                                }
                                div id={(format!("channel-{}", i))} class="accordion-collapse collapse" data-bs-parent="#channelsAccordion" {
                                    div class="accordion-body" {
                                        table class="table table-sm table-borderless mb-3" {
                                            tbody {
                                                tr {
                                                    td class="fw-bold" style="width: 1px; white-space: nowrap;" { "User Channel ID" }
                                                    td style="width: 100%; min-width: 0;" {
                                                        (copyable_hex_input(&channel.user_channel_id.0.to_be_bytes().as_hex().to_string(), None))
                                                    }
                                                }
                                                tr {
                                                    td class="fw-bold" style="width: 1px; white-space: nowrap;" { "Counterparty Node ID" }
                                                    td style="width: 100%; min-width: 0;" {
                                                        (copyable_hex_input(&channel.counterparty_node_id.to_string(), None))
                                                    }
                                                }
                                                tr {
                                                    td class="fw-bold" { "Channel Value" }
                                                    td { (format_sats(channel.channel_value_sats)) " ₿" }
                                                }
                                                tr {
                                                    td class="fw-bold" { "Outbound Capacity" }
                                                    td { (format_sats(channel.outbound_capacity_msat / 1000)) " ₿" }
                                                }
                                                tr {
                                                    td class="fw-bold" { "Inbound Capacity" }
                                                    td { (format_sats(channel.inbound_capacity_msat / 1000)) " ₿" }
                                                }
                                                tr {
                                                    td class="fw-bold" { "Channel Ready" }
                                                    td { (channel.is_channel_ready) }
                                                }
                                                tr {
                                                    td class="fw-bold" { "Usable" }
                                                    td { (channel.is_usable) }
                                                }
                                            }
                                        }
                                        div class="d-flex justify-content-end" {
                                            div class="accordion" id={(format!("closeAccordion-{}", i))} style="width: 300px;" {
                                                div class="accordion-item" {
                                                    h2 class="accordion-header" {
                                                        button class="accordion-button collapsed btn-outline-warning" type="button"
                                                               data-bs-toggle="collapse"
                                                               data-bs-target={(format!("#closeCollapse-{}", i))}
                                                               aria-expanded="false"
                                                               aria-controls={(format!("closeCollapse-{}", i))} {
                                                            "Close Channel"
                                                        }
                                                    }
                                                    div id={(format!("closeCollapse-{}", i))} class="accordion-collapse collapse"
                                                         data-bs-parent={(format!("#closeAccordion-{}", i))} {
                                                        div class="accordion-body" {
                                                            (close_channel_form(
                                                                &channel.user_channel_id.0.to_be_bytes().as_hex().to_string(),
                                                                &channel.counterparty_node_id.to_string(),
                                                                i,
                                                                None
                                                            ))
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Connected Peers
        div class="card h-100 overflow-hidden" {
            div class="card-body" {
                h5 class="card-title" { "Connected Peers" }
                @if peers.is_empty() {
                    p class="text-muted" { "No peers connected. Use the sidebar to connect to your first peer." }
                } @else {
                    div class="accordion" id="peersAccordion" {
                        @for (i, peer) in peers.iter().enumerate() {
                            div class="accordion-item" {
                                h2 class="accordion-header" {
                                    button class="accordion-button collapsed" type="button" data-bs-toggle="collapse"
                                            data-bs-target={(format!("#peer-{}", i))} aria-expanded="false"
                                            aria-controls={(format!("peer-{}", i))} {
                                        div class="d-flex align-items-center w-100 me-3" {
                                            div class="me-3 font-monospace small" {
                                                (peer.node_id.to_string()[..16]) "..."
                                            }
                                            @if peer.is_connected {
                                                span class="badge bg-success ms-auto" { "Connected" }
                                            } @else {
                                                span class="badge bg-danger ms-auto" { "Disconnected" }
                                            }
                                        }
                                    }
                                }
                                div id={(format!("peer-{}", i))} class="accordion-collapse collapse" data-bs-parent="#peersAccordion" {
                                    div class="accordion-body" {
                                        table class="table table-sm table-borderless mb-3" {
                                            tbody {
                                                tr {
                                                    td class="fw-bold" style="width: 1px; white-space: nowrap;" { "Node ID" }
                                                    td style="width: 100%; min-width: 0;" {
                                                        (copyable_hex_input(&peer.node_id.to_string(), None))
                                                    }
                                                }
                                                tr {
                                                    td class="fw-bold" { "Address" }
                                                    td class="font-monospace small" { (peer.address.to_string()) }
                                                }
                                                tr {
                                                    td class="fw-bold" { "Connected" }
                                                    td { (peer.is_connected) }
                                                }
                                                tr {
                                                    td class="fw-bold" { "Persisted" }
                                                    td { (peer.is_persisted) }
                                                }
                                            }
                                        }
                                        div class="d-flex justify-content-end" {
                                            div class="accordion" id={(format!("disconnectAccordion-{}", i))} style="width: 300px;" {
                                                div class="accordion-item" {
                                                    h2 class="accordion-header" {
                                                        button class="accordion-button collapsed btn-outline-danger" type="button"
                                                               data-bs-toggle="collapse"
                                                               data-bs-target={(format!("#disconnectCollapse-{}", i))}
                                                               aria-expanded="false"
                                                               aria-controls={(format!("disconnectCollapse-{}", i))} {
                                                            "Disconnect Peer"
                                                        }
                                                    }
                                                    div id={(format!("disconnectCollapse-{}", i))} class="accordion-collapse collapse"
                                                         data-bs-parent={(format!("#disconnectAccordion-{}", i))} {
                                                        div class="accordion-body" {
                                                            (disconnect_peer_form(&peer.node_id.to_string(), None))
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    };

    let action_sidebar = html! {
        // Node ID at top of sidebar
        div class="mb-3" {
            h6 class="text-muted mb-2" { "Node ID" }
            (copyable_hex_input(node_id, None))
        }

        div class="accordion" id="lightningActionsAccordion" {
            div class="accordion-item" {
                h2 class="accordion-header" {
                    button class="accordion-button collapsed" type="button" data-bs-toggle="collapse" data-bs-target="#openChannelCollapse" aria-expanded="false" aria-controls="openChannelCollapse" {
                        "Open Channel"
                    }
                }
                div id="openChannelCollapse" class="accordion-collapse collapse" data-bs-parent="#lightningActionsAccordion" {
                    div class="accordion-body" {
                        (open_channel_form(None))
                    }
                }
            }

            div class="accordion-item" {
                h2 class="accordion-header" {
                    button class="accordion-button collapsed" type="button" data-bs-toggle="collapse" data-bs-target="#requestChannelCollapse" aria-expanded="false" aria-controls="requestChannelCollapse" {
                        "Request Channel"
                    }
                }
                div id="requestChannelCollapse" class="accordion-collapse collapse" data-bs-parent="#lightningActionsAccordion" {
                    div class="accordion-body" {
                        (request_channel_form(None))
                    }
                }
            }

            div class="accordion-item" {
                h2 class="accordion-header" {
                    button class="accordion-button collapsed" type="button" data-bs-toggle="collapse" data-bs-target="#connectPeerCollapse" aria-expanded="false" aria-controls="connectPeerCollapse" {
                        "Connect Peer"
                    }
                }
                div id="connectPeerCollapse" class="accordion-collapse collapse" data-bs-parent="#lightningActionsAccordion" {
                    div class="accordion-body" {
                        (connect_peer_form(None))
                    }
                }
            }
        }
    };

    base_template("Lightning", "/lightning", content, action_sidebar)
}

// Form structs (reusing existing ones)
#[derive(Deserialize)]
pub struct OpenChannelForm {
    pub node_id: String,
    pub socket_address: String,
    pub channel_amount_sats: u64,
    #[serde(default)]
    pub public: bool,
}

#[derive(Deserialize)]
pub struct RequestChannelForm {
    pub lsp_balance_sat: u64,
    #[serde(default)]
    pub public: bool,
}

#[derive(Deserialize)]
pub struct CloseChannelForm {
    pub user_channel_id: String,
    pub counterparty_node_id: String,
    #[serde(default)]
    pub force: bool,
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

// Page handler
pub async fn lightning_page(State(state): State<AppState>) -> impl IntoResponse {
    let channels = state.node.list_channels();
    let peers = state.node.list_peers();

    let total_inbound_capacity_msat = channels
        .iter()
        .filter(|c| c.is_usable)
        .map(|c| c.inbound_capacity_msat)
        .sum();

    let total_outbound_capacity_msat = channels
        .iter()
        .filter(|c| c.is_usable)
        .map(|c| c.outbound_capacity_msat)
        .sum();

    Html(
        lightning_template(
            &state.node.node_id().to_string(),
            total_inbound_capacity_msat,
            total_outbound_capacity_msat,
            &channels,
            &peers,
        )
        .into_string(),
    )
}

fn open_channel_form(error: Option<&str>) -> Markup {
    html! {
        form hx-post="/lightning/channel/open"
             hx-target="this"
             hx-swap="outerHTML" {

            @if let Some(err) = error {
                div class="alert alert-danger" { (err) }
            }

            div class="mb-3" {
                label for="open-node-id" class="form-label" { "Node ID" }
                input type="text" class="form-control font-monospace" id="open-node-id" name="node_id" required placeholder="03..." {}
            }
            div class="mb-3" {
                label for="open-address" class="form-label" { "Address" }
                input type="text" class="form-control" id="open-address" name="socket_address" required placeholder="host:port" {}
            }
            div class="mb-3" {
                label for="open-amount" class="form-label" { "Amount (sats)" }
                input type="number" class="form-control" id="open-amount" name="channel_amount_sats" required placeholder="1000000" {}
            }
            div class="mb-3" {
                div class="form-check" {
                    input class="form-check-input" type="checkbox" id="open-public" name="public" value="true" {}
                    label class="form-check-label" for="open-public" { "Public Channel" }
                }
            }
            button type="submit" class="btn btn-outline-primary w-100" { "Open Channel" }
        }
    }
}

fn request_channel_form(error: Option<&str>) -> Markup {
    html! {
        form hx-post="/lightning/channel/request"
             hx-target="this"
             hx-swap="outerHTML" {

            @if let Some(err) = error {
                div class="alert alert-danger" { (err) }
            }

            div class="mb-3" {
                label for="request-amount" class="form-label" { "Amount (sats)" }
                input type="number" class="form-control" id="request-amount" name="lsp_balance_sat" required placeholder="1000000" {}
            }
            div class="mb-3" {
                div class="form-check" {
                    input class="form-check-input" type="checkbox" id="request-public" name="public" value="true" {}
                    label class="form-check-label" for="request-public" { "Public Channel" }
                }
            }
            button type="submit" class="btn btn-outline-primary w-100" { "Request Channel" }
        }
    }
}

fn connect_peer_form(error: Option<&str>) -> Markup {
    html! {
        form hx-post="/lightning/peer/connect"
             hx-target="this"
             hx-swap="outerHTML" {

            @if let Some(err) = error {
                div class="alert alert-danger" { (err) }
            }

            div class="mb-3" {
                label for="connect-node-id" class="form-label" { "Node ID" }
                input type="text" class="form-control font-monospace" id="connect-node-id" name="node_id" required placeholder="03..." {}
            }
            div class="mb-3" {
                label for="connect-address" class="form-label" { "Address" }
                input type="text" class="form-control" id="connect-address" name="socket_address" required placeholder="host:port" {}
            }
            div class="mb-3" {
                div class="form-check" {
                    input class="form-check-input" type="checkbox" id="persist-connection" name="persist" value="true" {}
                    label class="form-check-label" for="persist-connection" { "Persist Connection" }
                }
            }
            button type="submit" class="btn btn-outline-primary w-100" { "Connect Peer" }
        }
    }
}

fn disconnect_peer_form(counterparty_node_id: &str, error: Option<&str>) -> Markup {
    html! {
        form hx-post="/lightning/peer/disconnect"
             hx-target="this"
             hx-swap="outerHTML" {

            @if let Some(err) = error {
                div class="alert alert-danger" { (err) }
            }

            input type="hidden" name="counterparty_node_id" value=(counterparty_node_id) {}
            button type="submit" class="btn btn-outline-danger w-100" {
                "Disconnect"
            }
        }
    }
}

fn close_channel_form(
    user_channel_id: &str,
    counterparty_node_id: &str,
    index: usize,
    error: Option<&str>,
) -> Markup {
    html! {
        form hx-post="/lightning/channel/close"
             hx-target="this"
             hx-swap="outerHTML" {

            @if let Some(err) = error {
                div class="alert alert-danger" { (err) }
            }

            input type="hidden" name="user_channel_id" value=(user_channel_id) {}
            input type="hidden" name="counterparty_node_id" value=(counterparty_node_id) {}

            div class="form-check mb-2" {
                input class="form-check-input" type="checkbox" name="force" value="true" id={(format!("forceClose-{}", index))} {}
                label class="form-check-label text-danger small" for={(format!("forceClose-{}", index))} {
                    "Force close (unilateral)"
                }
            }

            button type="submit" class="btn btn-outline-danger w-100" {
                "Close"
            }
        }
    }
}

async fn try_open_channel(
    state: &AppState,
    form: &OpenChannelForm,
) -> Result<UserChannelId, String> {
    let node_id = parse_node_id(&form.node_id).map_err(|e| e.to_string())?;

    let socket_address = parse_socket_address(&form.socket_address).map_err(|e| e.to_string())?;

    let result = if form.public {
        // Public channel
        state.node.open_announced_channel(
            node_id,
            socket_address,
            form.channel_amount_sats,
            None,
            None,
        )
    } else {
        // Private channel (default)
        state.node.open_channel(
            node_id,
            socket_address,
            form.channel_amount_sats,
            None,
            None,
        )
    };

    result.map_err(|e| format!("Failed to open channel: {e}"))
}

async fn try_request_channel(
    state: &AppState,
    form: &RequestChannelForm,
) -> Result<String, String> {
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
        .map_err(|_| "Failed to connect to Megalith LSP node".to_string())
        .ok();

    let client = reqwest::Client::new();

    // Create request payload for Megalith LSPS1 API
    let payload = serde_json::json!({
        "lsp_balance_sat": form.lsp_balance_sat.to_string(),
        "client_balance_sat": "0",
        "required_channel_confirmations": 0,
        "funding_confirms_within_blocks": 6,
        "channel_expiry_blocks": 13140,
        "token": "",
        "refund_on_chain_address": null,
        "announce_channel": form.public,
        "public_key": state.node.node_id().to_string()
    });

    // Make HTTP request to Megalith LSPS1 API
    let response = client
        .post("https://megalithic.me/api/lsps1/v1/create_order")
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("Failed to call Megalith API: {e}"))?;

    if !response.status().is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(format!("Megalith API error: {error_text}"));
    }

    // Parse response to get the BOLT11 invoice
    let api_response: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse Megalith response: {e}"))?;

    let invoice = api_response
        .get("payment")
        .and_then(|v| v.get("bolt11"))
        .and_then(|v| v.get("invoice"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing invoice in Megalith response".to_string())?;

    Ok(invoice.to_string())
}

async fn try_connect_peer(state: &AppState, form: &ConnectPeerForm) -> Result<(), String> {
    let node_id = parse_node_id(&form.node_id).map_err(|e| e.to_string())?;

    let socket_address = parse_socket_address(&form.socket_address).map_err(|e| e.to_string())?;

    state
        .node
        .connect(node_id, socket_address, form.persist)
        .map_err(|e| format!("Failed to connect to peer: {e}"))?;

    Ok(())
}

async fn try_disconnect_peer(state: &AppState, form: &DisconnectPeerForm) -> Result<(), String> {
    let node_id = parse_node_id(&form.counterparty_node_id).map_err(|e| e.to_string())?;

    state
        .node
        .disconnect(node_id)
        .map_err(|e| format!("Failed to disconnect from peer: {e}"))?;

    Ok(())
}

async fn try_close_channel(state: &AppState, form: &CloseChannelForm) -> Result<(), String> {
    let node_id = parse_node_id(&form.counterparty_node_id).map_err(|e| e.to_string())?;

    let user_channel_id = <[u8; 16]>::from_hex(&form.user_channel_id)
        .map(u128::from_be_bytes)
        .map(ldk_node::UserChannelId)
        .map_err(|_| "Invalid channel ID format".to_string())?;

    let result = if form.force {
        state
            .node
            .force_close_channel(&user_channel_id, node_id, None)
    } else {
        state.node.close_channel(&user_channel_id, node_id)
    };

    result.map_err(|e| format!("Failed to close channel: {e}"))?;

    Ok(())
}

pub async fn open_channel_submit(
    State(state): State<AppState>,
    Form(form): Form<OpenChannelForm>,
) -> Html<String> {
    match try_open_channel(&state, &form).await {
        Ok(_) => Html(success_message("Channel opened!").into_string()),
        Err(error) => Html(open_channel_form(Some(&error)).into_string()),
    }
}

pub async fn request_channel_submit(
    State(state): State<AppState>,
    Form(form): Form<RequestChannelForm>,
) -> Html<String> {
    match try_request_channel(&state, &form).await {
        Ok(invoice) => {
            let html = success_replacement(
                "Channel Requested",
                "Pay this Lightning invoice to open the channel:",
                qr_code_with_copy(&invoice),
            );
            Html(html.into_string())
        }
        Err(error) => Html(request_channel_form(Some(&error)).into_string()),
    }
}

pub async fn close_channel_submit(
    State(state): State<AppState>,
    Form(form): Form<CloseChannelForm>,
) -> Html<String> {
    match try_close_channel(&state, &form).await {
        Ok(_) => Html(success_message("Channel closed!").into_string()),
        Err(error) => Html(
            close_channel_form(
                &form.user_channel_id,
                &form.counterparty_node_id,
                0,
                Some(&error),
            )
            .into_string(),
        ),
    }
}

pub async fn connect_peer_submit(
    State(state): State<AppState>,
    Form(form): Form<ConnectPeerForm>,
) -> Html<String> {
    match try_connect_peer(&state, &form).await {
        Ok(_) => Html(success_message("Peer connected!").into_string()),
        Err(error) => Html(connect_peer_form(Some(&error)).into_string()),
    }
}

pub async fn disconnect_peer_submit(
    State(state): State<AppState>,
    Form(form): Form<DisconnectPeerForm>,
) -> Html<String> {
    match try_disconnect_peer(&state, &form).await {
        Ok(_) => Html(success_message("Peer disconnected!").into_string()),
        Err(error) => {
            Html(disconnect_peer_form(&form.counterparty_node_id, Some(&error)).into_string())
        }
    }
}
