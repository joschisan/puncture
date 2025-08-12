use std::str::FromStr;

use lightning::offers::offer::{Amount, Offer};
use lightning_invoice::{Bolt11Invoice, Bolt11InvoiceDescriptionRef};
use lnurl_pay::{lud06::LnUrl, lud16::LightningAddress};
use serde::Deserialize;

#[allow(clippy::large_enum_variant)]
pub enum PaymentRequestWithAmount {
    Bolt11(Bolt11PaymentRequest),
    Bolt12(Bolt12PaymentRequest),
}

pub struct Bolt11PaymentRequest {
    pub invoice: Bolt11Invoice,
    pub amount_msat: u64,
    pub ln_address: Option<String>,
}

pub struct Bolt12PaymentRequest {
    pub offer: Offer,
    pub amount_msat: u64,
}

impl PaymentRequestWithAmount {
    pub fn amount_msat(&self) -> u64 {
        match self {
            PaymentRequestWithAmount::Bolt11(request) => request.amount_msat,
            PaymentRequestWithAmount::Bolt12(request) => request.amount_msat,
        }
    }

    pub fn description(&self) -> String {
        match self {
            PaymentRequestWithAmount::Bolt11(request) => match request.invoice.description() {
                Bolt11InvoiceDescriptionRef::Direct(description) => description.to_string(),
                Bolt11InvoiceDescriptionRef::Hash(..) => String::new(),
            },
            PaymentRequestWithAmount::Bolt12(request) => request
                .offer
                .description()
                .map(|description| description.to_string())
                .unwrap_or_default(),
        }
    }
}

#[allow(clippy::large_enum_variant)]
pub enum PaymentRequestWithoutAmount {
    Bolt11(Bolt11Invoice),
    Bolt12(Offer),
    LnUrl(LnUrl),
    LightningAddress(LightningAddress),
}

pub fn parse_with_amount(request: String) -> Option<PaymentRequestWithAmount> {
    if let Some(stripped) = request.strip_prefix("lightning:") {
        return parse_with_amount(stripped.to_string());
    }

    if let Ok(invoice) = Bolt11Invoice::from_str(&request) {
        if let Some(amount_msat) = invoice.amount_milli_satoshis() {
            return Some(PaymentRequestWithAmount::Bolt11(Bolt11PaymentRequest {
                invoice,
                amount_msat,
                ln_address: None,
            }));
        }
    }

    if let Ok(offer) = Offer::from_str(&request) {
        if let Some(Amount::Bitcoin { amount_msats }) = offer.amount() {
            return Some(PaymentRequestWithAmount::Bolt12(Bolt12PaymentRequest {
                offer,
                amount_msat: amount_msats,
            }));
        }
    }

    None
}

pub fn parse_without_amount(request: String) -> Option<PaymentRequestWithoutAmount> {
    if let Some(stripped) = request.strip_prefix("lightning:") {
        return parse_without_amount(stripped.to_string());
    }

    if let Some(stripped) = request.strip_prefix("lnurl:") {
        return parse_without_amount(stripped.to_string());
    }

    if let Ok(invoice) = Bolt11Invoice::from_str(&request) {
        if invoice.amount_milli_satoshis().is_none() {
            return Some(PaymentRequestWithoutAmount::Bolt11(invoice));
        }
    }

    if let Ok(offer) = Offer::from_str(&request) {
        if offer.amount().is_none() {
            return Some(PaymentRequestWithoutAmount::Bolt12(offer));
        }
    }

    if let Ok(lnurl) = LnUrl::from_str(&request) {
        return Some(PaymentRequestWithoutAmount::LnUrl(lnurl));
    }

    if let Ok(lightning_address) = LightningAddress::from_str(&request) {
        return Some(PaymentRequestWithoutAmount::LightningAddress(
            lightning_address,
        ));
    }

    None
}

pub async fn resolve(
    request: &PaymentRequestWithoutAmount,
    amount_msat: u64,
) -> Result<PaymentRequestWithAmount, String> {
    match request {
        PaymentRequestWithoutAmount::Bolt11(invoice) => {
            Ok(PaymentRequestWithAmount::Bolt11(Bolt11PaymentRequest {
                invoice: invoice.clone(),
                amount_msat,
                ln_address: None,
            }))
        }
        PaymentRequestWithoutAmount::Bolt12(offer) => {
            Ok(PaymentRequestWithAmount::Bolt12(Bolt12PaymentRequest {
                offer: offer.clone(),
                amount_msat,
            }))
        }
        PaymentRequestWithoutAmount::LnUrl(lnurl) => {
            Ok(PaymentRequestWithAmount::Bolt11(Bolt11PaymentRequest {
                invoice: resolve_endpoint(lnurl.endpoint(), amount_msat).await?,
                amount_msat,
                ln_address: None,
            }))
        }
        PaymentRequestWithoutAmount::LightningAddress(ln_address) => {
            Ok(PaymentRequestWithAmount::Bolt11(Bolt11PaymentRequest {
                invoice: resolve_endpoint(ln_address.endpoint(), amount_msat).await?,
                amount_msat,
                ln_address: Some(ln_address.to_string()),
            }))
        }
    }
}

#[derive(Deserialize)]
struct LnUrlPayResponse {
    callback: String,
    #[serde(alias = "minSendable")]
    min_sendable: u64,
    #[serde(alias = "maxSendable")]
    max_sendable: u64,
}

#[derive(Deserialize)]
struct LnUrlPayInvoiceResponse {
    pr: Bolt11Invoice,
}

async fn resolve_endpoint(endpoint: String, amount: u64) -> Result<Bolt11Invoice, String> {
    let response = reqwest::get(endpoint)
        .await
        .map_err(|_| "Failed to fetch LNURL".to_string())?
        .json::<LnUrlPayResponse>()
        .await
        .map_err(|_| "Failed to parse LNURL response".to_string())?;

    if amount < response.min_sendable {
        return Err("Amount too low".to_string());
    }

    if amount > response.max_sendable {
        return Err("Amount too high".to_string());
    }

    let callback_url = format!("{}?amount={}", response.callback, amount);

    let response = reqwest::get(callback_url)
        .await
        .map_err(|_| "Failed to fetch LNURL callback".to_string())?
        .json::<LnUrlPayInvoiceResponse>()
        .await
        .map_err(|_| "Failed to parse LNURL callback response".to_string())?;

    Ok(response.pr)
}
