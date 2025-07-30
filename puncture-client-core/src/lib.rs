use bitcoin::Network;
use lightning_invoice::Bolt11Invoice;
use serde::{Deserialize, Serialize};

/// A helper struct for JSON-RPC requests over Iroh
#[derive(Serialize, Deserialize, Debug)]
pub struct ClientRpcRequest<R> {
    pub method: String,
    pub request: R,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Payment {
    /// The payment id
    pub id: String,
    /// The payment type, "send" or "receive"
    pub payment_type: String,
    /// Wether the payment was processed after the client connected
    pub is_live: bool,
    /// The amount in millisatoshis
    pub amount_msat: i64,
    /// The fee in millisatoshis
    pub fee_msat: i64,
    /// The description of the payment
    pub description: String,
    /// The status of the payment: "pending", "successful", or "failed"
    pub status: String,
    /// The lightning address of the payment
    pub ln_address: Option<String>,
    /// The creation time of the payment
    pub created_at: i64,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterRequest {
    /// The invite id
    pub invite_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegisterResponse {
    /// The bitcoin network the daemon is running on
    pub network: Network,
    /// The name of the daemon
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeesResponse {
    /// Fee rate in parts per million (PPM)
    pub fee_ppm: u64,
    /// Base fee in millisatoshis
    pub base_fee_msat: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bolt11ReceiveRequest {
    /// Amount in millisatoshis
    pub amount_msat: u32,
    /// Description of the invoice
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bolt11ReceiveResponse {
    /// The generated invoice
    pub invoice: Bolt11Invoice,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bolt12ReceiveResponse {
    /// The offer to receive
    pub offer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bolt11SendRequest {
    /// The invoice to pay
    pub invoice: Bolt11Invoice,
    /// Amount override in millisatoshis
    pub amount_msat: u64,
    /// The lightning address we retrived the invoice from
    pub ln_address: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bolt12SendRequest {
    /// The offer to pay
    pub offer: String,
    /// Amount override in millisatoshis
    pub amount_msat: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetRecoveryNameRequest {
    /// The recovery name
    pub recovery_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoverRequest {
    /// The recovery id
    pub recovery_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoverResponse {
    /// The recovered balance in millisatoshis
    pub balance_msat: u64,
}
