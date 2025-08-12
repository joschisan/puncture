use std::str::FromStr;
use std::sync::Arc;

use bitcoin::hashes::Hash;
use diesel::SqliteConnection;
use lightning::offers::offer::Offer;
use lightning_invoice::{Bolt11InvoiceDescription, Description};
use tracing::{error, info};

use puncture_client_core::{
    Bolt11ReceiveRequest, Bolt11ReceiveResponse, Bolt11SendRequest, Bolt12ReceiveResponse,
    Bolt12SendRequest, RecoverRequest, RecoverResponse, RegisterRequest, RegisterResponse,
    SetRecoveryNameRequest,
};
use puncture_core::unix_time;

use super::db;
use crate::{AppState, Args, EventBus, convert::IntoPayment};

pub async fn register(
    app_state: Arc<AppState>,
    user_pk: String,
    request: RegisterRequest,
) -> Result<RegisterResponse, String> {
    let mut conn = app_state.db.get_connection().await;

    let invite = db::get_invite(&mut conn, &request.invite_id)
        .await
        .ok_or("Unknown invite code".to_string())?;

    if invite.expires_at < unix_time() {
        return Err("Invite expired".to_string());
    }

    if invite.expires_at < unix_time() {
        return Err("Invite expired".to_string());
    }

    if invite.user_limit <= db::count_invite_users(&mut conn, &request.invite_id).await {
        return Err("Invite user limit reached".to_string());
    }

    db::register_user_with_invite(&mut conn, user_pk.clone(), request.invite_id.clone()).await;

    info!(?user_pk, ?request.invite_id, "New user registered");

    Ok(RegisterResponse {
        network: app_state.args.bitcoin_network,
        name: app_state.args.daemon_name.clone(),
    })
}

pub async fn bolt11_receive(
    state: Arc<AppState>,
    user_pk: String,
    request: Bolt11ReceiveRequest,
) -> Result<Bolt11ReceiveResponse, String> {
    info!(?request, "bolt11 receive");

    let mut conn = state.db.get_connection().await;

    let pending = db::count_pending_invoices(&mut conn, user_pk.clone()).await;

    if pending >= state.args.max_pending_payments_per_user as i64 {
        return Err("Too many pending invoices".to_string());
    }

    check_amount_bounds(&state.args, request.amount_msat as u64)?;

    let invoice = state
        .node
        .bolt11_payment()
        .receive(
            request.amount_msat.into(),
            &Description::new(request.description.clone())
                .map(Bolt11InvoiceDescription::Direct)
                .map_err(|e| e.to_string())?,
            state.args.invoice_expiry_secs,
        )
        .inspect_err(|error| error!(?error, "ldk node bolt11 receive error"))
        .map_err(|_| "Failed to create invoice".to_string())?;

    db::create_invoice(
        &mut conn,
        user_pk,
        invoice.clone(),
        request.amount_msat.into(),
        request.description,
        state.args.invoice_expiry_secs,
    )
    .await;

    Ok(Bolt11ReceiveResponse { invoice })
}

pub async fn bolt12_receive(
    state: Arc<AppState>,
    user_pk: String,
    _request: (),
) -> Result<Bolt12ReceiveResponse, String> {
    let mut conn = state.db.get_connection().await;

    if let Some(record) = db::get_offer_by_user_pk(&mut conn, user_pk.clone()).await {
        if record.created_at > unix_time() - (24 * 60 * 60 * 1000) {
            return Ok(Bolt12ReceiveResponse { offer: record.pr });
        }
    }

    let offer = state
        .node
        .bolt12_payment()
        .receive_variable_amount("", None)
        .map_err(|_| "Failed to create offer".to_string())?;

    db::create_offer(
        &mut conn,
        user_pk.clone(),
        offer.clone(),
        None,
        String::new(),
        None,
    )
    .await;

    Ok(Bolt12ReceiveResponse {
        offer: offer.to_string(),
    })
}

