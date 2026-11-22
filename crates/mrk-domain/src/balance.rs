//! Account balances, derived from the ledger in Rust (ADR-0003, design Q8).

use std::collections::BTreeMap;

use mrk_money::Money;

use crate::ledger::{DomainError, Posting};

/// A posting row as the store reads it for balances.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BalanceRow {
    pub posting: Posting,
    /// The entry's civil date, `YYYY-MM-DD`.
    pub entry_date: String,
    /// `false` when the posting or its entry is deleted.
    pub live: bool,
}

/// Sums the live account legs dated on or before `as_of` (`YYYY-MM-DD`), per account id.
pub fn account_balances(
    _rows: &[BalanceRow],
    _as_of: &str,
) -> Result<BTreeMap<String, Money>, DomainError> {
    unimplemented!("S2: account_balances")
}
