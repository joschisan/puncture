use std::path::Path;

use anyhow::{Context, Result};
use diesel::r2d2::PooledConnection;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::sqlite::SqliteConnection;
use diesel_migrations::{EmbeddedMigrations, MigrationHarness};

#[derive(Clone)]
pub struct Database {
    pool: Pool<ConnectionManager<SqliteConnection>>,
}

impl Database {
    pub fn new(data_dir: &Path, migrations: EmbeddedMigrations, max_size: u32) -> Result<Self> {
        let file_path = data_dir.join("puncture_data.sqlite").display().to_string();

        let pool = Pool::builder()
            .max_size(max_size)
            .build(ConnectionManager::<SqliteConnection>::new(&file_path))
            .context("Error creating connection pool")?;

        let mut conn = pool.get().expect("Failed to get connection for migrations");

        conn.run_pending_migrations(migrations)
            .map_err(|e| anyhow::anyhow!("Database migration failed: {}", e))?;

        Ok(Database { pool })
    }

    pub async fn get_connection(&self) -> PooledConnection<ConnectionManager<SqliteConnection>> {
        let pool = self.pool.clone();

        tokio::task::spawn_blocking(move || pool.get().expect("Failed to get connection from pool"))
            .await
            .expect("Failed to join task")
    }
}
