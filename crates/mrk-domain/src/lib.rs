//! Pure domain rules for Mr Krabs. No IO, no database, no async.
//!
//! M0 shipped the core ledger invariant (balanced double entry); M1 adds two-leg
//! transactions and derived balances. The full model described in `docs/03-data-model.md`
//! grows here milestone by milestone.

pub mod balance;
pub mod category;
pub mod entry;
pub mod ledger;

pub use balance::{BalanceRow, account_balances};
pub use category::{
    BudgetLineRow, BudgetSum, CategoryError, CategoryKind, CategoryNode, MergePlan, normalize_name,
    plan_merge, validate_kind_change, validate_parent,
};
pub use entry::{
    AccountInfo, JournalEntry, LedgerContext, LegField, LegStatus, LegUpdate, NewTx, StoredLeg,
    TxType, UpdatePlan, ensure_unlocked, plan_update,
};
pub use ledger::{DomainError, EntryKind, Leg, Posting, classify, validate_balanced};
