use bitcoin::hex::DisplayHex;
use diesel::ExpressionMethods;
use diesel::{QueryDsl, RunQueryDsl};

use puncture_cli_core::UserInfo;
use puncture_core::db::Database;
use puncture_core::unix_time;
use puncture_daemon_db::models::{InviteRecord, RecoveryRecord, User};
use puncture_daemon_db::schema::{invite, recovery, user};

pub async fn create_invite(
    db: &Database,
    invite_id: &[u8; 16],
    user_limit: u32,
    expiry_secs: u32,
) -> InviteRecord {
    let mut conn = db.get_connection().await;

    let new_invite = InviteRecord {
        id: invite_id.as_hex().to_string(),
        user_limit: user_limit as i64,
        expires_at: unix_time() + expiry_secs as i64 * 1000,
        created_at: unix_time(),
    };

    diesel::insert_into(invite::table)
        .values(&new_invite)
        .execute(&mut *conn)
        .expect("Failed to create invite");

    new_invite
}

pub async fn create_recovery(
    db: &Database,
    recovery_id: &[u8; 16],
    user_pk: &str,
    expiry_secs: u32,
) -> RecoveryRecord {
    let mut conn = db.get_connection().await;

    let new_recovery = RecoveryRecord {
        id: recovery_id.as_hex().to_string(),
        user_pk: user_pk.to_string(),
        expires_at: unix_time() + expiry_secs as i64 * 1000,
        created_at: unix_time(),
    };

    diesel::insert_into(recovery::table)
        .values(&new_recovery)
        .execute(&mut *conn)
        .expect("Failed to create recovery");

    new_recovery
}

pub async fn user_exists(db: &Database, user_pk: String) -> bool {
    let mut conn = db.get_connection().await;

    diesel::select(diesel::dsl::exists(
        user::table.filter(user::user_pk.eq(user_pk)),
    ))
    .get_result::<bool>(&mut *conn)
    .expect("Failed to check if user exists")
}

pub async fn list_users(db: &Database) -> Vec<UserInfo> {
    let mut conn = db.get_connection().await;

    let user_records = user::table
        .load::<User>(&mut *conn)
        .expect("Failed to load users");

    let mut user_infos = Vec::new();

    for user_record in user_records {
        user_infos.push(UserInfo {
            user_pk: user_record.user_pk.clone(),
            balance_msat: crate::db::user_balance(db, user_record.user_pk.clone()).await,
            recovery_name: user_record.recovery_name,
            created_at: user_record.created_at,
        });
    }

    user_infos
}
