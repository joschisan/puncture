use bitcoin::hex::DisplayHex;
use diesel::RunQueryDsl;

use puncture_cli_core::UserInfo;
use puncture_core::db::Database;
use puncture_core::unix_time;
use puncture_daemon_db::models::{InviteRecord, User};
use puncture_daemon_db::schema::{invite, user};

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
            created_at: user_record.created_at,
        });
    }

    user_infos
}
