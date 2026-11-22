//! Account balances, derived from the ledger in Rust (ADR-0003, design Q8).

use std::collections::BTreeMap;

use mrk_money::Money;

use crate::ledger::{DomainError, Leg, Posting};

/// A posting row as the store reads it for balances.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BalanceRow {
    pub posting: Posting,
    /// The entry's civil date, `YYYY-MM-DD`.
    pub entry_date: String,
    /// `false` when the posting or its entry is deleted.
    pub live: bool,
}

/// Sums the live account legs dated on or before `as_of` (`YYYY-MM-DD`; civil dates compare
/// as strings, so any other shape is refused), per account id. Category and FX legs are skipped; an account with no counted
/// leg is absent from the result. Sums are checked: overflow is an error.
pub fn account_balances(
    rows: &[BalanceRow],
    as_of: &str,
) -> Result<BTreeMap<String, Money>, DomainError> {
    if !is_civil_date_shape(as_of) {
        return Err(DomainError::InvalidDate(as_of.to_owned()));
    }
    let mut balances: BTreeMap<String, Money> = BTreeMap::new();
    for row in rows
        .iter()
        .filter(|r| r.live && r.entry_date.as_str() <= as_of)
    {
        if let Leg::Account(id) = &row.posting.leg {
            let amount = row.posting.amount;
            match balances.get_mut(id) {
                Some(sum) => *sum = sum.checked_add(amount)?,
                None => {
                    balances.insert(id.clone(), amount);
                }
            }
        }
    }
    Ok(balances)
}

/// `YYYY-MM-DD` digits and dashes: the shape the schema's `entry_date` GLOB enforces, and the
/// one under which string order is date order.
fn is_civil_date_shape(date: &str) -> bool {
    let b = date.as_bytes();
    b.len() == 10
        && b.iter().enumerate().all(|(i, c)| match i {
            4 | 7 => *c == b'-',
            _ => c.is_ascii_digit(),
        })
}
