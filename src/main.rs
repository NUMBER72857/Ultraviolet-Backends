mod config;
mod db;
mod error;
mod models;
mod money;

use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::get,
    Json, Router,
};
use chrono::{DateTime, Utc};
use config::Config;
use error::ApiError;
use models::InvoiceRecord;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use tower_http::trace::TraceLayer;
use uuid::Uuid;

#[derive(Clone)]
struct AppState {
    config: Config,
    pool: PgPool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let config = Config::from_env().map_err(|message| format!("configuration error: {message}"))?;
    let pool = db::connect(&config.database_url).await?;
    let listener = tokio::net::TcpListener::bind(config.bind_addr).await?;

    let app = app(AppState { config, pool });
    tracing::info!(addr = %listener.local_addr()?, "ultraviolet backend listening");
    axum::serve(listener, app).await?;
    Ok(())
}

fn app(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
        .route("/v1/invoices", get(list_invoices).post(create_invoice))
        .route("/v1/invoices/:id", get(get_invoice))
        .route("/v1/public/invoices/:public_id", get(get_public_invoice))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        service: "ultraviolet-backend",
        status: "ok",
    })
}

async fn ready(State(state): State<AppState>) -> Result<Json<ReadyResponse>, ApiError> {
    sqlx::query_scalar::<_, i64>("SELECT 1")
        .fetch_one(&state.pool)
        .await?;

    Ok(Json(ReadyResponse {
        database: "ok",
        stellar_network_passphrase: state.config.stellar_network_passphrase,
        stellar_horizon_url: state.config.stellar_horizon_url,
        platform_fee_bps: state.config.platform_fee_bps,
        session_secret_configured: state.config.session_secret.len() >= 32,
    }))
}

async fn list_invoices(
    State(state): State<AppState>,
    Query(query): Query<ListInvoicesQuery>,
) -> Result<Json<Vec<InvoiceRecord>>, ApiError> {
    let invoices = sqlx::query_as::<_, InvoiceRecord>(INVOICE_SELECT_BY_MERCHANT)
        .bind(query.merchant_id)
        .fetch_all(&state.pool)
        .await?;

    Ok(Json(invoices))
}

