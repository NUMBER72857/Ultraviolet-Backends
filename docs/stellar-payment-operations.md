# Stellar Payment Verification Operations

<!--
Operational guide for Stellar payment truth. It explains why reconciliation
belongs to the backend, what must be verified, and where contracts or workers
fit without letting wallets or AI decide accounting state.
-->

## Backend-Owned Payment Truth

The backend owns invoice payment truth. A submitted transaction hash is only a candidate. An invoice can move to `paid` only after backend reconciliation proves all of the following:

- The submitting merchant owns the invoice request.
- The transaction hash has not already been verified for another invoice.
- The transaction exists on the configured Stellar network.
- The transaction succeeded.
- The destination account equals the invoice treasury account.
- The amount equals the invoice gross amount in Stellar atomic units.
- The asset code and issuer equal the invoice asset.
- The memo equals the invoice payment memo.
- A previously verified invoice is never downgraded by a later bad observation, outage, or duplicate submission.

This is deliberately stricter than wallet callback handling. Wallets are UX surfaces, not accounting systems.

## Why A Contract Can Be Worth It

A contract solves a real business problem better than backend-owned payment requests only when the business needs programmable custody or settlement guarantees that the backend cannot credibly provide alone.

For Ultraviolet, the credible contract use case is escrowed tuition settlement:

- A student pays tuition into a contract-controlled balance.
- Release conditions are explicit: course start, attendance milestone, refund window expiry, dispute decision, or admin override with a timelock.
- Merchant payout, platform fee, and refund paths are enforced from the same locked funds.
- Every party can inspect the escrow rules before paying.

That is better than backend-owned payment requests because the core promise is not "did a payment arrive?" The promise is "funds cannot be unilaterally misrouted after arrival." A backend can record a fair policy, but if it controls the treasury key it can still violate that policy. A contract can reduce that custody trust assumption.

Do not build a contract just to verify invoice payment. That is expensive theater. Backend-owned reconciliation already solves invoice payment truth better: it is cheaper, easier to patch, easier to operate, can protect PII off-chain, and can handle Horizon/RPC outages without freezing product state.

## Threat Model

Primary assets:

- Treasury USDC balance.
- Invoice state and ledger entries.
- Transaction hash uniqueness.
- Merchant payout destination records.
- Student PII and invoice metadata.
- Reconciliation cursors and audit logs.

Threats and controls:

- Fake hash submission: never mark paid from client submission; fetch transaction server-side.
- Hash replay across invoices: enforce unique verified hashes and reject hashes owned by another invoice.
- Wrong destination payment: verify destination against invoice treasury account.
- Underpayment or overpayment ambiguity: require exact atomic-unit amount for automatic verification; route exceptions to manual review.
- Wrong asset spoofing: verify both asset code and issuer.
- Memo collision or omission: unique invoice memo; require exact memo match.
- Failed transaction: verify successful status before state transition.
- Merchant cross-claiming: require authenticated merchant ownership before reconciliation accepts a submission.
- Horizon/RPC outage: return source unavailable; do not fail closed into `paid` or downgrade verified payments.
- Reorg or ingestion inconsistency: persist ledger sequence and first-seen time; reconcile again before payout where risk warrants it.
- Insider payout tampering: payout destination changes need audit logs, role controls, and delayed high-risk changes.
- Contract upgrade abuse, if a contract is introduced: use timelocked upgrades, multisig admin, testnet rehearsal, and published migration notes.

## Audit Plan

Backend audit:

- Unit tests for all verifier branches: valid payment, wrong destination, wrong amount, wrong asset, wrong memo, failed transaction, duplicate hash, unknown hash, Horizon/RPC outage, unauthorized owner, and no downgrade from verified.
- Database tests for uniqueness constraints on `payments.stellar_transaction_hash`, `payment_attempts.transaction_hash`, invoice memo uniqueness, and ledger immutability.
- Integration tests against Stellar testnet for fee-bump, muxed-account policy, multi-operation transactions, and pagination cursor behavior.
- Manual review of every SQL state transition that touches invoice, payment, payout, ledger, or audit tables.
- Structured logs must exclude customer PII and secret material.

Contract audit, only if escrow is built:

- Write invariants before implementation: conservation of funds, authorized release only, refund path, no stuck funds, fee bounds, and replay resistance.
- Run local property tests and adversarial scenario tests.
- Testnet deployment with seeded accounts and scripted release/refund/dispute flows.
- Independent review of upgrade authority, admin key custody, and emergency pause behavior.
- Mainnet launch only after a freeze window where bytecode, config, and admin signers are final.

## Deployment Runbook

Backend verifier:

1. Run `cargo fmt --check`, `cargo check`, and `cargo test`.
2. Apply database migrations in staging.
3. Configure production `STELLAR_NETWORK_PASSPHRASE`, Horizon/RPC URL, USDC issuer, and treasury account.
4. Run testnet reconciliation fixtures for valid and invalid payments.
5. Deploy with reconciliation in observe-only mode if possible: persist candidates and decisions, but gate invoice state changes.
6. Compare observe-only decisions against expected invoices.
7. Enable invoice `paid` transitions.
8. Monitor rejected reasons, source availability, duplicate hash attempts, cursor lag, and paid invoice volume.

Contract, if escrow is introduced:

1. Deploy to testnet with production-like config.
2. Run scripted pay, release, refund, dispute, pause, and upgrade tests.
3. Publish contract ID, network, admin signers, upgrade policy, and supported asset issuer.
4. Mainnet deploy with a low transfer limit.
5. Increase limits only after successful settlement monitoring.

## Rollback Plan

Backend rollback:

- Disable the reconciliation worker or paid-state feature flag first.
- Keep accepting invoice creation if the treasury destination remains valid.
- Do not delete payment records; append corrective audit logs.
- If a bad release marks invoices paid, freeze payout creation, identify affected invoice IDs, and apply compensating ledger entries after review.
- Roll back code only after preserving reconciliation inputs: transaction hash, ledger sequence, memo, destination, amount, asset, and rejection reason.

Contract rollback:

- You cannot roll back chain history.
- Pause new deposits if the contract supports pause.
- Stop advertising contract payment instructions in the backend.
- Drain or migrate funds only through pre-audited admin or user-withdrawal paths.
- Publish exact affected contract IDs and replacement payment instructions.
- Keep backend reconciliation able to read old contract events until every open escrow is resolved.
