use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize, Serializer};
use sqlx::FromRow;

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InvoiceState {
    Pending,
    Paid,
    Expired,
    Settled,
    Failed,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PayoutState {
    Queued,
    Submitted,
    Settled,
    Failed,
    DeadLettered,
}

#[derive(Debug, FromRow, Serialize)]
pub struct InvoiceRecord {
    pub id: String,
    pub merchant_id: String,
    pub public_id: String,
    pub invoice_number: String,
    pub customer_email: Option<String>,
    pub description: String,
    pub state: String,
    #[serde(serialize_with = "serialize_i64_as_string")]
    pub gross_amount_atomic: i64,
    #[serde(serialize_with = "serialize_i64_as_string")]
    pub platform_fee_atomic: i64,
    #[serde(serialize_with = "serialize_i64_as_string")]
    pub merchant_net_atomic: i64,
    pub asset_code: String,
    pub asset_issuer: String,
    pub network_passphrase: String,
    pub treasury_account: String,
    pub payment_memo: String,
    pub expires_at: DateTime<Utc>,
    pub paid_at: Option<DateTime<Utc>>,
    pub settled_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

fn serialize_i64_as_string<S>(value: &i64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&value.to_string())
}