#[tracing::instrument(skip(state))]
pub async fn bolt11_send(
    state: Arc<AppState>,
    user_pk: String,
    request: Bolt11SendRequest,
) -> Result<(), String> {
    let mut conn = state.db.get_connection().await;

    let fee_msat = check_send(&mut conn, user_pk.clone(), request.amount_msat, &state.args).await?;

    match crate::db::get_invoice(&mut conn, request.invoice.payment_hash().to_byte_array()).await {
        Some(invoice) => {
            if invoice.user_pk == user_pk {
                return Err("This is your own invoice".to_string());
            }

            if let Some(amount_msat) = invoice.amount_msat {
                if amount_msat as u64 > request.amount_msat {
                    return Err("Amount is lower than the invoice's minimum amount".to_string());
                }
            }

            let (send_record, receive_record) = db::create_internal_transfer(
                &mut conn,
                user_pk.clone(),
                invoice.user_pk.clone(),
                request.amount_msat as i64,
                1000,
                invoice.pr.clone(),
                invoice.description.clone(),
            )
            .await;

            push_events(
                &mut conn,
                state.event_bus.clone(),
                user_pk.clone(),
                send_record.into_payment(true),
            )
            .await;

            push_events(
                &mut conn,
                state.event_bus.clone(),
                invoice.user_pk.clone(),
                receive_record.into_payment(true),
            )
            .await;
        }
        None => {
            let payment_id = state
                .node
                .bolt11_payment()
                .send_using_amount(&request.invoice, request.amount_msat, None)
                .map_err(|e| e.to_string())?;

            let record = db::create_send_payment(
                &mut conn,
                payment_id.0,
                user_pk.clone(),
                request.amount_msat as i64,
                fee_msat as i64,
                request.invoice.description().to_string(),
                request.invoice.to_string(),
                "pending".to_string(),
                request.ln_address.clone(),
            )
            .await;

            push_events(
                &mut conn,
                state.event_bus.clone(),
                user_pk,
                record.into_payment(true),
            )
            .await;
        }
    };

    Ok(())
}

#[tracing::instrument(skip(state))]
pub async fn bolt12_send(
    state: Arc<AppState>,
    user_pk: String,
    request: Bolt12SendRequest,
) -> Result<(), String> {
    let mut conn = state.db.get_connection().await;

    let fee_msat = check_send(&mut conn, user_pk.clone(), request.amount_msat, &state.args).await?;

    let offer = Offer::from_str(&request.offer).map_err(|_| "Invalid offer".to_string())?;

    match crate::db::get_offer(&mut conn, offer.id().0).await {
        Some(offer) => {
            if offer.user_pk == user_pk {
                return Err("This is your own payment request".to_string());
            }

            if let Some(amount_msat) = offer.amount_msat {
                if amount_msat as u64 > request.amount_msat {
                    return Err("Amount is lower than the offer's minimum amount".to_string());
                }
            }

            let (send_record, receive_record) = db::create_internal_transfer(
                &mut conn,
                user_pk.clone(),
                offer.user_pk.clone(),
                request.amount_msat as i64,
                1000,
                offer.pr.clone(),
                offer.description.clone(),
            )
            .await;

            push_events(
                &mut conn,
                state.event_bus.clone(),
                user_pk.clone(),
                send_record.into_payment(true),
            )
            .await;

            push_events(
                &mut conn,
                state.event_bus.clone(),
                offer.user_pk.clone(),
                receive_record.into_payment(true),
            )
            .await;
        }
        None => {
            let payment_id = state
                .node
                .bolt12_payment()
                .send_using_amount(&offer, request.amount_msat, None, None)
                .map_err(|e| e.to_string())?;

            let send_record = db::create_send_payment(
                &mut conn,
                payment_id.0,
                user_pk.clone(),
                request.amount_msat as i64,
                fee_msat as i64,
                offer.description().unwrap().to_string(),
                offer.to_string(),
                "pending".to_string(),
                None,
            )
            .await;

            push_events(
                &mut conn,
                state.event_bus.clone(),
                user_pk,
                send_record.into_payment(true),
            )
            .await;
        }
    };

    Ok(())
}

