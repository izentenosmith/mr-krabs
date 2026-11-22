//! Double-entry ledger invariants (ADR-0003).

use std::collections::BTreeMap;
use std::fmt;

use mrk_money::{CurrencyCode, Money, MoneyError};

/// What a posting points at: a real account (asset/liability), a category (nominal
/// income/expense account), or the FX-trading position of the posting's own currency.
///
/// An FX leg has no target row: its currency *is* the target. An `fx_exchange` entry
/// balances per currency through a pair of FX legs (docs/03, schema v2 `posting.leg = 'fx'`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Leg {
    Account(String),
    Category(String),
    FxTrading,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Posting {
    pub leg: Leg,
    pub amount: Money,
}

impl Posting {
    pub fn account(id: &str, amount: Money) -> Self {
        Self {
            leg: Leg::Account(id.to_owned()),
            amount,
        }
    }

    pub fn category(id: &str, amount: Money) -> Self {
        Self {
            leg: Leg::Category(id.to_owned()),
            amount,
        }
    }

    /// FX-trading leg in `amount`'s currency.
    pub fn fx(amount: Money) -> Self {
        Self {
            leg: Leg::FxTrading,
            amount,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainError {
    TooFewPostings,
    ZeroAmount,
    Unbalanced {
        currency: CurrencyCode,
        difference: i64,
    },
    Money(MoneyError),
}

impl fmt::Display for DomainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooFewPostings => write!(f, "an entry needs at least two postings"),
            Self::ZeroAmount => write!(f, "postings cannot be zero"),
            Self::Unbalanced {
                currency,
                difference,
            } => write!(f, "entry is unbalanced in {currency} by {difference}"),
            Self::Money(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for DomainError {}

impl From<MoneyError> for DomainError {
    fn from(e: MoneyError) -> Self {
        Self::Money(e)
    }
}

/// Checks the fundamental invariant: ≥ 2 non-zero postings that sum to zero **per currency**.
pub fn validate_balanced(postings: &[Posting]) -> Result<(), DomainError> {
    if postings.len() < 2 {
        return Err(DomainError::TooFewPostings);
    }
    let mut sums: BTreeMap<CurrencyCode, Money> = BTreeMap::new();
    for p in postings {
        if p.amount.is_zero() {
            return Err(DomainError::ZeroAmount);
        }
        let entry = sums
            .entry(p.amount.currency)
            .or_insert_with(|| Money::zero(p.amount.currency));
        *entry = entry.checked_add(p.amount)?;
    }
    match sums.into_iter().find(|(_, m)| !m.is_zero()) {
        Some((currency, m)) => Err(DomainError::Unbalanced {
            currency,
            difference: m.minor,
        }),
        None => Ok(()),
    }
}

/// How an entry affects reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryKind {
    /// Money left the user's accounts into expense categories.
    Expense,
    /// Money entered from income categories.
    Income,
    /// Only account legs: moves between own accounts — never income or expense (FR-TX-02).
    Transfer,
    /// Mixture of income and expense category legs.
    Mixed,
}

/// Classifies a (balanced) entry by its category legs.
/// Positive category amount = expense (debit), negative = income (credit).
/// Account and FX-trading legs never make an entry income or expense, so a currency
/// exchange between own accounts is a transfer.
pub fn classify(postings: &[Posting]) -> EntryKind {
    let (mut debit, mut credit) = (false, false);
    for p in postings {
        if let Leg::Category(_) = p.leg {
            if p.amount.minor > 0 {
                debit = true;
            } else {
                credit = true;
            }
        }
    }
    match (debit, credit) {
        (false, false) => EntryKind::Transfer,
        (true, false) => EntryKind::Expense,
        (false, true) => EntryKind::Income,
        (true, true) => EntryKind::Mixed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrk_money::Currency;

    fn clp(v: i64) -> Money {
        Money::new(v, Currency::CLP.code)
    }
    fn usd(v: i64) -> Money {
        Money::new(v, Currency::USD.code)
    }

    #[test]
    fn expense_is_balanced_and_classified() {
        let entry = [
            Posting::account("checking", clp(-45_990)),
            Posting::category("supermercado", clp(45_990)),
        ];
        assert_eq!(validate_balanced(&entry), Ok(()));
        assert_eq!(classify(&entry), EntryKind::Expense);
    }

    #[test]
    fn transfer_to_savings_is_not_an_expense() {
        let entry = [
            Posting::account("checking", clp(-100_000)),
            Posting::account("fondo-emergencia", clp(100_000)),
        ];
        assert_eq!(validate_balanced(&entry), Ok(()));
        assert_eq!(classify(&entry), EntryKind::Transfer);
    }

    #[test]
    fn salary_is_income() {
        let entry = [
            Posting::account("checking", clp(1_000_000)),
            Posting::category("sueldo", clp(-1_000_000)),
        ];
        assert_eq!(classify(&entry), EntryKind::Income);
    }

    #[test]
    fn unbalanced_entry_is_rejected() {
        let entry = [
            Posting::account("checking", clp(-10_000)),
            Posting::category("delivery", clp(9_999)),
        ];
        assert_eq!(
            validate_balanced(&entry),
            Err(DomainError::Unbalanced {
                currency: Currency::CLP.code,
                difference: -1
            })
        );
    }

    #[test]
    fn balance_is_checked_per_currency() {
        // CLP -> USD exchange through FX legs: each currency balances on its own.
        let entry = [
            Posting::account("checking", clp(-23_750)),
            Posting::fx(clp(23_750)),
            Posting::fx(usd(-2_500)),
            Posting::account("card-usd", usd(2_500)),
        ];
        assert_eq!(validate_balanced(&entry), Ok(()));
        // FX legs are neither income nor expense: paying a USD card from CLP is a transfer.
        assert_eq!(classify(&entry), EntryKind::Transfer);

        let broken = [
            Posting::account("checking", clp(-23_750)),
            Posting::account("card-usd", usd(23_750)),
        ];
        assert!(matches!(
            validate_balanced(&broken),
            Err(DomainError::Unbalanced { .. })
        ));
    }

    #[test]
    fn needs_two_non_zero_postings() {
        assert_eq!(
            validate_balanced(&[Posting::account("a", clp(1))]),
            Err(DomainError::TooFewPostings)
        );
        let zero = [Posting::account("a", clp(0)), Posting::account("b", clp(0))];
        assert_eq!(validate_balanced(&zero), Err(DomainError::ZeroAmount));
    }

    #[test]
    fn fx_legs_do_not_change_classification_of_a_purchase() {
        // A USD software purchase paid from a CLP account through an FX pair is an expense.
        let entry = [
            Posting::account("checking", clp(-23_750)),
            Posting::fx(clp(23_750)),
            Posting::fx(usd(-2_500)),
            Posting::category("software", usd(2_500)),
        ];
        assert_eq!(validate_balanced(&entry), Ok(()));
        assert_eq!(classify(&entry), EntryKind::Expense);
    }

    mod props {
        use super::*;
        use proptest::prelude::*;

        fn leg() -> impl Strategy<Value = Posting> {
            (0u8..2, -1_000_000i64..1_000_000)
                .prop_filter("non-zero", |(_, a)| *a != 0)
                .prop_map(|(k, a)| match k {
                    0 => Posting::account("a", clp(a)),
                    _ => Posting::category("c", clp(a)),
                })
        }

        proptest! {
            /// Adding FX-trading legs never changes how an entry is classified.
            #[test]
            fn classification_ignores_fx_legs(
                legs in proptest::collection::vec(leg(), 1..6),
                fx in proptest::collection::vec(-1_000_000i64..1_000_000, 0..4),
            ) {
                let mut with_fx = legs.clone();
                with_fx.extend(fx.into_iter().filter(|a| *a != 0).map(|a| Posting::fx(usd(a))));
                prop_assert_eq!(classify(&legs), classify(&with_fx));
            }
        }
    }
}
