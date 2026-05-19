-- Core payment, auth, ledger, reconciliation, and payout schema.
-- This migration defines the PostgreSQL source of truth for merchant sessions,
-- invoices, Stellar payment observations, payout state, and immutable ledger
-- entries so browser callbacks never decide money truth.

CREATE TABLE IF NOT EXISTS schema_migrations (
  filename text PRIMARY KEY,
  applied_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS fee_plans (
  id text PRIMARY KEY,
  name text NOT NULL UNIQUE,
  platform_fee_bps integer NOT NULL CHECK (platform_fee_bps >= 0 AND platform_fee_bps <= 10000),
  platform_fee_fixed_atomic bigint NOT NULL DEFAULT 0 CHECK (platform_fee_fixed_atomic >= 0),
  status text NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'disabled')),
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now()
);

INSERT INTO fee_plans (id, name, platform_fee_bps, platform_fee_fixed_atomic)
VALUES ('fee_default', 'Default', 100, 0)
ON CONFLICT (id) DO NOTHING;

CREATE TABLE IF NOT EXISTS merchants (
  id text PRIMARY KEY,
  legal_name text NOT NULL,
  display_name text NOT NULL,
  status text NOT NULL DEFAULT 'pending_review' CHECK (status IN ('pending_review', 'active', 'suspended', 'closed')),
  fee_plan_id text NOT NULL DEFAULT 'fee_default' REFERENCES fee_plans(id),
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS merchant_users (
  id text PRIMARY KEY,
  merchant_id text NOT NULL REFERENCES merchants(id) ON DELETE CASCADE,
  email text NOT NULL UNIQUE,
  password_hash text NOT NULL,
  role text NOT NULL CHECK (role IN ('owner', 'admin', 'viewer')),
  status text NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'disabled')),
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS merchant_sessions (
  id text PRIMARY KEY,
  merchant_user_id text NOT NULL REFERENCES merchant_users(id) ON DELETE CASCADE,
  token_hash text NOT NULL UNIQUE,
  expires_at timestamptz NOT NULL,
  revoked_at timestamptz,
  last_seen_at timestamptz,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS merchant_stellar_accounts (
  id text PRIMARY KEY,
  merchant_id text NOT NULL REFERENCES merchants(id) ON DELETE RESTRICT,
  stellar_account text NOT NULL,
  memo_required boolean NOT NULL DEFAULT false,
  memo_type text CHECK (memo_type IN ('text', 'id', 'hash', 'return')),
  memo_value text,
  status text NOT NULL DEFAULT 'pending_verification' CHECK (status IN ('pending_verification', 'active', 'disabled')),
  verified_at timestamptz,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now(),
  CHECK (
    (memo_required = false AND memo_type IS NULL AND memo_value IS NULL)
    OR
    (memo_required = true AND memo_type IS NOT NULL AND memo_value IS NOT NULL)
  )
);

CREATE UNIQUE INDEX IF NOT EXISTS merchant_stellar_accounts_one_active_idx
  ON merchant_stellar_accounts(merchant_id)
  WHERE status = 'active';

CREATE TABLE IF NOT EXISTS invoices (
  id text PRIMARY KEY,
  merchant_id text NOT NULL REFERENCES merchants(id) ON DELETE RESTRICT,
  public_id text NOT NULL UNIQUE,
  invoice_number text NOT NULL,
  customer_email text,
  description text NOT NULL,
  state text NOT NULL DEFAULT 'pending' CHECK (state IN ('pending', 'paid', 'expired', 'settled', 'failed')),
  gross_amount_atomic bigint NOT NULL CHECK (gross_amount_atomic > 0),
  platform_fee_atomic bigint NOT NULL CHECK (platform_fee_atomic >= 0),
  merchant_net_atomic bigint NOT NULL CHECK (merchant_net_atomic >= 0),
  asset_code text NOT NULL,
  asset_issuer text NOT NULL,
  network_passphrase text NOT NULL,
  treasury_account text NOT NULL,
  payment_memo text NOT NULL UNIQUE,
  expires_at timestamptz NOT NULL,
  paid_at timestamptz,
  settled_at timestamptz,
  metadata_json jsonb NOT NULL DEFAULT '{}'::jsonb,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now(),
  UNIQUE (merchant_id, invoice_number),
  CHECK (gross_amount_atomic = platform_fee_atomic + merchant_net_atomic)
);

CREATE TABLE IF NOT EXISTS payment_attempts (
  id text PRIMARY KEY,
  invoice_id text NOT NULL REFERENCES invoices(id) ON DELETE RESTRICT,
  transaction_hash text,
  source_account text,
  status text NOT NULL CHECK (status IN ('submitted', 'failed')),
  message text NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX IF NOT EXISTS payment_attempts_transaction_hash_unique
  ON payment_attempts(transaction_hash)
  WHERE transaction_hash IS NOT NULL;

CREATE TABLE IF NOT EXISTS payments (
  id text PRIMARY KEY,
  invoice_id text NOT NULL REFERENCES invoices(id) ON DELETE RESTRICT,
  state text NOT NULL CHECK (state IN ('candidate', 'verified', 'rejected')),
  stellar_transaction_hash text NOT NULL UNIQUE,
  ledger_sequence bigint,
  source_account text,
  destination_account text NOT NULL,
  amount_atomic bigint NOT NULL CHECK (amount_atomic > 0),
  asset_code text NOT NULL,
  asset_issuer text NOT NULL,
  memo text NOT NULL,
  rejection_reason text,
  observed_at timestamptz NOT NULL DEFAULT now(),
  verified_at timestamptz
);

CREATE TABLE IF NOT EXISTS payouts (
  id text PRIMARY KEY,
  merchant_id text NOT NULL REFERENCES merchants(id) ON DELETE RESTRICT,
  invoice_id text NOT NULL UNIQUE REFERENCES invoices(id) ON DELETE RESTRICT,
  state text NOT NULL DEFAULT 'queued' CHECK (state IN ('queued', 'submitted', 'settled', 'failed', 'dead_lettered')),
  amount_atomic bigint NOT NULL CHECK (amount_atomic > 0),
  asset_code text NOT NULL,
  asset_issuer text NOT NULL,
  destination_account text NOT NULL,
  destination_memo_type text,
  destination_memo_value text,
  submitted_transaction_hash text UNIQUE,
  queued_at timestamptz NOT NULL DEFAULT now(),
  submitted_at timestamptz,
  settled_at timestamptz,
  failure_reason text,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS payout_attempts (
  id text PRIMARY KEY,
  payout_id text NOT NULL REFERENCES payouts(id) ON DELETE RESTRICT,
  signed_envelope_xdr text,
  stellar_transaction_hash text UNIQUE,
  status text NOT NULL CHECK (status IN ('created', 'submitted', 'settled', 'failed')),
  message text NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS idempotency_keys (
  scope text NOT NULL,
  key text NOT NULL,
  method text NOT NULL,
  path text NOT NULL,
  request_hash text NOT NULL,
  response_status integer,
  response_reference text,
  locked_at timestamptz NOT NULL DEFAULT now(),
  completed_at timestamptz,
  created_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (scope, key)
);

CREATE TABLE IF NOT EXISTS stellar_reconciliation_cursors (
  id text PRIMARY KEY,
  network_passphrase text NOT NULL,
  account_id text NOT NULL,
  cursor text NOT NULL,
  last_ledger_sequence bigint,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now(),
  UNIQUE (network_passphrase, account_id)
);

CREATE TABLE IF NOT EXISTS stellar_transactions_seen (
  transaction_hash text PRIMARY KEY,
  network_passphrase text NOT NULL,
  ledger_sequence bigint,
  source_account text,
  memo text,
  successful boolean NOT NULL,
  first_seen_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS ledger_accounts (
  id text PRIMARY KEY,
  code text NOT NULL UNIQUE,
  normal_balance text NOT NULL CHECK (normal_balance IN ('debit', 'credit')),
  created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS ledger_transactions (
  id text PRIMARY KEY,
  source_type text NOT NULL,
  source_id text NOT NULL,
  description text NOT NULL,
  status text NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'posted')),
  metadata jsonb NOT NULL DEFAULT '{}'::jsonb,
  posted_at timestamptz,
  created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS ledger_entries (
  id text PRIMARY KEY,
  ledger_transaction_id text NOT NULL REFERENCES ledger_transactions(id) ON DELETE RESTRICT,
  ledger_account_id text NOT NULL REFERENCES ledger_accounts(id) ON DELETE RESTRICT,
  asset_code text NOT NULL,
  amount_atomic bigint NOT NULL CHECK (amount_atomic > 0),
  direction text NOT NULL CHECK (direction IN ('debit', 'credit')),
  created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS ledger_events (
  id text PRIMARY KEY,
  ledger_transaction_id text REFERENCES ledger_transactions(id) ON DELETE RESTRICT,
  event_type text NOT NULL,
  entity_type text NOT NULL,
  entity_id text NOT NULL,
  metadata jsonb NOT NULL DEFAULT '{}'::jsonb,
  created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS audit_logs (
  id text PRIMARY KEY,
  merchant_id text REFERENCES merchants(id) ON DELETE SET NULL,
  actor_user_id text REFERENCES merchant_users(id) ON DELETE SET NULL,
  action text NOT NULL,
  entity_type text NOT NULL,
  entity_id text,
  metadata jsonb NOT NULL DEFAULT '{}'::jsonb,
  created_at timestamptz NOT NULL DEFAULT now()
);

CREATE OR REPLACE FUNCTION post_ledger_transaction(p_transaction_id text)
RETURNS void
LANGUAGE plpgsql
AS $$
DECLARE
  unbalanced_count integer;
BEGIN
  SELECT count(*)
  INTO unbalanced_count
  FROM (
    SELECT asset_code,
      sum(CASE direction WHEN 'debit' THEN amount_atomic ELSE -amount_atomic END) AS balance
    FROM ledger_entries
    WHERE ledger_transaction_id = p_transaction_id
    GROUP BY asset_code
    HAVING sum(CASE direction WHEN 'debit' THEN amount_atomic ELSE -amount_atomic END) <> 0
  ) unbalanced;

  IF unbalanced_count > 0 THEN
    RAISE EXCEPTION 'ledger transaction % does not balance to zero per asset', p_transaction_id;
  END IF;

  IF NOT EXISTS (SELECT 1 FROM ledger_entries WHERE ledger_transaction_id = p_transaction_id) THEN
    RAISE EXCEPTION 'ledger transaction % has no entries', p_transaction_id;
  END IF;

  UPDATE ledger_transactions
  SET status = 'posted', posted_at = COALESCE(posted_at, now())
  WHERE id = p_transaction_id AND status = 'draft';
END;
$$;

CREATE OR REPLACE FUNCTION prevent_posted_ledger_entry_mutation()
RETURNS trigger
LANGUAGE plpgsql
AS $$
DECLARE
  parent_status text;
  parent_id text;
BEGIN
  IF TG_OP = 'INSERT' THEN
    parent_id := NEW.ledger_transaction_id;
  ELSE
    parent_id := OLD.ledger_transaction_id;
  END IF;

  SELECT status INTO parent_status
  FROM ledger_transactions
  WHERE id = parent_id;

  IF parent_status = 'posted' THEN
    RAISE EXCEPTION 'posted ledger entries are immutable';
  END IF;

  IF TG_OP = 'DELETE' THEN
    RETURN OLD;
  END IF;

  RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS ledger_entries_immutable_after_posted ON ledger_entries;
CREATE TRIGGER ledger_entries_immutable_after_posted
BEFORE INSERT OR UPDATE OR DELETE ON ledger_entries
FOR EACH ROW
EXECUTE FUNCTION prevent_posted_ledger_entry_mutation();

CREATE OR REPLACE FUNCTION prevent_posted_ledger_transaction_mutation()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
  IF OLD.status = 'posted' THEN
    RAISE EXCEPTION 'posted ledger transactions are immutable';
  END IF;

  RETURN COALESCE(NEW, OLD);
END;
$$;

DROP TRIGGER IF EXISTS ledger_transactions_immutable_after_posted ON ledger_transactions;
CREATE TRIGGER ledger_transactions_immutable_after_posted
BEFORE UPDATE OR DELETE ON ledger_transactions
FOR EACH ROW
EXECUTE FUNCTION prevent_posted_ledger_transaction_mutation();

CREATE INDEX IF NOT EXISTS invoices_merchant_created_idx ON invoices(merchant_id, created_at DESC);
CREATE INDEX IF NOT EXISTS invoices_public_state_idx ON invoices(public_id, state);
CREATE INDEX IF NOT EXISTS merchant_sessions_user_idx ON merchant_sessions(merchant_user_id, expires_at DESC);
CREATE INDEX IF NOT EXISTS payment_attempts_invoice_idx ON payment_attempts(invoice_id, created_at DESC);
CREATE INDEX IF NOT EXISTS payouts_merchant_state_idx ON payouts(merchant_id, state);
CREATE INDEX IF NOT EXISTS ledger_entries_transaction_idx ON ledger_entries(ledger_transaction_id);
CREATE INDEX IF NOT EXISTS ledger_events_entity_idx ON ledger_events(entity_type, entity_id, created_at DESC);
CREATE INDEX IF NOT EXISTS audit_logs_entity_idx ON audit_logs(entity_type, entity_id, created_at DESC);

INSERT INTO ledger_accounts (id, code, normal_balance)
VALUES
  ('acct_treasury_cash_usdc', 'treasury_cash_usdc', 'debit'),
  ('acct_merchant_payable_usdc', 'merchant_payable_usdc', 'credit'),
  ('acct_platform_fee_revenue_usdc', 'platform_fee_revenue_usdc', 'credit')
ON CONFLICT (id) DO NOTHING;
