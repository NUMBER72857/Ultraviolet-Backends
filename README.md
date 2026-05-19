# NUMBER / Ultraviolet Backends

<!--
README for the Rust backend: explains the service boundaries, local setup,
runtime controls, and remaining production risks so operators do not confuse
testnet readiness with production readiness.
-->

Rust backend for Ultraviolet education operations and Stellar USDC payment truth.

This repo is intentionally boring. It is not a token demo, not an NFT credential product, and not a Soroban-first experiment. The backend exists to make money-state transitions auditable, idempotent, and recoverable when browsers close, wallets misbehave, or external APIs fail.

## Responsibilities

- Merchant and admin API boundary.
- PostgreSQL source of truth.
- Raw SQL migrations.
- Invoice creation with idempotency.
- Exact USDC atomic-unit accounting.
- Payment, payout, ledger, audit, and reconciliation persistence.
- Server-side Stellar verification and payout worker boundary.
- Future scraping and AI enrichment boundaries for education operations.

## Non-Negotiables

- PostgreSQL owns truth.
- Amounts are stored as integer Stellar atomic units with 7 decimals.
- `gross_amount_atomic = platform_fee_atomic + merchant_net_atomic`.
- Browser events, wallet callbacks, and submitted transaction hashes never mark invoices paid.
- Only backend reconciliation can move an invoice to `paid`.
- Payout state is separate from invoice state.
- Posted ledger transactions must balance to zero per asset.
- Money-state changes must write audit logs.
- Student PII stays off-chain.

## Architecture

```text
src/
  bin/
    migrate.rs          # raw SQL migration runner
  auth.rs               # merchant login and bearer session enforcement
  config.rs             # startup env validation
  db.rs                 # PostgreSQL pool
  error.rs              # HTTP error boundary
  http.rs               # CORS, request size limits, and rate limiting
  main.rs               # Axum API
  models.rs             # API/database projections
  money.rs              # atomic USDC helpers
  payouts.rs            # payout worker boundary
  reconciliation.rs     # reconciliation worker boundary
  stellar.rs            # backend-owned Stellar verification rules
migrations/
  001_core_payment_schema.sql
docs/
  stellar-payment-operations.md
```

Planned next modules:

```text
src/
  auth/
  invoices/
  ledger/
  payouts/
  scraping/
  ai/
```

Scraping belongs in isolated workers that write reviewable operational metadata. AI belongs in advisory admin workflows only. Neither scraping nor AI is allowed to decide payment truth.

## Environment

Copy `.env.example` to `.env` and replace every placeholder:

```bash
APP_ENV=development
BIND_ADDR=127.0.0.1:8080
DATABASE_URL=postgres://ultraviolet:ultraviolet@127.0.0.1:5432/ultraviolet
SESSION_SECRET=replace-with-at-least-32-random-bytes
SESSION_TTL_HOURS=12
CORS_ALLOWED_ORIGIN=*
MAX_JSON_BODY_BYTES=65536
RATE_LIMIT_PER_MINUTE=120
STELLAR_NETWORK_PASSPHRASE="Test SDF Network ; September 2015"
STELLAR_HORIZON_URL=https://horizon-testnet.stellar.org
STELLAR_USDC_ASSET_CODE=USDC
STELLAR_USDC_ASSET_ISSUER=G...
STELLAR_TREASURY_ACCOUNT=G...
PLATFORM_FEE_BPS=100
RECONCILIATION_WORKER_ENABLED=false
RECONCILIATION_INTERVAL_SECONDS=30
PAYOUT_WORKER_ENABLED=false
PAYOUT_INTERVAL_SECONDS=30
```

Production must not use placeholder Stellar accounts, weak session secrets, or testnet issuer values.

## Local Development

Start PostgreSQL from the ops repo:

```bash
cd ../ultraviolet-ops
docker compose --env-file .env.example -f deploy/docker-compose.yml up -d
```

Run migrations:

```bash
cd ../ultraviolet-backend
DATABASE_URL=postgres://ultraviolet:ultraviolet@127.0.0.1:5432/ultraviolet \
  cargo run --bin ultraviolet-migrate
```

Run the API:

```bash
cargo run --bin ultraviolet-backend
```

Health checks:

```text
GET /health
GET /ready
```

Invoice API currently exposed:

```text
POST /v1/auth/login
POST /v1/auth/logout
GET  /v1/invoices
POST /v1/invoices
GET  /v1/invoices/:id
POST /v1/invoices/:id/payment-attempts
GET  /v1/public/invoices/:public_id
```

## Verification

```bash
cargo fmt --check
cargo check
cargo test
```

Payment verification and operating controls are documented in
[`docs/stellar-payment-operations.md`](docs/stellar-payment-operations.md).

## License

Open source under the MIT License. See [LICENSE](./LICENSE).

## Production Gaps

Do not deploy this backend to production until these are done:

- Seed/admin tooling for creating merchants and PBKDF2 password hashes.
- Real Horizon/RPC reconciliation client with cursors, retry policy, fee-bump handling, muxed-account policy, and multi-operation ambiguity handling.
- Payout submission worker with signer isolation and transaction construction.
- Payout reconciliation with transaction success, destination, asset, amount, memo, and hash uniqueness checks.
- Testnet end-to-end suite for valid payment, wrong destination, wrong amount, wrong asset, wrong memo, failed transaction, duplicate hash, unknown hash, Horizon/RPC outage, and payout failure.
