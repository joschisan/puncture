use std::{path::Path, sync::Arc};

use anyhow::{Context, Result};
use diesel::Connection;
use diesel::sqlite::SqliteConnection;
use diesel_migrations::{EmbeddedMigrations, MigrationHarness};
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<SqliteConnection>>,
}

impl Database {
    pub fn new(data_dir: &Path, migrations: EmbeddedMigrations, _max_size: u32) -> Result<Self> {
        let file_path = data_dir.join("puncture_data.sqlite").display().to_string();

        let mut conn = SqliteConnection::establish(&file_path)
            .context("Error establishing connection to database")?;

        conn.run_pending_migrations(migrations)
            .map_err(|e| anyhow::anyhow!("Database migration failed: {}", e))?;

        Ok(Database {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub async fn get_connection(&self) -> tokio::sync::MutexGuard<'_, SqliteConnection> {
        self.conn.lock().await
    }
}
