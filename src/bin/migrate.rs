//! Raw SQL migration runner.
//!
//! The backend intentionally keeps migrations simple and inspectable: this
//! binary applies the core schema file and records it in `schema_migrations`.

use sqlx::postgres::PgPoolOptions;
use std::{env, time::Duration};

const CORE_MIGRATION_NAME: &str = "001_core_payment_schema.sql";
const CORE_MIGRATION_SQL: &str = include_str!("../../migrations/001_core_payment_schema.sql");

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = env::var("DATABASE_URL")
        .map_err(|_| "DATABASE_URL is required and must point at PostgreSQL")?;
    if !database_url.starts_with("postgres://") && !database_url.starts_with("postgresql://") {
        return Err("DATABASE_URL must be PostgreSQL".into());
    }

    let pool = PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&database_url)
        .await?;

    sqlx::raw_sql(CORE_MIGRATION_SQL).execute(&pool).await?;
    sqlx::query(
        r#"
        INSERT INTO schema_migrations (filename)
        VALUES ($1)
        ON CONFLICT (filename) DO NOTHING
        "#,
    )
    .bind(CORE_MIGRATION_NAME)
    .execute(&pool)
    .await?;

    println!("applied {CORE_MIGRATION_NAME}");
    Ok(())
}
