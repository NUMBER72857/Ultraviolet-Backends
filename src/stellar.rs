//! Stellar payment verification rules.
//!
//! This module is deliberately pure business logic: callers provide observed
//! transactions from Horizon/RPC, and this code decides whether they satisfy
//! the invoice contract without trusting browser callbacks.

#![allow(dead_code)]

use std::collections::HashMap;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpectedPayment {
    pub invoice_id: String,
    pub merchant_id: String,
    pub state: PaymentRequestState,
    pub network_passphrase: String,
    pub destination_account: String,
    pub amount_atomic: i64,
    pub asset_code: String,
    pub asset_issuer: String,
    pub memo: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PaymentRequestState {
    Pending,
    Verified,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservedPayment {
    pub transaction_hash: String,
    pub network_passphrase: String,
    pub successful: bool,
    pub ledger_sequence: Option<i64>,
    pub source_account: Option<String>,
    pub destination_account: String,
    pub amount_atomic: i64,
    pub asset_code: String,
    pub asset_issuer: String,
    pub memo: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LookupError {
    NotFound,
    Unavailable,
}

pub trait StellarTransactionLookup {
    fn payment_by_hash(&self, transaction_hash: &str) -> Result<ObservedPayment, LookupError>;
}

pub trait TransactionHashRegistry {
    fn invoice_for_hash(&self, transaction_hash: &str) -> Option<&str>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReconciliationDecision {
    Verified(VerifiedPayment),
    Rejected(RejectionReason),
    UnknownHash,
    SourceUnavailable,
    AlreadyVerified,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedPayment {
    pub invoice_id: String,
    pub transaction_hash: String,
    pub ledger_sequence: Option<i64>,
    pub source_account: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RejectionReason {
    DuplicateHash,
    FailedTransaction,
    NetworkMismatch,
    WrongDestination,
    WrongAmount,
    WrongAsset,
    WrongMemo,
    UnauthorizedRequestOwner,
}

pub fn reconcile_submitted_hash<L, R>(
    expected: &ExpectedPayment,
    submitted_by_merchant_id: &str,
    transaction_hash: &str,
    lookup: &L,
    registry: &R,
) -> ReconciliationDecision
where
    L: StellarTransactionLookup,
    R: TransactionHashRegistry,
{
    if expected.state == PaymentRequestState::Verified {
        return ReconciliationDecision::AlreadyVerified;
    }

    if expected.merchant_id != submitted_by_merchant_id {
        return ReconciliationDecision::Rejected(RejectionReason::UnauthorizedRequestOwner);
    }

    if let Some(owner_invoice_id) = registry.invoice_for_hash(transaction_hash) {
        if owner_invoice_id != expected.invoice_id {
            return ReconciliationDecision::Rejected(RejectionReason::DuplicateHash);
        }
    }

    let observed = match lookup.payment_by_hash(transaction_hash) {
        Ok(observed) => observed,
        Err(LookupError::NotFound) => return ReconciliationDecision::UnknownHash,
        Err(LookupError::Unavailable) => return ReconciliationDecision::SourceUnavailable,
    };

    verify_observed_payment(expected, &observed)
}

pub fn verify_observed_payment(
    expected: &ExpectedPayment,
    observed: &ObservedPayment,
) -> ReconciliationDecision {
    if !observed.successful {
        return ReconciliationDecision::Rejected(RejectionReason::FailedTransaction);
    }

    if observed.network_passphrase != expected.network_passphrase {
        return ReconciliationDecision::Rejected(RejectionReason::NetworkMismatch);
    }

    if observed.destination_account != expected.destination_account {
        return ReconciliationDecision::Rejected(RejectionReason::WrongDestination);
    }

    if observed.amount_atomic != expected.amount_atomic {
        return ReconciliationDecision::Rejected(RejectionReason::WrongAmount);
    }

    if observed.asset_code != expected.asset_code || observed.asset_issuer != expected.asset_issuer
    {
        return ReconciliationDecision::Rejected(RejectionReason::WrongAsset);
    }

    if observed.memo != expected.memo {
        return ReconciliationDecision::Rejected(RejectionReason::WrongMemo);
    }

    ReconciliationDecision::Verified(VerifiedPayment {
        invoice_id: expected.invoice_id.clone(),
        transaction_hash: observed.transaction_hash.clone(),
        ledger_sequence: observed.ledger_sequence,
        source_account: observed.source_account.clone(),
    })
}

#[derive(Default)]
pub struct InMemoryHashRegistry {
    owners: HashMap<String, String>,
}

impl InMemoryHashRegistry {
    pub fn with_owner(mut self, transaction_hash: &str, invoice_id: &str) -> Self {
        self.owners
            .insert(transaction_hash.to_string(), invoice_id.to_string());
        self
    }
}

impl TransactionHashRegistry for InMemoryHashRegistry {
    fn invoice_for_hash(&self, transaction_hash: &str) -> Option<&str> {
        self.owners.get(transaction_hash).map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct StaticLookup(Result<ObservedPayment, LookupError>);

    impl StellarTransactionLookup for StaticLookup {
        fn payment_by_hash(&self, _transaction_hash: &str) -> Result<ObservedPayment, LookupError> {
            self.0.clone()
        }
    }

    fn expected() -> ExpectedPayment {
        ExpectedPayment {
            invoice_id: "inv_1".to_string(),
            merchant_id: "merch_1".to_string(),
            state: PaymentRequestState::Pending,
            network_passphrase: "Test SDF Network ; September 2015".to_string(),
            destination_account: "GBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB"
                .to_string(),
            amount_atomic: 25_000_000,
            asset_code: "USDC".to_string(),
            asset_issuer: "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_string(),
            memo: "UV123".to_string(),
        }
    }

    fn observed() -> ObservedPayment {
        ObservedPayment {
            transaction_hash: "hash_1".to_string(),
            network_passphrase: "Test SDF Network ; September 2015".to_string(),
            successful: true,
            ledger_sequence: Some(123),
            source_account: Some(
                "GCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC".to_string(),
            ),
            destination_account: "GBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB"
                .to_string(),
            amount_atomic: 25_000_000,
            asset_code: "USDC".to_string(),
            asset_issuer: "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_string(),
            memo: "UV123".to_string(),
        }
    }

    fn reconcile(payment: ObservedPayment) -> ReconciliationDecision {
        reconcile_submitted_hash(
            &expected(),
            "merch_1",
            "hash_1",
            &StaticLookup(Ok(payment)),
            &InMemoryHashRegistry::default(),
        )
    }

    #[test]
    fn reconciles_valid_payment() {
        assert_eq!(
            reconcile(observed()),
            ReconciliationDecision::Verified(VerifiedPayment {
                invoice_id: "inv_1".to_string(),
                transaction_hash: "hash_1".to_string(),
                ledger_sequence: Some(123),
                source_account: Some(
                    "GCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC".to_string()
                ),
            })
        );
    }

    #[test]
    fn rejects_wrong_destination() {
        let mut payment = observed();
        payment.destination_account =
            "GDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDD".to_string();
        assert_eq!(
            reconcile(payment),
            ReconciliationDecision::Rejected(RejectionReason::WrongDestination)
        );
    }

    #[test]
    fn rejects_wrong_amount() {
        let mut payment = observed();
        payment.amount_atomic += 1;
        assert_eq!(
            reconcile(payment),
            ReconciliationDecision::Rejected(RejectionReason::WrongAmount)
        );
    }

    #[test]
    fn rejects_wrong_asset() {
        let mut payment = observed();
        payment.asset_code = "XLM".to_string();
        payment.asset_issuer = String::new();
        assert_eq!(
            reconcile(payment),
            ReconciliationDecision::Rejected(RejectionReason::WrongAsset)
        );
    }

    #[test]
    fn rejects_wrong_memo() {
        let mut payment = observed();
        payment.memo = "OTHER".to_string();
        assert_eq!(
            reconcile(payment),
            ReconciliationDecision::Rejected(RejectionReason::WrongMemo)
        );
    }

    #[test]
    fn rejects_failed_transaction() {
        let mut payment = observed();
        payment.successful = false;
        assert_eq!(
            reconcile(payment),
            ReconciliationDecision::Rejected(RejectionReason::FailedTransaction)
        );
    }

    #[test]
    fn rejects_duplicate_hash_owned_by_another_invoice() {
        let decision = reconcile_submitted_hash(
            &expected(),
            "merch_1",
            "hash_1",
            &StaticLookup(Ok(observed())),
            &InMemoryHashRegistry::default().with_owner("hash_1", "inv_2"),
        );

        assert_eq!(
            decision,
            ReconciliationDecision::Rejected(RejectionReason::DuplicateHash)
        );
    }

    #[test]
    fn returns_unknown_hash_without_marking_paid() {
        let decision = reconcile_submitted_hash(
            &expected(),
            "merch_1",
            "hash_missing",
            &StaticLookup(Err(LookupError::NotFound)),
            &InMemoryHashRegistry::default(),
        );

        assert_eq!(decision, ReconciliationDecision::UnknownHash);
    }

    #[test]
    fn returns_source_unavailable_during_rpc_or_horizon_outage() {
        let decision = reconcile_submitted_hash(
            &expected(),
            "merch_1",
            "hash_1",
            &StaticLookup(Err(LookupError::Unavailable)),
            &InMemoryHashRegistry::default(),
        );

        assert_eq!(decision, ReconciliationDecision::SourceUnavailable);
    }

    #[test]
    fn rejects_request_from_non_owner() {
        let decision = reconcile_submitted_hash(
            &expected(),
            "merch_2",
            "hash_1",
            &StaticLookup(Ok(observed())),
            &InMemoryHashRegistry::default(),
        );

        assert_eq!(
            decision,
            ReconciliationDecision::Rejected(RejectionReason::UnauthorizedRequestOwner)
        );
    }

    #[test]
    fn does_not_downgrade_already_verified_invoice() {
        let mut request = expected();
        request.state = PaymentRequestState::Verified;

        let mut bad_payment = observed();
        bad_payment.amount_atomic = 1;

        let decision = reconcile_submitted_hash(
            &request,
            "merch_1",
            "hash_1",
            &StaticLookup(Ok(bad_payment)),
            &InMemoryHashRegistry::default(),
        );

        assert_eq!(decision, ReconciliationDecision::AlreadyVerified);
    }
}
