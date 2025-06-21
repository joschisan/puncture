use bitcoin::hex::DisplayHex;
use diesel::{ExpressionMethods, OptionalExtension, QueryDsl, RunQueryDsl};
use puncture_core::db::Database;
use puncture_daemon_db::models::{InvoiceRecord, OfferRecord, ReceiveRecord, SendRecord};
use puncture_daemon_db::schema::{invoice, offer, receive, send};
use tracing::info;

pub async fn get_invoice(db: &Database, payment_hash: [u8; 32]) -> Option<InvoiceRecord> {
    let mut conn = db.get_connection().await;

    invoice::table
        .filter(invoice::id.eq(payment_hash.as_hex().to_string()))
        .first::<InvoiceRecord>(&mut *conn)
        .optional()
        .expect("Failed to query invoice")
}

pub async fn get_offer(db: &Database, payment_id: [u8; 32]) -> Option<OfferRecord> {
    let mut conn = db.get_connection().await;

    offer::table
        .filter(offer::id.eq(payment_id.as_hex().to_string()))
        .first::<OfferRecord>(&mut *conn)
        .optional()
        .expect("Failed to query offer")
}

pub async fn update_send_status(db: &Database, id: [u8; 32], status: &str) -> SendRecord {
    let mut conn = db.get_connection().await;

    info!(id = ?id.as_hex(), ?status, "Updating send status");

    let status = status.to_string();

    diesel::update(send::table.find(&id.as_hex().to_string()))
        .set(send::status.eq(&status))
        .execute(&mut *conn)
        .expect("Failed to update payment status");

    send::table
        .filter(send::id.eq(id.as_hex().to_string()))
        .first::<SendRecord>(&mut *conn)
        .expect("Failed to fetch updated payment")
}

pub async fn create_receive_payment(db: &Database, record: ReceiveRecord) {
    let mut conn = db.get_connection().await;

    diesel::insert_into(receive::table)
        .values(&record)
        .on_conflict(receive::id)
        .do_nothing()
        .execute(&mut *conn)
        .expect("Failed to create receive payment");
}

pub async fn user_balance(db: &Database, user_pk: String) -> u64 {
    let mut conn = db.get_connection().await;

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
}
