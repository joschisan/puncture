use bitcoin::hashes::Hash;
use bitcoin::hex::DisplayHex;
use diesel::{Connection, ExpressionMethods, JoinOnDsl, OptionalExtension, QueryDsl, RunQueryDsl};
use lightning::offers::offer::Offer;
use lightning_invoice::Bolt11Invoice;
use rand::Rng;
use tracing::info;

use puncture_core::unix_time;
use puncture_daemon_db::models::{
    InviteRecord, InvoiceRecord, OfferRecord, ReceiveRecord, RecoveryRecord, SendRecord, User,
};
use puncture_daemon_db::schema::{invite, invoice, offer, receive, recovery, send, user};

use crate::convert::IntoPayment;

pub async fn user_exists(conn: &mut diesel::SqliteConnection, user_pk: String) -> bool {
    diesel::select(diesel::dsl::exists(
        user::table.filter(user::user_pk.eq(user_pk)),
    ))
    .get_result::<bool>(conn)
    .expect("Failed to check if user exists")
}

pub async fn get_invite(
    conn: &mut diesel::SqliteConnection,
    invite_id: &str,
) -> Option<InviteRecord> {
    let invite_id = invite_id.to_string();

    invite::table
        .filter(invite::id.eq(invite_id))
        .first::<InviteRecord>(conn)
        .optional()
        .expect("Failed to query invite")
}

pub async fn count_invite_users(conn: &mut diesel::SqliteConnection, invite_id: &str) -> i64 {
    let invite_id = invite_id.to_string();

    user::table
        .filter(user::invite_id.eq(invite_id))
        .count()
        .first::<i64>(conn)
        .expect("Failed to count invite users")
}

pub async fn register_user_with_invite(
    conn: &mut diesel::SqliteConnection,
    user_pk: String,
    invite_id: String,
) {
    diesel::insert_into(user::table)
        .values(&User {
            user_pk,
            invite_id,
            created_at: unix_time(),
            recovery_name: None,
        })
        .on_conflict(user::user_pk)
        .do_nothing()
        .execute(conn)
        .expect("Failed to register user with invite");
}

pub async fn get_recovery(
    conn: &mut diesel::SqliteConnection,
    recovery_id: &str,
) -> Option<RecoveryRecord> {
    let recovery_id = recovery_id.to_string();

    recovery::table
        .filter(recovery::id.eq(recovery_id))
        .first::<RecoveryRecord>(conn)
        .optional()
        .expect("Failed to query recovery")
}

pub async fn create_invoice(
    conn: &mut diesel::SqliteConnection,
    user_pk: String,
    invoice: Bolt11Invoice,
    amount_msat: i64,
    description: String,
    expiry_secs: u32,
) {
    let new_invoice = InvoiceRecord {
        id: invoice.payment_hash().as_byte_array().as_hex().to_string(),
        user_pk,
        amount_msat: Some(amount_msat),
        description,
        pr: invoice.to_string(),
        expires_at: unix_time() + expiry_secs as i64 * 1000,
        created_at: unix_time(),
    };

    info!(?new_invoice, "Creating invoice");

    diesel::insert_into(invoice::table)
        .values(&new_invoice)
        .execute(conn)
        .expect("Failed to create invoice");
}

pub async fn count_pending_invoices(conn: &mut diesel::SqliteConnection, user_pk: String) -> i64 {
    invoice::table
        .filter(invoice::user_pk.eq(user_pk))
        .filter(invoice::expires_at.gt(unix_time()))
        .left_join(receive::table.on(invoice::id.eq(receive::id)))
        .filter(receive::id.is_null())
        .count()
        .first::<i64>(conn)
        .expect("Failed to count pending invoices")
}

pub async fn create_offer(
    conn: &mut diesel::SqliteConnection,
    user_pk: String,
    offer: Offer,
    amount_msat: Option<i64>,
    description: String,
    expiry_secs: Option<u32>,
) {
    let new_offer = OfferRecord {
        id: offer.id().0.as_hex().to_string(),
        user_pk,
        amount_msat,
        description,
        pr: offer.to_string(),
        expires_at: expiry_secs.map(|secs| unix_time() + secs as i64 * 1000),
        created_at: unix_time(),
    };

    info!(?new_offer, "Creating offer");

    diesel::insert_into(offer::table)
        .values(&new_offer)
        .execute(conn)
        .expect("Failed to create offer");
}

