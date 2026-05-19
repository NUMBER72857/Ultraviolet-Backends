//! Startup configuration and environment validation.
//!
//! This module fails fast on missing money, auth, HTTP, and worker settings so
//! deployment errors are caught before the service accepts requests.

use std::{env, net::SocketAddr};

#[derive(Clone, Debug)]
pub struct Config {
    pub bind_addr: SocketAddr,
    pub database_url: String,
    pub session_secret: String,
    pub stellar_network_passphrase: String,
    pub stellar_horizon_url: String,
    pub stellar_usdc_asset_code: String,
    pub stellar_usdc_asset_issuer: String,
    pub stellar_treasury_account: String,
    pub platform_fee_bps: u16,
    pub cors_allowed_origin: String,
    pub max_json_body_bytes: usize,
    pub rate_limit_per_minute: u32,
    pub session_ttl_hours: i64,
    pub reconciliation_worker_enabled: bool,
    pub reconciliation_interval_seconds: u64,
    pub payout_worker_enabled: bool,
    pub payout_interval_seconds: u64,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        Self::from_lookup(|key| env::var(key).ok())
    }

    fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Result<Self, String> {
        let bind_addr = get("BIND_ADDR")
            .unwrap_or_else(|| "127.0.0.1:8080".to_string())
            .parse::<SocketAddr>()
            .map_err(|_| "BIND_ADDR must be host:port".to_string())?;

        let database_url = required(&get, "DATABASE_URL")?;
        if !database_url.starts_with("postgres://") && !database_url.starts_with("postgresql://") {
            return Err("DATABASE_URL must be PostgreSQL".to_string());
        }

        let session_secret = required(&get, "SESSION_SECRET")?;
        if session_secret.len() < 32 {
            return Err("SESSION_SECRET must be at least 32 bytes".to_string());
        }

        let stellar_network_passphrase = required(&get, "STELLAR_NETWORK_PASSPHRASE")?;
        let stellar_horizon_url = required(&get, "STELLAR_HORIZON_URL")?;
        let stellar_usdc_asset_code = required(&get, "STELLAR_USDC_ASSET_CODE")?;
        if stellar_usdc_asset_code != "USDC" {
            return Err("STELLAR_USDC_ASSET_CODE must be USDC for this MVP".to_string());
        }

        let stellar_usdc_asset_issuer = required(&get, "STELLAR_USDC_ASSET_ISSUER")?;
        validate_stellar_account("STELLAR_USDC_ASSET_ISSUER", &stellar_usdc_asset_issuer)?;

        let stellar_treasury_account = required(&get, "STELLAR_TREASURY_ACCOUNT")?;
        validate_stellar_account("STELLAR_TREASURY_ACCOUNT", &stellar_treasury_account)?;

        let platform_fee_bps = get("PLATFORM_FEE_BPS")
            .unwrap_or_else(|| "0".to_string())
            .parse::<u16>()
            .map_err(|_| "PLATFORM_FEE_BPS must be an integer".to_string())?;
        if platform_fee_bps > 10_000 {
            return Err("PLATFORM_FEE_BPS cannot exceed 10000".to_string());
        }

        let cors_allowed_origin = get("CORS_ALLOWED_ORIGIN").unwrap_or_else(|| "*".to_string());
        let max_json_body_bytes = parse_usize(&get, "MAX_JSON_BODY_BYTES", 65_536)?;
        let rate_limit_per_minute = parse_u32(&get, "RATE_LIMIT_PER_MINUTE", 120)?;
        if rate_limit_per_minute == 0 {
            return Err("RATE_LIMIT_PER_MINUTE must be at least 1".to_string());
        }

        let session_ttl_hours = parse_i64(&get, "SESSION_TTL_HOURS", 12)?;
        if !(1..=720).contains(&session_ttl_hours) {
            return Err("SESSION_TTL_HOURS must be between 1 and 720".to_string());
        }

        let reconciliation_worker_enabled =
            parse_bool(&get, "RECONCILIATION_WORKER_ENABLED", false)?;
        let reconciliation_interval_seconds =
            parse_u64(&get, "RECONCILIATION_INTERVAL_SECONDS", 30)?;
        let payout_worker_enabled = parse_bool(&get, "PAYOUT_WORKER_ENABLED", false)?;
        let payout_interval_seconds = parse_u64(&get, "PAYOUT_INTERVAL_SECONDS", 30)?;

        Ok(Self {
            bind_addr,
            database_url,
            session_secret,
            stellar_network_passphrase,
            stellar_horizon_url,
            stellar_usdc_asset_code,
            stellar_usdc_asset_issuer,
            stellar_treasury_account,
            platform_fee_bps,
            cors_allowed_origin,
            max_json_body_bytes,
            rate_limit_per_minute,
            session_ttl_hours,
            reconciliation_worker_enabled,
            reconciliation_interval_seconds,
            payout_worker_enabled,
            payout_interval_seconds,
        })
    }
}

