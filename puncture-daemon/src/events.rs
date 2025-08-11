use futures::StreamExt;
use tokio::sync::broadcast;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
use tokio_stream::{Stream, wrappers::BroadcastStream};
use tracing::trace;

use puncture_client_core::{AppEvent, Balance, Payment, Update};

#[derive(Clone)]
pub struct EventBus {
    tx: broadcast::Sender<(String, AppEvent)>,
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        Self {
            tx: broadcast::channel(capacity).0,
        }
    }

    pub fn send_balance_event(&self, user_id: String, amount_msat: u64) {
        trace!(?user_id, ?amount_msat, "Balance event");

        self.tx
            .send((user_id, AppEvent::Balance(Balance { amount_msat })))
            .ok();
    }

    pub fn send_payment_event(&self, user_id: String, payment: Payment) {
        trace!(?user_id, ?payment, "Payment event");

        self.tx.send((user_id, AppEvent::Payment(payment))).ok();
    }

    pub fn send_update_event(&self, user_id: String, id: String, status: &str, fee_msat: i64) {
        trace!(?user_id, ?id, ?status, "Update event");

        self.tx
            .send((
                user_id,
                AppEvent::Update(Update {
                    id,
                    status: status.to_string(),
                    fee_msat,
                }),
            ))
            .ok();
    }

    pub fn subscribe_to_events(
        &self,
        user_id: String,
    ) -> impl Stream<Item = Result<AppEvent, String>> + Send + 'static + use<> {
        BroadcastStream::new(self.tx.subscribe()).filter_map(move |r| filter(user_id.clone(), r))
    }
}

async fn filter<T>(
    user_id: String,
    result: Result<(String, T), BroadcastStreamRecvError>,
) -> Option<Result<T, String>> {
    match result {
        Ok((event_user_id, event)) => {
            if event_user_id == user_id {
                Some(Ok(event))
            } else {
                None
            }
        }
        Err(e) => Some(Err(e.to_string())),
    }
}
