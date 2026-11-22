//! Two-leg transactions (M1, design Q1/Q6): expansion of a user's transaction into postings,
//! validation against the accounts it touches, and in-place updates with stable posting ids.

use std::collections::BTreeMap;

use mrk_money::{CurrencyCode, Money};

use crate::ledger::{DomainError, EntryKind, Leg, Posting, classify, validate_balanced};

/// The type the user chose for a transaction. Expense, income and transfer are re-derived
/// from the legs by `classify`; the declared type must agree with them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TxType {
    Expense,
    Income,
    Transfer,
    OpeningBalance,
}

/// A transaction as the user enters it. Amounts of expense, income and transfer are positive;
/// an opening balance may be negative (a card's debt) but not zero.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NewTx {
    Expense {
        account: String,
        category: String,
        amount: Money,
    },
    Income {
        account: String,
        category: String,
        amount: Money,
    },
    Transfer {
        from: String,
        to: String,
        amount: Money,
    },
    /// FR-AC-03. `date` is the entry date core stores; the legs don't depend on it.
    OpeningBalance {
        account: String,
        amount: Money,
        date: String,
    },
}

/// A balanced two-leg entry. The **account leg** is the account the transaction is recorded
/// on (a transfer's `from`); the **counter leg** is the category, the transfer's `to`, or the
/// opening balance's FX-trading leg.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JournalEntry {
    pub tx_type: TxType,
    pub account_leg: Posting,
    pub counter_leg: Posting,
}

/// What validation needs to know about an account.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AccountInfo {
    pub currency: CurrencyCode,
    pub archived: bool,
}

/// The accounts an entry may post to, by id.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LedgerContext {
    pub accounts: BTreeMap<String, AccountInfo>,
}

/// Status of an account leg (ADR-0009). Category and FX legs have none.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LegStatus {
    Pending,
    Cleared,
    Reconciled,
}

/// A live posting of an existing entry, as the store read it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredLeg {
    pub id: String,
    pub posting: Posting,
    pub status: Option<LegStatus>,
}

/// A part of a posting an update can change.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum LegField {
    /// What the leg points at: its kind (account/category/fx) and target id.
    Target,
    /// Amount and currency.
    Amount,
}

/// One existing posting, rewritten in place.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LegUpdate {
    /// The id the posting keeps.
    pub id: String,
    /// The posting after the update.
    pub posting: Posting,
    /// What changed; empty when the leg is untouched.
    pub changes: Vec<LegField>,
}

/// How an existing two-leg entry becomes the new transaction (design Q6).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdatePlan {
    pub entry: JournalEntry,
    pub account_leg: LegUpdate,
    pub counter_leg: LegUpdate,
}

impl NewTx {
    /// Expands the transaction into its two legs.
    pub fn expand(&self) -> Result<JournalEntry, DomainError> {
        let (tx_type, account_leg, counter_leg) = match self {
            Self::Expense {
                account,
                category,
                amount,
            } => {
                let amount = positive(*amount)?;
                (
                    TxType::Expense,
                    Posting::account(account, amount.checked_neg()?),
                    Posting::category(category, amount),
                )
            }
            Self::Income {
                account,
                category,
                amount,
            } => {
                let amount = positive(*amount)?;
                (
                    TxType::Income,
                    Posting::account(account, amount),
                    Posting::category(category, amount.checked_neg()?),
                )
            }
            Self::Transfer { from, to, amount } => {
                if from == to {
                    return Err(DomainError::SameAccountTransfer);
                }
                let amount = positive(*amount)?;
                (
                    TxType::Transfer,
                    Posting::account(from, amount.checked_neg()?),
                    Posting::account(to, amount),
                )
            }
            Self::OpeningBalance {
                account, amount, ..
            } => {
                if amount.is_zero() {
                    return Err(DomainError::ZeroAmount);
                }
                (
                    TxType::OpeningBalance,
                    Posting::account(account, *amount),
                    opening_balance_counter_leg(*amount)?,
                )
            }
        };
        Ok(JournalEntry {
            tx_type,
            account_leg,
            counter_leg,
        })
    }
}

fn positive(amount: Money) -> Result<Money, DomainError> {
    if amount.minor > 0 {
        Ok(amount)
    } else {
        Err(DomainError::NonPositiveAmount)
    }
}

/// The counter leg of an opening balance: an FX-trading leg of the opposite amount in the
/// account's currency (DEC-4, ADR-0011). It keeps the entry balanced and classified as a
/// transfer, never income or expense. The FX position (M4) must therefore exclude
/// `kind = opening_balance` entries, or every opening balance would look like an open
/// currency position.
fn opening_balance_counter_leg(amount: Money) -> Result<Posting, DomainError> {
    Ok(Posting::fx(amount.checked_neg()?))
}

impl TxType {
    /// The classification the legs of an entry of this type must have.
    fn expected_kind(self) -> EntryKind {
        match self {
            Self::Expense => EntryKind::Expense,
            Self::Income => EntryKind::Income,
            Self::Transfer | Self::OpeningBalance => EntryKind::Transfer,
        }
    }
}

