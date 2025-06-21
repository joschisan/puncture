use std::time::{SystemTime, UNIX_EPOCH};

use diesel::RunQueryDsl;

use puncture_client_core::RegisterResponse;
use puncture_client_db::models::DaemonRecord;
use puncture_client_db::schema::daemon;
use puncture_core::db::Database;

pub fn unix_time() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_millis() as i64
}

pub async fn save_daemon(db: &Database, node_id: String, config: RegisterResponse) {
    let mut conn = db.get_connection().await;

    diesel::insert_or_ignore_into(daemon::table)
        .values(&DaemonRecord {
            node_id,
            network: config.network.to_string(),
            name: config.name,
            created_at: unix_time(),
        })
        .execute(&mut conn)
        .expect("Failed to save daemon");
}

pub async fn get_daemons(db: &Database) -> Vec<DaemonRecord> {
    let mut conn = db.get_connection().await;

    daemon::dsl::daemon
        .load::<DaemonRecord>(&mut conn)
        .expect("Failed to load daemons")
}
