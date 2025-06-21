use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};

use puncture_client_core::RegisterResponse;
use puncture_client_db::models::DaemonRecord;
use puncture_client_db::schema::daemon;
use puncture_core::{db::Database, unix_time};

pub async fn save_daemon(db: &Database, node_id: iroh::NodeId, config: RegisterResponse) {
    diesel::insert_or_ignore_into(daemon::table)
        .values(&DaemonRecord {
            node_id: node_id.to_string(),
            network: config.network.to_string(),
            name: config.name,
            created_at: unix_time(),
        })
        .execute(&mut db.get_connection().await)
        .expect("Failed to save daemon");
}

pub async fn list_daemons(db: &Database) -> Vec<DaemonRecord> {
    daemon::dsl::daemon
        .load::<DaemonRecord>(&mut db.get_connection().await)
        .expect("Failed to load daemons")
}

pub async fn delete_daemon(db: &Database, node_id: iroh::NodeId) {
    diesel::delete(daemon::table.filter(daemon::node_id.eq(node_id.to_string())))
        .execute(&mut db.get_connection().await)
        .expect("Failed to remove daemon");
}
