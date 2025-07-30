use diesel::{Insertable, Queryable, Selectable};

#[derive(Queryable, Selectable, Insertable, Debug, Clone)]
#[diesel(table_name = crate::schema::user)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct User {
    pub user_pk: String,
    pub invite_id: String,
    pub recovery_name: Option<String>,
    pub created_at: i64,
}

#[derive(Queryable, Selectable, Insertable, Debug, Clone)]
#[diesel(table_name = crate::schema::invite)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct InviteRecord {
    pub id: String,
    pub user_limit: i64,
    pub expires_at: i64,
    pub created_at: i64,
}

#[derive(Queryable, Selectable, Insertable, Debug, Clone)]
#[diesel(table_name = crate::schema::invoice)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct InvoiceRecord {
    pub id: String,
    pub user_pk: String,
    pub amount_msat: Option<i64>,
    pub description: String,
    pub pr: String,
    pub expires_at: i64,
    pub created_at: i64,
}

#[derive(Queryable, Selectable, Insertable, Debug, Clone)]
#[diesel(table_name = crate::schema::receive)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct ReceiveRecord {
    pub id: String,
    pub user_pk: String,
    pub amount_msat: i64,
    pub description: String,
    pub pr: String,
    pub created_at: i64,
}

#[derive(Queryable, Selectable, Insertable, Debug, Clone)]
#[diesel(table_name = crate::schema::send)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct SendRecord {
    pub id: String,
    pub user_pk: String,
    pub amount_msat: i64,
    pub fee_msat: i64,
    pub description: String,
    pub pr: String,
    pub status: String,
    pub ln_address: Option<String>,
    pub created_at: i64,
}

#[derive(Queryable, Selectable, Insertable, Debug, Clone)]
#[diesel(table_name = crate::schema::offer)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct OfferRecord {
    pub id: String,
    pub user_pk: String,
    pub amount_msat: Option<i64>,
    pub description: String,
    pub pr: String,
    pub expires_at: Option<i64>,
    pub created_at: i64,
}

#[derive(Queryable, Selectable, Insertable, Debug, Clone)]
#[diesel(table_name = crate::schema::recovery)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct RecoveryRecord {
    pub id: String,
    pub user_pk: String,
    pub expires_at: i64,
    pub created_at: i64,
}
