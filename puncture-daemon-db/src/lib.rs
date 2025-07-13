pub mod models;
pub mod schema;

use diesel_migrations::{EmbeddedMigrations, embed_migrations};

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");
