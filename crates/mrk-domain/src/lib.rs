//! Pure domain rules for Mr Krabs. No IO, no database, no async.
//!
//! M0 ships the core ledger invariant (balanced double entry); the full model
//! described in `docs/03-data-model.md` grows here milestone by milestone.

pub mod ledger;

pub use ledger::{DomainError, EntryKind, Leg, Posting, classify, validate_balanced};