impl JournalEntry {
    /// The account leg is an account; the counter leg is the kind the type needs.
    fn legs_fit_type(&self) -> bool {
        let counter_fits = match (self.tx_type, &self.counter_leg.leg) {
            (TxType::Expense | TxType::Income, Leg::Category(_)) => true,
            (TxType::Transfer, Leg::Account(_)) => true,
            (TxType::OpeningBalance, Leg::FxTrading) => true,
            _ => false,
        };
        matches!(self.account_leg.leg, Leg::Account(_)) && counter_fits
    }

    /// The legs, account leg first.
    pub fn postings(&self) -> [Posting; 2] {
        [self.account_leg.clone(), self.counter_leg.clone()]
    }

    /// Checks the entry against the accounts it posts to: every account leg's account exists,
    /// is not archived and has the leg's currency; the entry balances per currency; the legs
    /// classify as the declared type (a transfer carries no category leg, FR-TX-02); and each
    /// leg is the kind its role needs for that type.
    pub fn validate(&self, ctx: &LedgerContext) -> Result<(), DomainError> {
        let postings = self.postings();
        for p in &postings {
            if let Leg::Account(id) = &p.leg {
                let info = ctx
                    .accounts
                    .get(id)
                    .ok_or_else(|| DomainError::UnknownAccount(id.clone()))?;
                if info.archived {
                    return Err(DomainError::ArchivedAccount(id.clone()));
                }
                if info.currency != p.amount.currency {
                    return Err(DomainError::CurrencyMismatch {
                        account: id.clone(),
                        expected: info.currency,
                        found: p.amount.currency,
                    });
                }
            }
        }
        validate_balanced(&postings)?;
        let derived = classify(&postings);
        let expected = self.tx_type.expected_kind();
        if derived != expected {
            return Err(DomainError::TypeMismatch {
                declared: self.tx_type,
                derived,
            });
        }
        if !self.legs_fit_type() {
            return Err(DomainError::LegKindMismatch {
                declared: self.tx_type,
            });
        }
        Ok(())
    }
}

/// Fails when the entry is locked: any of its legs is reconciled (FR-AC-06; the lock is
/// derived from the legs, docs/03). Core calls it before deleting an entry or editing its
/// header; `plan_update` calls it when a leg would change.
pub fn ensure_unlocked(legs: &[StoredLeg]) -> Result<(), DomainError> {
    match legs
        .iter()
        .find(|l| l.status == Some(LegStatus::Reconciled))
    {
        Some(l) => Err(DomainError::ReconciledEntryLocked {
            posting_id: l.id.clone(),
        }),
        None => Ok(()),
    }
}

/// Plans rewriting `existing` as `new`, keeping both posting ids (design Q6): each stored leg
/// keeps its role, so a type change converts the counter leg in place and no leg is dropped
/// or added. An update that changes no leg succeeds as is (the legs were validated when they
/// were written, so history on a since-archived account stays editable as a no-op). Any
/// change is refused when the entry is locked, and otherwise the new entry is validated.
pub fn plan_update(
    existing: &[StoredLeg],
    new: &NewTx,
    ctx: &LedgerContext,
) -> Result<UpdatePlan, DomainError> {
    let entry = new.expand()?;
    let (account, counter) = roles(existing)?;
    let account_leg = rewrite(account, &entry.account_leg);
    let counter_leg = rewrite(counter, &entry.counter_leg);
    if !(account_leg.changes.is_empty() && counter_leg.changes.is_empty()) {
        // Locked before invalid: a reconciled entry answers CONFLICT whatever the edit.
        ensure_unlocked(existing)?;
        entry.validate(ctx)?;
    }
    Ok(UpdatePlan {
        entry,
        account_leg,
        counter_leg,
    })
}

/// Splits a stored two-leg entry into (account leg, counter leg). The account leg is the only
/// account leg or, for a transfer, the one money leaves (negative).
fn roles(legs: &[StoredLeg]) -> Result<(&StoredLeg, &StoredLeg), DomainError> {
    let [a, b] = legs else {
        return Err(DomainError::UnsupportedEntryShape);
    };
    let is_account = |l: &StoredLeg| matches!(l.posting.leg, Leg::Account(_));
    match (is_account(a), is_account(b)) {
        (true, false) => Ok((a, b)),
        (false, true) => Ok((b, a)),
        (true, true) if a.posting.amount.minor < 0 && b.posting.amount.minor > 0 => Ok((a, b)),
        (true, true) if b.posting.amount.minor < 0 && a.posting.amount.minor > 0 => Ok((b, a)),
        _ => Err(DomainError::UnsupportedEntryShape),
    }
}

fn rewrite(stored: &StoredLeg, posting: &Posting) -> LegUpdate {
    let mut changes = Vec::new();
    if stored.posting.leg != posting.leg {
        changes.push(LegField::Target);
    }
    if stored.posting.amount != posting.amount {
        changes.push(LegField::Amount);
    }
    LegUpdate {
        id: stored.id.clone(),
        posting: posting.clone(),
        changes,
    }
}