async fn get_invoice(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<InvoiceRecord>, ApiError> {
    let invoice = sqlx::query_as::<_, InvoiceRecord>(INVOICE_SELECT_BY_ID)
        .bind(id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(ApiError::NotFound("invoice not found"))?;

    Ok(Json(invoice))
}

async fn get_public_invoice(
    State(state): State<AppState>,
    Path(public_id): Path<String>,
) -> Result<Json<InvoiceRecord>, ApiError> {
    let invoice = sqlx::query_as::<_, InvoiceRecord>(INVOICE_SELECT_BY_PUBLIC_ID)
        .bind(public_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(ApiError::NotFound("invoice not found"))?;

    Ok(Json(invoice))
}

async fn create_invoice(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateInvoiceRequest>,
) -> Result<Json<InvoiceRecord>, ApiError> {
    money::validate_fee_split(
        payload.gross_amount_atomic,
        payload.platform_fee_atomic,
        payload.merchant_net_atomic,
    )
    .map_err(ApiError::BadRequest)?;

    if payload.expires_at <= Utc::now() {
        return Err(ApiError::BadRequest("expires_at must be in the future"));
    }

    let idempotency_key = read_idempotency_key(&headers)?;
    let request_hash = request_hash(&payload)?;
    let scope = format!("invoice:create:{}", payload.merchant_id);
    let mut tx = state.pool.begin().await?;

    let inserted_key = sqlx::query(
        r#"
        INSERT INTO idempotency_keys (scope, key, method, path, request_hash, locked_at)
        VALUES ($1, $2, 'POST', '/v1/invoices', $3, now())
        ON CONFLICT (scope, key) DO NOTHING
        "#,
    )
    .bind(&scope)
    .bind(&idempotency_key)
    .bind(&request_hash)
    .execute(&mut *tx)
    .await?;

    if inserted_key.rows_affected() == 0 {
        let existing = sqlx::query_as::<_, IdempotencyRecord>(
            r#"
            SELECT request_hash, response_reference
            FROM idempotency_keys
            WHERE scope = $1 AND key = $2
            "#,
        )
        .bind(&scope)
        .bind(&idempotency_key)
        .fetch_one(&mut *tx)
        .await?;

        if existing.request_hash != request_hash {
            return Err(ApiError::Conflict(
                "idempotency key reused with a different request",
            ));
        }

        if let Some(invoice_id) = existing.response_reference {
            let invoice = sqlx::query_as::<_, InvoiceRecord>(INVOICE_SELECT_BY_ID)
                .bind(invoice_id)
                .fetch_one(&mut *tx)
                .await?;
            tx.commit().await?;
            return Ok(Json(invoice));
        }

        return Err(ApiError::Conflict("idempotency key is already in progress"));
    }

    let invoice_id = prefixed_id("inv");
    let public_id = prefixed_id("pay");
    let payment_memo = memo_text();

    let invoice = sqlx::query_as::<_, InvoiceRecord>(
        r#"
        INSERT INTO invoices (
          id, merchant_id, public_id, invoice_number, customer_email, description, state,
          gross_amount_atomic, platform_fee_atomic, merchant_net_atomic,
          asset_code, asset_issuer, network_passphrase, treasury_account,
          payment_memo, expires_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, 'pending', $7, $8, $9, $10, $11, $12, $13, $14, $15)
        RETURNING
          id, merchant_id, public_id, invoice_number, customer_email, description, state,
          gross_amount_atomic, platform_fee_atomic, merchant_net_atomic,
          asset_code, asset_issuer, network_passphrase, treasury_account,
          payment_memo, expires_at, paid_at, settled_at, created_at, updated_at
        "#,
    )
    .bind(&invoice_id)
    .bind(&payload.merchant_id)
    .bind(&public_id)
    .bind(&payload.invoice_number)
    .bind(&payload.customer_email)
    .bind(&payload.description)
    .bind(payload.gross_amount_atomic)
    .bind(payload.platform_fee_atomic)
    .bind(payload.merchant_net_atomic)
    .bind(&state.config.stellar_usdc_asset_code)
    .bind(&state.config.stellar_usdc_asset_issuer)
    .bind(&state.config.stellar_network_passphrase)
    .bind(&state.config.stellar_treasury_account)
    .bind(&payment_memo)
    .bind(payload.expires_at)
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query(
        r#"
        UPDATE idempotency_keys
        SET response_status = 201, response_reference = $3, completed_at = now()
        WHERE scope = $1 AND key = $2
        "#,
    )
    .bind(&scope)
    .bind(&idempotency_key)
    .bind(&invoice.id)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO audit_logs (id, merchant_id, action, entity_type, entity_id, metadata)
        VALUES ($1, $2, 'invoice_created', 'invoice', $3, $4)
        "#,
    )
    .bind(prefixed_id("aud"))
    .bind(&payload.merchant_id)
    .bind(&invoice.id)
    .bind(serde_json::json!({
        "gross_amount_atomic": invoice.gross_amount_atomic,
        "platform_fee_atomic": invoice.platform_fee_atomic,
        "merchant_net_atomic": invoice.merchant_net_atomic
    }))
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(Json(invoice))
}

fn read_idempotency_key(headers: &HeaderMap) -> Result<String, ApiError> {
    let value = headers
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or(ApiError::BadRequest("Idempotency-Key header is required"))?;

    if value.len() > 120 {
        return Err(ApiError::BadRequest("Idempotency-Key header is too long"));
    }

    Ok(value.to_string())
}

fn request_hash(payload: &CreateInvoiceRequest) -> Result<String, ApiError> {
    let body =
        serde_json::to_vec(payload).map_err(|_| ApiError::Internal("could not hash request"))?;
    let mut hasher = Sha256::new();
    hasher.update(body);
    Ok(hex::encode(hasher.finalize()))
}

fn prefixed_id(prefix: &str) -> String {
    format!("{prefix}_{}", Uuid::new_v4().simple())
}

fn memo_text() -> String {
    let raw = Uuid::new_v4().simple().to_string();
    format!("UV{}", raw.chars().take(24).collect::<String>())
}

const INVOICE_SELECT_BY_ID: &str = r#"
SELECT
  id, merchant_id, public_id, invoice_number, customer_email, description, state,
  gross_amount_atomic, platform_fee_atomic, merchant_net_atomic,
  asset_code, asset_issuer, network_passphrase, treasury_account,
  payment_memo, expires_at, paid_at, settled_at, created_at, updated_at
FROM invoices
WHERE id = $1
"#;

const INVOICE_SELECT_BY_PUBLIC_ID: &str = r#"
SELECT
  id, merchant_id, public_id, invoice_number, customer_email, description, state,
  gross_amount_atomic, platform_fee_atomic, merchant_net_atomic,
  asset_code, asset_issuer, network_passphrase, treasury_account,
  payment_memo, expires_at, paid_at, settled_at, created_at, updated_at
FROM invoices
WHERE public_id = $1
"#;

const INVOICE_SELECT_BY_MERCHANT: &str = r#"
SELECT
  id, merchant_id, public_id, invoice_number, customer_email, description, state,
  gross_amount_atomic, platform_fee_atomic, merchant_net_atomic,
  asset_code, asset_issuer, network_passphrase, treasury_account,
  payment_memo, expires_at, paid_at, settled_at, created_at, updated_at
FROM invoices
WHERE merchant_id = $1
ORDER BY created_at DESC
LIMIT 100
"#;

#[derive(Deserialize)]
struct ListInvoicesQuery {
    merchant_id: String,
}

#[derive(Deserialize, Serialize)]
struct CreateInvoiceRequest {
    merchant_id: String,
    invoice_number: String,
    customer_email: Option<String>,
    description: String,
    gross_amount_atomic: i64,
    platform_fee_atomic: i64,
    merchant_net_atomic: i64,
    expires_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
struct IdempotencyRecord {
    request_hash: String,
    response_reference: Option<String>,
}

#[derive(Serialize)]
struct HealthResponse {
    service: &'static str,
    status: &'static str,
}

#[derive(Serialize)]
struct ReadyResponse {
    database: &'static str,
    stellar_network_passphrase: String,
    stellar_horizon_url: String,
    platform_fee_bps: u16,
    session_secret_configured: bool,
}

#[cfg(test)]
mod tests {
    use super::{memo_text, prefixed_id};

    #[test]
    fn memo_fits_stellar_text_limit() {
        assert!(memo_text().len() <= 28);
    }

    #[test]
    fn generated_ids_are_prefixed() {
        assert!(prefixed_id("inv").starts_with("inv_"));
    }
}
