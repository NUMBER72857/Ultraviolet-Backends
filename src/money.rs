//! Exact-money helpers for Stellar USDC atomic units.
//!
//! All payment amounts are integer atomic units with seven decimals; this file
//! prevents floating-point drift and validates invoice fee splits before writes.

#[allow(dead_code)]
pub const STELLAR_ATOMIC_SCALE: i64 = 10_000_000;

#[allow(dead_code)]
pub fn parse_usdc_atomic(input: &str) -> Result<i64, &'static str> {
    let trimmed = input.trim();
    if trimmed.is_empty() || trimmed.starts_with('-') {
        return Err("amount must be positive");
    }

    let mut parts = trimmed.split('.');
    let whole = parts.next().ok_or("amount is invalid")?;
    let fractional = parts.next().unwrap_or("");
    if parts.next().is_some() || fractional.len() > 7 {
        return Err("amount supports at most 7 decimal places");
    }
    if !whole.chars().all(|c| c.is_ascii_digit()) || !fractional.chars().all(|c| c.is_ascii_digit())
    {
        return Err("amount must be numeric");
    }

    let whole_atomic = whole
        .parse::<i64>()
        .map_err(|_| "amount is too large")?
        .checked_mul(STELLAR_ATOMIC_SCALE)
        .ok_or("amount is too large")?;
    let padded_fraction = format!("{fractional:0<7}");
    let fractional_atomic = padded_fraction
        .parse::<i64>()
        .map_err(|_| "amount is invalid")?;

    whole_atomic
        .checked_add(fractional_atomic)
        .filter(|value| *value > 0)
        .ok_or("amount must be positive")
}

pub fn validate_fee_split(
    gross_amount_atomic: i64,
    platform_fee_atomic: i64,
    merchant_net_atomic: i64,
) -> Result<(), &'static str> {
    if gross_amount_atomic <= 0 || platform_fee_atomic < 0 || merchant_net_atomic < 0 {
        return Err("amounts must be non-negative and gross must be positive");
    }

    if gross_amount_atomic != platform_fee_atomic + merchant_net_atomic {
        return Err("gross_amount_atomic must equal platform_fee_atomic plus merchant_net_atomic");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{parse_usdc_atomic, validate_fee_split};

    #[test]
    fn parses_seven_decimal_usdc() {
        assert_eq!(parse_usdc_atomic("12.3456789").unwrap(), 123_456_789);
        assert_eq!(parse_usdc_atomic("1").unwrap(), 10_000_000);
        assert_eq!(parse_usdc_atomic("0.0000001").unwrap(), 1);
    }

    #[test]
    fn rejects_too_much_precision() {
        assert!(parse_usdc_atomic("1.00000001").is_err());
    }

    #[test]
    fn enforces_fee_split() {
        assert!(validate_fee_split(10_000_000, 100_000, 9_900_000).is_ok());
        assert!(validate_fee_split(10_000_000, 100_000, 9_800_000).is_err());
    }
}
