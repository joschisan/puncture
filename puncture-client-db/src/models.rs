use diesel::{Insertable, Queryable, Selectable};
use serde::{Deserialize, Serialize};

#[derive(Queryable, Selectable, Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = crate::schema::daemon)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct DaemonRecord {
    pub node_id: String,
    pub network: String,
    pub name: String,
    pub created_at: i64,
}
