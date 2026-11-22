//! Two-leg transactions (M1, design Q1/Q6): expansion of a user's transaction into postings,
//! validation against the accounts it touches, and in-place updates with stable posting ids.

use std::collections::BTreeMap;

use mrk_money::{CurrencyCode, Money};

use crate::ledger::{DomainError, Posting};

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
        unimplemented!("S2: NewTx::expand")
    }
}

impl JournalEntry {
    /// The legs, account leg first.
    pub fn postings(&self) -> [Posting; 2] {
        [self.account_leg.clone(), self.counter_leg.clone()]
    }

    /// Checks the entry against the accounts it posts to.
    pub fn validate(&self, _ctx: &LedgerContext) -> Result<(), DomainError> {
        unimplemented!("S2: JournalEntry::validate")
    }
}

/// Fails when the entry is locked: any of its legs is reconciled (FR-AC-06).
pub fn ensure_unlocked(_legs: &[StoredLeg]) -> Result<(), DomainError> {
    unimplemented!("S2: ensure_unlocked")
}

/// Plans rewriting `existing` as `new`, keeping both posting ids.
pub fn plan_update(
    _existing: &[StoredLeg],
    _new: &NewTx,
    _ctx: &LedgerContext,
) -> Result<UpdatePlan, DomainError> {
    unimplemented!("S2: plan_update")
}
