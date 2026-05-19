//! PostgreSQL pool construction.
//!
//! Database connectivity is centralized here so pool sizing and connection
//! timeout behavior stay consistent across the API and background workers.

use sqlx::{postgres::PgPoolOptions, PgPool};
use std::time::Duration;

pub async fn connect(database_url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(5))
        .connect(database_url)
        .await
}
