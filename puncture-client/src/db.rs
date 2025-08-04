use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl, SqliteConnection};

use puncture_client_core::RegisterResponse;
use puncture_client_db::models::DaemonRecord;
use puncture_client_db::schema::daemon;
use puncture_core::unix_time;

pub async fn save_daemon(
    conn: &mut SqliteConnection,
    node_id: iroh::NodeId,
    config: RegisterResponse,
) {
    diesel::insert_or_ignore_into(daemon::table)
        .values(&DaemonRecord {
            node_id: node_id.to_string(),
            network: config.network.to_string(),
            name: config.name,
            created_at: unix_time(),
        })
        .execute(conn)
        .expect("Failed to save daemon");
}

pub async fn list_daemons(conn: &mut SqliteConnection) -> Vec<DaemonRecord> {
    daemon::dsl::daemon
        .load::<DaemonRecord>(conn)
        .expect("Failed to load daemons")
}

pub async fn delete_daemon(conn: &mut SqliteConnection, node_id: iroh::NodeId) {
    diesel::delete(daemon::table.filter(daemon::node_id.eq(node_id.to_string())))
        .execute(conn)
        .expect("Failed to remove daemon");
}
