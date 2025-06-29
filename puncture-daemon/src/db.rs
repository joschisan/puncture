use std::time::{SystemTime, UNIX_EPOCH};

use bitcoin::hashes::Hash;
use bitcoin::hex::DisplayHex;
use diesel::{ExpressionMethods, JoinOnDsl, OptionalExtension, QueryDsl, RunQueryDsl};
use diesel_migrations::{EmbeddedMigrations, embed_migrations};
use lightning_invoice::Bolt11Invoice;

use puncture_cli_core::UserInfo;
use puncture_core::db::DbConnection;

use crate::models::{Bolt11InvoiceRecord, Bolt11Receive, Bolt11Send, User};
use crate::schema::{bolt11_invoice, bolt11_receive, bolt11_send, users};

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

fn get_payment_hash_hex(invoice: &Bolt11Invoice) -> String {
    invoice.payment_hash().as_byte_array().as_hex().to_string()
}

pub fn unix_time() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_millis() as i64
}

pub async fn create_bolt11_send_payment(
    db: &DbConnection,
    user_pk: String,
    invoice: Bolt11Invoice,
    amount_msat: i64,
    fee_msat: i64,
    ln_address: Option<String>,
    status: String,
) -> Bolt11Send {
    let mut conn = db.get().expect("Failed to get connection from pool");

    let new_send = Bolt11Send {
        payment_hash: get_payment_hash_hex(&invoice),
        user_pk,
        amount_msat,
        fee_msat,
        description: invoice.description().to_string(),
        invoice: invoice.to_string(),
        created_at: unix_time(),
        status,
        ln_address,
    };

    tokio::task::spawn_blocking(move || {
        diesel::insert_into(bolt11_send::table)
            .values(&new_send)
            .execute(&mut *conn)
            .expect("Failed to insert send payment");

        new_send
    })
    .await
    .expect("Failed to join task")
}

pub async fn update_bolt11_send_payment_status(
    db: &DbConnection,
    payment_hash: [u8; 32],
    status: String,
) -> Bolt11Send {
    let mut conn = db.get().expect("Failed to get connection from pool");

    tokio::task::spawn_blocking(move || {
        diesel::update(bolt11_send::table.find(&payment_hash.as_hex().to_string()))
            .set(bolt11_send::status.eq(&status))
            .execute(&mut *conn)
            .expect("Failed to update payment status");

        bolt11_send::table
            .filter(bolt11_send::payment_hash.eq(payment_hash.as_hex().to_string()))
            .first::<Bolt11Send>(&mut *conn)
            .expect("Failed to fetch updated payment")
    })
    .await
    .expect("Failed to join task")
}

pub async fn create_bolt11_invoice(
    db: &DbConnection,
    user_pk: String,
    invoice: Bolt11Invoice,
    amount_msat: i64,
    description: String,
    expiry_secs: u32,
) {
    let mut conn = db.get().expect("Failed to get connection from pool");

    let new_invoice = Bolt11InvoiceRecord {
        payment_hash: get_payment_hash_hex(&invoice),
        user_pk,
        amount_msat,
        description,
        invoice: invoice.to_string(),
        expires_at: unix_time() + expiry_secs as i64 * 1000,
        created_at: unix_time(),
    };

    tokio::task::spawn_blocking(move || {
        diesel::insert_into(bolt11_invoice::table)
            .values(&new_invoice)
            .execute(&mut *conn)
            .expect("Failed to create invoice");
    })
    .await
    .expect("Failed to join task");
}

pub async fn count_pending_invoices(db: &DbConnection, user_pk: String) -> i64 {
    let mut conn = db.get().expect("Failed to get connection from pool");

    tokio::task::spawn_blocking(move || {
        bolt11_invoice::table
            .filter(bolt11_invoice::user_pk.eq(user_pk))
            .filter(bolt11_invoice::expires_at.gt(unix_time()))
            .left_join(
                bolt11_receive::table
                    .on(bolt11_invoice::payment_hash.eq(bolt11_receive::payment_hash)),
            )
            .filter(bolt11_receive::payment_hash.is_null())
            .count()
            .first::<i64>(&mut *conn)
            .expect("Failed to count pending invoices")
    })
    .await
    .expect("Failed to join task")
}

pub async fn bolt11_invoice(
    db: &DbConnection,
    payment_hash: [u8; 32],
) -> Option<Bolt11InvoiceRecord> {
    let mut conn = db.get().expect("Failed to get connection from pool");

    tokio::task::spawn_blocking(move || {
        bolt11_invoice::table
            .filter(bolt11_invoice::payment_hash.eq(payment_hash.as_hex().to_string()))
            .first::<Bolt11InvoiceRecord>(&mut *conn)
            .optional()
            .expect("Failed to query invoice")
    })
    .await
    .expect("Failed to join task")
}

pub async fn create_bolt11_receive_payment(db: &DbConnection, record: Bolt11Receive) {
    let mut conn = db.get().expect("Failed to get connection from pool");

    tokio::task::spawn_blocking(move || {
        diesel::replace_into(bolt11_receive::table)
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
        let receive_sum: i64 = bolt11_receive::table
            .filter(bolt11_receive::user_pk.eq(user_pk.clone()))
            .select(bolt11_receive::amount_msat)
            .load::<i64>(&mut *conn)
            .unwrap_or_default()
            .into_iter()
            .sum();

        let send_sum: i64 = bolt11_send::table
            .filter(bolt11_send::user_pk.eq(user_pk))
            .filter(bolt11_send::status.ne("failed"))
            .select((bolt11_send::amount_msat, bolt11_send::fee_msat))
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
        // Load full Bolt11Receive records and convert using Into<Payment>
        let receive_payments: Vec<puncture_api_core::Payment> = bolt11_receive::table
            .filter(bolt11_receive::user_pk.eq(user_pk.clone()))
            .load::<Bolt11Receive>(&mut *conn)
            .unwrap_or_default()
            .into_iter()
            .map(|record| record.into())
            .collect();

        // Load full Bolt11Send records and convert using Into<Payment>
        let send_payments: Vec<puncture_api_core::Payment> = bolt11_send::table
            .filter(bolt11_send::user_pk.eq(user_pk))
            .load::<Bolt11Send>(&mut *conn)
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
        users::table
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

pub async fn count_pending_bolt11_sends(db: &DbConnection, user_pk: String) -> i64 {
    let mut conn = db.get().expect("Failed to get connection from pool");

    tokio::task::spawn_blocking(move || {
        bolt11_send::table
            .filter(bolt11_send::user_pk.eq(user_pk))
            .filter(bolt11_send::status.eq("pending"))
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
            users::table.filter(users::user_pk.eq(user_pk)),
        ))
        .get_result::<bool>(&mut *conn)
        .expect("Failed to check if user exists")
    })
    .await
    .expect("Failed to join task")
}

pub async fn user_count(db: &DbConnection) -> i64 {
    let mut conn = db.get().expect("Failed to get connection from pool");

    tokio::task::spawn_blocking(move || {
        users::table
            .count()
            .first::<i64>(&mut *conn)
            .expect("Failed to count users")
    })
    .await
    .expect("Failed to join task")
}

pub async fn register_user(db: &DbConnection, user_pk: String) {
    let mut conn = db.get().expect("Failed to get connection from pool");

    tokio::task::spawn_blocking(move || {
        diesel::insert_into(users::table)
            .values(&User {
                user_pk,
                created_at: unix_time(),
            })
            .execute(&mut *conn)
            .expect("Failed to register user");
    })
    .await
    .expect("Failed to join task")
}
