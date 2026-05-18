# NUMBER / Ultraviolet Backends

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
  config.rs             # startup env validation
  db.rs                 # PostgreSQL pool
  error.rs              # HTTP error boundary
  main.rs               # Axum API
  models.rs             # API/database projections
  money.rs              # atomic USDC helpers
migrations/
  001_core_payment_schema.sql
```

Planned next modules:

```text
src/
  auth/
  invoices/
  ledger/
  reconciliation/
  payouts/
  stellar/
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
STELLAR_NETWORK_PASSPHRASE="Test SDF Network ; September 2015"
STELLAR_HORIZON_URL=https://horizon-testnet.stellar.org
STELLAR_USDC_ASSET_CODE=USDC
STELLAR_USDC_ASSET_ISSUER=G...
STELLAR_TREASURY_ACCOUNT=G...
PLATFORM_FEE_BPS=100
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
GET  /v1/invoices?merchant_id=...
POST /v1/invoices
GET  /v1/invoices/:id
GET  /v1/public/invoices/:public_id
```

## Verification

```bash
cargo fmt --check
cargo check
cargo test
```

## License

Open source under the MIT License. See [LICENSE](./LICENSE).

## Production Gaps

Do not deploy this backend to production until these are done:

- Merchant auth/session routes.
- Full Stellar payment verification: network, success, hash uniqueness, destination, asset, amount, memo, fee-bump handling, muxed-account policy, and multi-operation ambiguity.
- Reconciliation workers with persisted cursors, retry policy, expiry policy, and no downgrade from verified.
- Payout submission worker with signer isolation.
- Payout reconciliation with transaction success, destination, asset, amount, memo, and hash uniqueness checks.
- Request size limits, rate limits, CORS policy, structured audit logs, and PII-safe tracing.
- Testnet end-to-end suite for valid payment, wrong destination, wrong amount, wrong asset, wrong memo, failed transaction, duplicate hash, unknown hash, Horizon/RPC outage, and payout failure.
