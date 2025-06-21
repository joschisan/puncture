use bitcoin::hex::DisplayHex;
use puncture_client_core::Payment;
use puncture_daemon_db::models::{InvoiceRecord, OfferRecord, ReceiveRecord, SendRecord};

use puncture_core::unix_time;

pub trait IntoPayment {
    fn into_payment(self, is_live: bool) -> Payment;
}

impl IntoPayment for ReceiveRecord {
    fn into_payment(self, is_live: bool) -> Payment {
        Payment {
            id: self.id,
            payment_type: "receive".to_string(),
            is_live,
            amount_msat: self.amount_msat,
            fee_msat: 0,
            description: self.description,
            ln_address: None,
            status: "successful".to_string(),
            created_at: self.created_at,
        }
    }
}

impl IntoPayment for SendRecord {
    fn into_payment(self, is_live: bool) -> Payment {
        Payment {
            id: self.id,
            payment_type: "send".to_string(),
            is_live,
            amount_msat: self.amount_msat,
            fee_msat: self.fee_msat,
            description: self.description,
            ln_address: self.ln_address,
            status: self.status,
            created_at: self.created_at,
        }
    }
}

pub trait IntoReceiveRecord {
    fn into_receive_record(self, id: [u8; 32], amount_msat: u64) -> ReceiveRecord;
}

impl IntoReceiveRecord for InvoiceRecord {
    fn into_receive_record(self, id: [u8; 32], amount_msat: u64) -> ReceiveRecord {
        ReceiveRecord {
            id: id.as_hex().to_string(),
            user_pk: self.user_pk,
            amount_msat: amount_msat as i64,
            description: self.description,
            pr: self.pr,
            created_at: unix_time(),
        }
    }
}

impl IntoReceiveRecord for OfferRecord {
    fn into_receive_record(self, id: [u8; 32], amount_msat: u64) -> ReceiveRecord {
        ReceiveRecord {
            id: id.as_hex().to_string(),
            user_pk: self.user_pk,
            amount_msat: amount_msat as i64,
            description: self.description,
            pr: self.pr,
            created_at: unix_time(),
        }
    }
}
