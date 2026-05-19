//! Payout worker boundary.
//!
//! Payout submission requires signer isolation and Stellar transaction
//! construction. This worker intentionally refuses to submit anything until a
//! real signer is configured; pretending here would create a false sense of
//! production readiness around merchant funds.

use std::time::Duration;

use sqlx::PgPool;
use tokio::time;

#[derive(Clone, Debug)]
pub struct PayoutWorkerConfig {
    pub enabled: bool,
    pub interval_seconds: u64,
}

pub fn spawn(pool: PgPool, config: PayoutWorkerConfig) {
    if !config.enabled {
        tracing::info!("payout worker disabled");
        return;
    }

    tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(config.interval_seconds.max(5)));
        loop {
            interval.tick().await;
            if let Err(error) = run_once(&pool).await {
                tracing::error!(?error, "payout worker iteration failed");
            }
        }
    });
}

async fn run_once(pool: &PgPool) -> Result<(), sqlx::Error> {
    let queued_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM payouts WHERE state = 'queued'")
            .fetch_one(pool)
            .await?;

    if queued_count > 0 {
        tracing::warn!(
            queued_count,
            "queued payouts exist but no Stellar signer implementation is configured"
        );
    }

    Ok(())
}