async fn check_send(
    conn: &mut SqliteConnection,
    user_pk: String,
    amount_msat: u64,
    args: &Args,
) -> Result<u64, String> {
    let pending_payments = db::count_pending_sends(conn, user_pk.clone()).await;

    if pending_payments >= args.max_pending_payments_per_user as i64 {
        return Err("Too many pending payments".to_string());
    }

    check_amount_bounds(args, amount_msat)?;

    let balance_msat = crate::db::user_balance(conn, user_pk.clone()).await;

    if balance_msat < amount_msat {
        return Err("Insufficient balance to cover the amount".to_string());
    }

    let fee_msat = (amount_msat * args.fee_ppm) / 1_000_000 + args.base_fee_msat;

    if balance_msat < amount_msat + fee_msat {
        return Err("Insufficient balance to cover the amount and potential fee".to_string());
    }

    Ok(fee_msat)
}

fn check_amount_bounds(args: &Args, amount_msat: u64) -> Result<(), String> {
    if amount_msat < args.min_amount_sats as u64 * 1000 {
        return Err(format!(
            "The minimum amount is {} sats",
            args.min_amount_sats
        ));
    }

    if amount_msat > args.max_amount_sats as u64 * 1000 {
        return Err(format!(
            "The maximum amount is {} sats",
            args.max_amount_sats
        ));
    }

    Ok(())
}

async fn push_events(
    conn: &mut SqliteConnection,
    event_bus: EventBus,
    user_pk: String,
    payment: puncture_client_core::Payment,
) {
    let balance_msat = crate::db::user_balance(conn, user_pk.clone()).await;

    event_bus.send_balance_event(user_pk.clone(), balance_msat);

    event_bus.send_payment_event(user_pk, payment);
}

pub async fn set_recovery_name(
    state: Arc<AppState>,
    user_pk: String,
    request: SetRecoveryNameRequest,
) -> Result<(), String> {
    if let Some(recovery_name) = request.recovery_name.as_ref() {
        if recovery_name.is_empty() {
            return Err("Recovery name cannot be empty".to_string());
        }

        if recovery_name.len() > 20 {
            return Err("Recovery name must be less than 20 characters".to_string());
        }

        if !recovery_name
            .chars()
            .all(|c| c.is_ascii_alphabetic() || c.is_ascii_whitespace())
        {
            return Err("Recovery name can only contain letters and spaces".to_string());
        }
    }

    let mut conn = state.db.get_connection().await;

    db::set_recovery_name(&mut conn, user_pk, request.recovery_name).await;

    Ok(())
}

pub async fn recover(
    app_state: Arc<AppState>,
    user_pk: String,
    request: RecoverRequest,
) -> Result<RecoverResponse, String> {
    let mut conn = app_state.db.get_connection().await;

    let recovery = db::get_recovery(&mut conn, &request.recovery_id)
        .await
        .ok_or("Unknown recovery code".to_string())?;

    if recovery.expires_at < unix_time() {
        return Err("Recovery expired".to_string());
    }

    if user_pk == recovery.user_pk {
        return Err("You cannot recover the current user".to_string());
    }

    let balance_msat = crate::db::user_balance(&mut conn, recovery.user_pk.clone()).await;

    if balance_msat == 0 {
        return Err("User has no balance to recover".to_string());
    }

    let (send_record, receive_record) = db::create_internal_transfer(
        &mut conn,
        recovery.user_pk.clone(),
        user_pk.clone(),
        balance_msat as i64,
        0,
        recovery.id.clone(),
        "Recovery".to_string(),
    )
    .await;

    push_events(
        &mut conn,
        app_state.event_bus.clone(),
        recovery.user_pk.clone(),
        send_record.into_payment(true),
    )
    .await;

    push_events(
        &mut conn,
        app_state.event_bus.clone(),
        user_pk.clone(),
        receive_record.into_payment(true),
    )
    .await;

    Ok(RecoverResponse { balance_msat })
}
