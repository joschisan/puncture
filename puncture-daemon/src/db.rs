use std::time::{SystemTime, UNIX_EPOCH};

use bitcoin::hashes::Hash;
use bitcoin::hex::DisplayHex;
use diesel::{ExpressionMethods, JoinOnDsl, OptionalExtension, QueryDsl, RunQueryDsl};
use diesel_migrations::{EmbeddedMigrations, embed_migrations};
use lightning::offers::offer::Offer;
use lightning_invoice::Bolt11Invoice;

use puncture_cli_core::UserInfo;
use puncture_core::db::DbConnection;

use crate::models::{InviteRecord, InvoiceRecord, OfferRecord, ReceiveRecord, SendRecord, User};
use crate::schema::{invite, invoice, offer, receive, send, user};

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

pub fn unix_time() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_millis() as i64
}

#[allow(clippy::too_many_arguments)]
pub async fn create_send_payment(
    db: &DbConnection,
    id: [u8; 32],
    user_pk: String,
    amount_msat: i64,
    fee_msat: i64,
    description: String,
    pr: String,
    status: String,
    ln_address: Option<String>,
) -> SendRecord {
    let mut conn = db.get().expect("Failed to get connection from pool");

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

    tokio::task::spawn_blocking(move || {
        diesel::insert_into(send::table)
            .values(&new_send)
            .execute(&mut *conn)
            .expect("Failed to insert send payment");

        new_send
    })
    .await
    .expect("Failed to join task")
}

pub async fn update_send_payment_status(
    db: &DbConnection,
    payment_hash: [u8; 32],
    status: String,
) -> SendRecord {
    let mut conn = db.get().expect("Failed to get connection from pool");

    tokio::task::spawn_blocking(move || {
        diesel::update(send::table.find(&payment_hash.as_hex().to_string()))
            .set(send::status.eq(&status))
            .execute(&mut *conn)
            .expect("Failed to update payment status");

        send::table
            .filter(send::id.eq(payment_hash.as_hex().to_string()))
            .first::<SendRecord>(&mut *conn)
            .expect("Failed to fetch updated payment")
    })
    .await
    .expect("Failed to join task")
}

pub async fn create_invoice(
    db: &DbConnection,
    user_pk: String,
    invoice: Bolt11Invoice,
    amount_msat: i64,
    description: String,
    expiry_secs: u32,
) {
    let mut conn = db.get().expect("Failed to get connection from pool");

    let new_invoice = InvoiceRecord {
        id: invoice.payment_hash().as_byte_array().as_hex().to_string(),
        user_pk,
        amount_msat: Some(amount_msat),
        description,
        pr: invoice.to_string(),
        expires_at: unix_time() + expiry_secs as i64 * 1000,
        created_at: unix_time(),
    };

    tokio::task::spawn_blocking(move || {
        diesel::insert_into(invoice::table)
            .values(&new_invoice)
            .execute(&mut *conn)
            .expect("Failed to create invoice");
    })
    .await
    .expect("Failed to join task");
}

pub async fn create_offer(
    db: &DbConnection,
    user_pk: String,
    offer: Offer,
    amount_msat: Option<i64>,
    description: String,
    expiry_secs: Option<u32>,
) {
    let mut conn = db.get().expect("Failed to get connection from pool");

    let new_offer = OfferRecord {
        id: offer.id().0.as_hex().to_string(),
        user_pk,
        amount_msat,
        description,
        pr: offer.to_string(),
        expires_at: expiry_secs.map(|secs| unix_time() + secs as i64 * 1000),
        created_at: unix_time(),
    };

    tokio::task::spawn_blocking(move || {
        diesel::insert_into(offer::table)
            .values(&new_offer)
            .execute(&mut *conn)
            .expect("Failed to create offer");
    })
    .await
    .expect("Failed to join task");
}