pub async fn get_offer_by_user_pk(
    conn: &mut diesel::SqliteConnection,
    user_pk: String,
) -> Option<OfferRecord> {
    offer::table
        .filter(offer::user_pk.eq(user_pk))
        .order_by(offer::created_at.desc())
        .first::<OfferRecord>(conn)
        .optional()
        .expect("Failed to query offer")
}

pub async fn count_pending_sends(conn: &mut diesel::SqliteConnection, user_pk: String) -> i64 {
    send::table
        .filter(send::user_pk.eq(user_pk))
        .filter(send::status.eq("pending"))
        .count()
        .first::<i64>(conn)
        .expect("Failed to count pending invoices")
}

pub async fn create_internal_transfer(
    conn: &mut diesel::SqliteConnection,
    send_user_pk: String,
    receive_user_pk: String,
    amount_msat: i64,
    fee_msat: i64,
    pr: String,
    description: String,
) -> (SendRecord, ReceiveRecord) {
    let transfer_id = rand::rng().random::<[u8; 32]>().as_hex().to_string();

    info!(
        ?transfer_id,
        ?send_user_pk,
        ?receive_user_pk,
        ?amount_msat,
        "Creating internal transfer"
    );

    let send_record = SendRecord {
        id: transfer_id.clone(),
        user_pk: send_user_pk,
        amount_msat,
        fee_msat,
        description: description.clone(),
        pr: pr.clone(),
        status: "successful".to_string(),
        ln_address: None,
        created_at: unix_time(),
    };

    let receive_record = ReceiveRecord {
        id: transfer_id,
        user_pk: receive_user_pk,
        amount_msat,
        description,
        pr,
        created_at: unix_time(),
    };

    conn.transaction(|conn| {
        diesel::insert_into(send::table)
            .values(&send_record)
            .execute(conn)?;

        diesel::insert_into(receive::table)
            .values(&receive_record)
            .execute(conn)?;

        Ok::<(), diesel::result::Error>(())
    })
    .expect("Failed to create internal transfer");

    (send_record, receive_record)
}

#[allow(clippy::too_many_arguments)]
pub async fn create_send_payment(
    conn: &mut diesel::SqliteConnection,
    id: [u8; 32],
    user_pk: String,
    amount_msat: i64,
    fee_msat: i64,
    description: String,
    pr: String,
    status: String,
    ln_address: Option<String>,
) -> SendRecord {
    let new_send = SendRecord {
        id: id.as_hex().to_string(),
        user_pk,
        amount_msat,
        fee_msat,
        description,
        pr,
        status,
        ln_address,
        created_at: unix_time(),
    };

    info!(?new_send, "Creating send payment");

    diesel::insert_into(send::table)
        .values(&new_send)
        .execute(conn)
        .expect("Failed to insert send payment");

    new_send
}

pub async fn user_payments(
    conn: &mut diesel::SqliteConnection,
    user_pk: String,
) -> Vec<puncture_client_core::Payment> {
    // Load full ReceiveRecord records and convert using IntoPayment trait
    let receive_payments: Vec<puncture_client_core::Payment> = receive::table
        .filter(receive::user_pk.eq(user_pk.clone()))
        .load::<ReceiveRecord>(conn)
        .unwrap_or_default()
        .into_iter()
        .map(|record| record.into_payment(false))
        .collect();

    // Load full SendRecord records and convert using IntoPayment trait
    let send_payments: Vec<puncture_client_core::Payment> = send::table
        .filter(send::user_pk.eq(user_pk))
        .load::<SendRecord>(conn)
        .unwrap_or_default()
        .into_iter()
        .map(|record| record.into_payment(false))
        .collect();

    let mut all_payments = [receive_payments, send_payments].concat();

    all_payments.sort_by_key(|payment| payment.created_at);

    all_payments
}

pub async fn set_recovery_name(
    conn: &mut diesel::SqliteConnection,
    user_pk: String,
    recovery_name: Option<String>,
) {
    diesel::update(user::table.filter(user::user_pk.eq(user_pk)))
        .set(user::recovery_name.eq(recovery_name))
        .execute(conn)
        .expect("Failed to update recovery name");
}
