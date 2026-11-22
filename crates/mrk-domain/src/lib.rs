//! Pure domain rules for Mr Krabs. No IO, no database, no async.
//!
//! M0 ships the core ledger invariant (balanced double entry); the full model
//! described in `docs/03-data-model.md` grows here milestone by milestone.

pub mod category;
pub mod ledger;

pub use category::{
    BudgetLineRow, BudgetSum, CategoryError, CategoryKind, CategoryNode, MergePlan, normalize_name,
    plan_merge, validate_kind_change, validate_parent,
};
pub use ledger::{DomainError, EntryKind, Leg, Posting, classify, validate_balanced};