pub async fn count_pending_invoices(db: &DbConnection, user_pk: String) -> i64 {
    let mut conn = db.get().expect("Failed to get connection from pool");

    tokio::task::spawn_blocking(move || {
        invoice::table
            .filter(invoice::user_pk.eq(user_pk))
            .filter(invoice::expires_at.gt(unix_time()))
            .left_join(receive::table.on(invoice::id.eq(receive::id)))
            .filter(receive::id.is_null())
            .count()
            .first::<i64>(&mut *conn)
            .expect("Failed to count pending invoices")
    })
    .await
    .expect("Failed to join task")
}

pub async fn get_invoice(db: &DbConnection, payment_hash: [u8; 32]) -> Option<InvoiceRecord> {
    let mut conn = db.get().expect("Failed to get connection from pool");

    tokio::task::spawn_blocking(move || {
        invoice::table
            .filter(invoice::id.eq(payment_hash.as_hex().to_string()))
            .first::<InvoiceRecord>(&mut *conn)
            .optional()
            .expect("Failed to query invoice")
    })
    .await
    .expect("Failed to join task")
}

pub async fn get_offer(db: &DbConnection, payment_id: [u8; 32]) -> Option<OfferRecord> {
    let mut conn = db.get().expect("Failed to get connection from pool");

    tokio::task::spawn_blocking(move || {
        offer::table
            .filter(offer::id.eq(payment_id.as_hex().to_string()))
            .first::<OfferRecord>(&mut *conn)
            .optional()
            .expect("Failed to query offer")
    })
    .await
    .expect("Failed to join task")
}

pub async fn get_offer_by_user_pk(db: &DbConnection, user_pk: String) -> Option<OfferRecord> {
    let mut conn = db.get().expect("Failed to get connection from pool");

    tokio::task::spawn_blocking(move || {
        offer::table
            .filter(offer::user_pk.eq(user_pk))
            .first::<OfferRecord>(&mut *conn)
            .optional()
            .expect("Failed to query offer")
    })
    .await
    .expect("Failed to join task")
}

pub async fn create_receive_payment(db: &DbConnection, record: ReceiveRecord) {
    let mut conn = db.get().expect("Failed to get connection from pool");

    tokio::task::spawn_blocking(move || {
        diesel::replace_into(receive::table)
            .values(&record)
            .execute(&mut *conn)
            .expect("Failed to create receive payment");
    })
    .await
    .expect("Failed to join task");
}

pub async fn user_balance(db: &DbConnection, user_pk: String) -> u64 {
    let mut conn = db.get().expect("Failed to get connection from pool");

    tokio::task::spawn_blocking(move || {
        let receive_sum: i64 = receive::table
            .filter(receive::user_pk.eq(user_pk.clone()))
            .select(receive::amount_msat)
            .load::<i64>(&mut *conn)
            .unwrap_or_default()
            .into_iter()
            .sum();

        let send_sum: i64 = send::table
            .filter(send::user_pk.eq(user_pk))
            .filter(send::status.ne("failed"))
            .select((send::amount_msat, send::fee_msat))
            .load::<(i64, i64)>(&mut *conn)
            .unwrap_or_default()
            .into_iter()
            .map(|(amount, fee)| amount + fee)
            .sum();

        (receive_sum as u64).checked_sub(send_sum as u64).unwrap()
    })
    .await
    .expect("Failed to join task")
}

pub async fn user_payments(db: &DbConnection, user_pk: String) -> Vec<puncture_api_core::Payment> {
    let mut conn = db.get().expect("Failed to get connection from pool");

    tokio::task::spawn_blocking(move || {
        // Load full ReceiveRecord records and convert using Into<Payment>
        let receive_payments: Vec<puncture_api_core::Payment> = receive::table
            .filter(receive::user_pk.eq(user_pk.clone()))
            .load::<ReceiveRecord>(&mut *conn)
            .unwrap_or_default()
            .into_iter()
            .map(|record| record.into())
            .collect();

        // Load full SendRecord records and convert using Into<Payment>
        let send_payments: Vec<puncture_api_core::Payment> = send::table
            .filter(send::user_pk.eq(user_pk))
            .load::<SendRecord>(&mut *conn)
            .unwrap_or_default()
            .into_iter()
            .map(|record| record.into())
            .collect();

        let mut all_payments = [receive_payments, send_payments].concat();

        all_payments.sort_by_key(|payment| payment.created_at);

        all_payments
    })
    .await
    .expect("Failed to join task")
}

