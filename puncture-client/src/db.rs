use std::time::{SystemTime, UNIX_EPOCH};

use diesel::RunQueryDsl;
use diesel_migrations::{EmbeddedMigrations, embed_migrations};

use puncture_api_core::ConfigResponse;
use puncture_core::db::DbConnection;

use crate::models::InstanceRecord;
use crate::schema::instances;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

pub fn unix_time() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_millis() as i64
}

pub fn save_instance_config(db: &DbConnection, node_id: String, config: ConfigResponse) {
    let mut conn = db.get().expect("Failed to get connection from pool");

    diesel::insert_or_ignore_into(instances::table)
        .values(&InstanceRecord {
            node_id,
            name: config.name,
            created_at: unix_time(),
        })
        .execute(&mut conn)
        .expect("Failed to save instance");
}

pub fn get_instances(db: &DbConnection) -> Vec<InstanceRecord> {
    let mut conn = db.get().expect("Failed to get connection from pool");

    instances::dsl::instances
        .load::<InstanceRecord>(&mut conn)
        .expect("Failed to load instances")
}
