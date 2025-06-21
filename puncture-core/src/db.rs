use std::path::Path;

use anyhow::{Context, Result};
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::sqlite::SqliteConnection;
use diesel_migrations::{EmbeddedMigrations, MigrationHarness};

pub type DbConnection = Pool<ConnectionManager<SqliteConnection>>;

pub fn setup_database(data_dir: &Path, migrations: EmbeddedMigrations) -> Result<DbConnection> {
    let file_path = data_dir.join("puncture_data.sqlite").display().to_string();

    let pool = Pool::builder()
        .max_size(10)
        .build(ConnectionManager::<SqliteConnection>::new(&file_path))
        .context("Error creating connection pool")?;

    let mut conn = pool
        .get()
        .expect("Failed to get connection from pool for migrations");

    conn.run_pending_migrations(migrations)
        .map_err(|e| anyhow::anyhow!("Database migration failed: {}", e))?;

    Ok(pool)
}