pub async fn list_users(db: &DbConnection) -> Vec<UserInfo> {
    let mut conn = db.get().expect("Failed to get connection from pool");

    let user_records = tokio::task::spawn_blocking(move || {
        user::table
            .load::<User>(&mut *conn)
            .expect("Failed to load users")
    })
    .await
    .expect("Failed to join task");

    let mut user_infos = Vec::new();

    for user_record in user_records {
        user_infos.push(UserInfo {
            user_pk: user_record.user_pk.clone(),
            balance_msat: user_balance(db, user_record.user_pk).await,
            created_at: user_record.created_at,
        });
    }

    user_infos
}

pub async fn count_pending_sends(db: &DbConnection, user_pk: String) -> i64 {
    let mut conn = db.get().expect("Failed to get connection from pool");

    tokio::task::spawn_blocking(move || {
        send::table
            .filter(send::user_pk.eq(user_pk))
            .filter(send::status.eq("pending"))
            .count()
            .first::<i64>(&mut *conn)
            .expect("Failed to count pending invoices")
    })
    .await
    .expect("Failed to join task")
}

pub async fn user_exists(db: &DbConnection, user_pk: String) -> bool {
    let mut conn = db.get().expect("Failed to get connection from pool");

    tokio::task::spawn_blocking(move || {
        diesel::select(diesel::dsl::exists(
            user::table.filter(user::user_pk.eq(user_pk)),
        ))
        .get_result::<bool>(&mut *conn)
        .expect("Failed to check if user exists")
    })
    .await
    .expect("Failed to join task")
}

pub async fn create_invite(
    db: &DbConnection,
    invite_id: &[u8; 16],
    user_limit: u32,
    expiry_secs: u32,
) -> InviteRecord {
    let mut conn = db.get().expect("Failed to get connection from pool");

    let new_invite = InviteRecord {
        id: invite_id.as_hex().to_string(),
        user_limit: user_limit as i64,
        expires_at: unix_time() + expiry_secs as i64 * 1000,
        created_at: unix_time(),
    };

    tokio::task::spawn_blocking(move || {
        diesel::insert_into(invite::table)
            .values(&new_invite)
            .execute(&mut *conn)
            .expect("Failed to create invite");

        new_invite
    })
    .await
    .expect("Failed to join task")
}

pub async fn get_invite(db: &DbConnection, invite_id: &str) -> Option<InviteRecord> {
    let mut conn = db.get().expect("Failed to get connection from pool");
    let invite_id = invite_id.to_string();

    tokio::task::spawn_blocking(move || {
        invite::table
            .filter(invite::id.eq(invite_id))
            .first::<InviteRecord>(&mut *conn)
            .optional()
            .expect("Failed to query invite")
    })
    .await
    .expect("Failed to join task")
}

pub async fn count_invite_users(db: &DbConnection, invite_id: &str) -> i64 {
    let mut conn = db.get().expect("Failed to get connection from pool");
    let invite_id = invite_id.to_string();

    tokio::task::spawn_blocking(move || {
        user::table
            .filter(user::invite_id.eq(invite_id))
            .count()
            .first::<i64>(&mut *conn)
            .expect("Failed to count invite users")
    })
    .await
    .expect("Failed to join task")
}

pub async fn register_user_with_invite(db: &DbConnection, user_pk: String, invite_id: String) {
    let mut conn = db.get().expect("Failed to get connection from pool");

    tokio::task::spawn_blocking(move || {
        diesel::insert_into(user::table)
            .values(&User {
                user_pk,
                invite_id,
                created_at: unix_time(),
            })
            .execute(&mut *conn)
            .expect("Failed to register user with invite");
    })
    .await
    .expect("Failed to join task")
}
