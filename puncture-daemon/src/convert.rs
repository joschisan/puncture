use puncture_client_core::Payment;
use puncture_daemon_db::models::{ReceiveRecord, SendRecord};

pub trait ToPayment {
    fn to_payment(self, is_live: bool) -> Payment;
}

impl ToPayment for ReceiveRecord {
    fn to_payment(self, is_live: bool) -> Payment {
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

impl ToPayment for SendRecord {
    fn to_payment(self, is_live: bool) -> Payment {
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
