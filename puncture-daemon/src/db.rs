use bitcoin::hex::DisplayHex;
use diesel::SqliteConnection;
use diesel::{ExpressionMethods, OptionalExtension, QueryDsl, RunQueryDsl};

use puncture_daemon_db::models::{InvoiceRecord, OfferRecord, ReceiveRecord, SendRecord};
use puncture_daemon_db::schema::{invoice, offer, receive, send};
use tracing::info;

pub async fn get_invoice(
    conn: &mut SqliteConnection,
    payment_hash: [u8; 32],
) -> Option<InvoiceRecord> {
    invoice::table
        .filter(invoice::id.eq(payment_hash.as_hex().to_string()))
        .first::<InvoiceRecord>(conn)
        .optional()
        .expect("Failed to query invoice")
}

pub async fn get_offer(conn: &mut SqliteConnection, payment_id: [u8; 32]) -> Option<OfferRecord> {
    offer::table
        .filter(offer::id.eq(payment_id.as_hex().to_string()))
        .first::<OfferRecord>(conn)
        .optional()
        .expect("Failed to query offer")
}

pub async fn update_send_status(
    conn: &mut SqliteConnection,
    id: [u8; 32],
    status: &str,
    fee_paid_msat: i64,
) -> Option<SendRecord> {
    info!(id = ?id.as_hex(), ?status, "Updating send status");

    let status = status.to_string();

    diesel::update(send::table.find(&id.as_hex().to_string()))
        .set(send::status.eq(&status))
        .execute(conn)
        .expect("Failed to update payment status");

    diesel::update(send::table.find(&id.as_hex().to_string()))
        .set(send::fee_msat.eq(fee_paid_msat))
        .execute(conn)
        .expect("Failed to update fee paid");

    send::table
        .filter(send::id.eq(id.as_hex().to_string()))
        .first::<SendRecord>(conn)
        .optional()
        .expect("Failed to fetch updated payment")
}

pub async fn create_receive_payment(conn: &mut SqliteConnection, record: ReceiveRecord) {
    diesel::insert_into(receive::table)
        .values(&record)
        .on_conflict(receive::id)
        .do_nothing()
        .execute(conn)
        .expect("Failed to create receive payment");
}

pub async fn user_balance(conn: &mut SqliteConnection, user_pk: String) -> u64 {
    let receive_sum: i64 = receive::table
        .filter(receive::user_pk.eq(user_pk.clone()))
        .select(receive::amount_msat)
        .load::<i64>(conn)
        .unwrap_or_default()
        .into_iter()
        .sum();

    let send_sum: i64 = send::table
        .filter(send::user_pk.eq(user_pk))
        .filter(send::status.ne("failed"))
        .select((send::amount_msat, send::fee_msat))
        .load::<(i64, i64)>(conn)
        .unwrap_or_default()
        .into_iter()
        .map(|(amount, fee)| amount + fee)
        .sum();

    receive_sum.saturating_sub(send_sum) as u64
}