fn required(get: &impl Fn(&str) -> Option<String>, key: &str) -> Result<String, String> {
    get(key)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty() && !value.contains("replace-with"))
        .ok_or_else(|| format!("{key} is required"))
}

fn validate_stellar_account(key: &str, value: &str) -> Result<(), String> {
    if value.len() == 56
        && value.starts_with('G')
        && value.chars().all(|c| c.is_ascii_alphanumeric())
    {
        return Ok(());
    }

    Err(format!("{key} must be a Stellar public account id"))
}

fn parse_bool(
    get: &impl Fn(&str) -> Option<String>,
    key: &str,
    default: bool,
) -> Result<bool, String> {
    match get(key).as_deref().map(str::trim) {
        None | Some("") => Ok(default),
        Some("true") | Some("1") | Some("yes") => Ok(true),
        Some("false") | Some("0") | Some("no") => Ok(false),
        Some(_) => Err(format!("{key} must be true or false")),
    }
}

fn parse_i64(
    get: &impl Fn(&str) -> Option<String>,
    key: &str,
    default: i64,
) -> Result<i64, String> {
    get(key)
        .unwrap_or_else(|| default.to_string())
        .parse::<i64>()
        .map_err(|_| format!("{key} must be an integer"))
}

fn parse_u32(
    get: &impl Fn(&str) -> Option<String>,
    key: &str,
    default: u32,
) -> Result<u32, String> {
    get(key)
        .unwrap_or_else(|| default.to_string())
        .parse::<u32>()
        .map_err(|_| format!("{key} must be an integer"))
}

fn parse_u64(
    get: &impl Fn(&str) -> Option<String>,
    key: &str,
    default: u64,
) -> Result<u64, String> {
    get(key)
        .unwrap_or_else(|| default.to_string())
        .parse::<u64>()
        .map_err(|_| format!("{key} must be an integer"))
}

fn parse_usize(
    get: &impl Fn(&str) -> Option<String>,
    key: &str,
    default: usize,
) -> Result<usize, String> {
    get(key)
        .unwrap_or_else(|| default.to_string())
        .parse::<usize>()
        .map_err(|_| format!("{key} must be an integer"))
}

#[cfg(test)]
mod tests {
    use super::Config;
    use std::collections::HashMap;

    fn valid_env() -> HashMap<&'static str, String> {
        HashMap::from([
            (
                "DATABASE_URL",
                "postgres://user:pass@localhost:5432/uv".to_string(),
            ),
            (
                "SESSION_SECRET",
                "12345678901234567890123456789012".to_string(),
            ),
            (
                "STELLAR_NETWORK_PASSPHRASE",
                "Test SDF Network ; September 2015".to_string(),
            ),
            (
                "STELLAR_HORIZON_URL",
                "https://horizon-testnet.stellar.org".to_string(),
            ),
            ("STELLAR_USDC_ASSET_CODE", "USDC".to_string()),
            (
                "STELLAR_USDC_ASSET_ISSUER",
                "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_string(),
            ),
            (
                "STELLAR_TREASURY_ACCOUNT",
                "GBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB".to_string(),
            ),
            ("PLATFORM_FEE_BPS", "100".to_string()),
        ])
    }

    #[test]
    fn accepts_valid_config() {
        let env = valid_env();
        let config = Config::from_lookup(|key| env.get(key).cloned()).expect("valid config");
        assert_eq!(config.platform_fee_bps, 100);
        assert_eq!(config.session_ttl_hours, 12);
    }

    #[test]
    fn rejects_non_postgres_database() {
        let mut env = valid_env();
        env.insert("DATABASE_URL", "sqlite://local.db".to_string());
        let error = Config::from_lookup(|key| env.get(key).cloned()).expect_err("invalid db");
        assert!(error.contains("PostgreSQL"));
    }
}
