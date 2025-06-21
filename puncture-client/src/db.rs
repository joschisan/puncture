use std::time::{SystemTime, UNIX_EPOCH};

use diesel::RunQueryDsl;
use diesel_migrations::{EmbeddedMigrations, embed_migrations};

use puncture_api_core::RegisterResponse;
use puncture_core::db::DbConnection;

use crate::models::DaemonRecord;
use crate::schema::daemon;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

pub fn unix_time() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_millis() as i64
}

pub fn save_daemon(db: &DbConnection, node_id: String, config: RegisterResponse) {
    let mut conn = db.get().expect("Failed to get connection from pool");

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

pub fn get_daemons(db: &DbConnection) -> Vec<DaemonRecord> {
    let mut conn = db.get().expect("Failed to get connection from pool");

    daemon::dsl::daemon
        .load::<DaemonRecord>(&mut conn)
        .expect("Failed to load daemons")
}
