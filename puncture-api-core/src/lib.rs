use lightning_invoice::Bolt11Invoice;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Payment {
    /// The payment id
    pub id: String,
    // The payment type
    pub payment_type: String,
    /// The amount in millisatoshis (positive for incoming, negative for outgoing)
    pub amount_msat: i64,
    /// The fee in millisatoshis
    pub fee_msat: i64,
    /// The description of the payment
    pub description: String,
    /// The bolt11 invoice string
    pub bolt11_invoice: String,
    /// The creation time of the payment
    pub created_at: i64,
    /// The status of the payment: "pending", "successful", or "failed"
    pub status: String,
    /// The lightning address of the payment
    pub ln_address: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Balance {
    /// The user's balance in millisatoshis
    pub amount_msat: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Update {
    /// The payment id being updated
    pub id: String,
    /// The new status of the payment
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AppEvent {
    Balance(Balance),
    Payment(Payment),
    Update(Update),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConfigResponse {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserBolt11ReceiveRequest {
    /// Amount in millisatoshis
    pub amount_msat: u32,
    /// Description of the invoice
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bolt11ReceiveResponse {
    /// The generated invoice
    pub invoice: Bolt11Invoice,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserBolt11SendRequest {
    /// The invoice to pay
    pub invoice: Bolt11Invoice,
    /// The lightning address we retrived the invoice from
    pub ln_address: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserBolt11QuoteRequest {
    /// The BOLT11 invoice to quote
    pub invoice: Bolt11Invoice,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bolt11QuoteResponse {
    /// Amount in millisatoshis
    pub amount_msat: u64,
    /// Fee in millisatoshis
    pub fee_msat: u64,
    /// Description of the invoice
    pub description: String,
    /// Expiry time in seconds
    pub expiry_secs: u64,
}
