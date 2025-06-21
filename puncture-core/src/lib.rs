pub mod db;
pub mod invite;
pub mod secret;

use std::time::{SystemTime, UNIX_EPOCH};

/// Returns the current time as milliseconds since Unix epoch
pub fn unix_time() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_millis() as i64
}
