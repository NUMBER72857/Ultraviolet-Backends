//! Background reconciliation worker for payment truth.
//!
//! The worker handles safe database-owned maintenance now: expiring stale
//! invoices and scanning submitted transaction hashes. Horizon verification is
//! kept behind `stellar::StellarTransactionLookup` so adding the real scraper
//! client does not change invoice state rules.

use std::time::Duration;

use sqlx::PgPool;
use tokio::time;

use crate::stellar::{LookupError, ObservedPayment, StellarTransactionLookup};

#[derive(Clone, Debug)]
pub struct ReconciliationWorkerConfig {
    pub enabled: bool,
    pub interval_seconds: u64,
}

pub fn spawn(pool: PgPool, config: ReconciliationWorkerConfig) {
    if !config.enabled {
        tracing::info!("reconciliation worker disabled");
        return;
    }

    tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(config.interval_seconds.max(5)));
        loop {
            interval.tick().await;
            if let Err(error) = run_once(&pool).await {
                tracing::error!(?error, "reconciliation worker iteration failed");
            }
        }
    });
}

async fn run_once(pool: &PgPool) -> Result<(), sqlx::Error> {
    expire_overdue_invoices(pool).await?;
    scan_submitted_hashes(pool).await?;
    Ok(())
}

async fn expire_overdue_invoices(pool: &PgPool) -> Result<(), sqlx::Error> {
    let expired = sqlx::query(
        r#"
        UPDATE invoices
        SET state = 'expired', updated_at = now()
        WHERE state = 'pending' AND expires_at <= now()
        "#,
    )
    .execute(pool)
    .await?
    .rows_affected();

    if expired > 0 {
        tracing::info!(expired, "expired overdue invoices");
    }

    Ok(())
}

async fn scan_submitted_hashes(pool: &PgPool) -> Result<(), sqlx::Error> {
    let pending_count: i64 = sqlx::query_scalar(
        r#"
        SELECT count(*)
        FROM payment_attempts
        WHERE status = 'submitted' AND transaction_hash IS NOT NULL
        "#,
    )
    .fetch_one(pool)
    .await?;

    if pending_count > 0 {
        tracing::warn!(
            pending_count,
            "submitted payment hashes are waiting for a Horizon lookup implementation"
        );
    }

    Ok(())
}

pub struct HorizonLookupDisabled;

impl StellarTransactionLookup for HorizonLookupDisabled {
    fn payment_by_hash(&self, _transaction_hash: &str) -> Result<ObservedPayment, LookupError> {
        Err(LookupError::Unavailable)
    }
}
