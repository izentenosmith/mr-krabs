# ADR-0009 · Posting legs and per-leg status

**Status:** Accepted (2026-11-22, with the schema review proposal rev. 3; schema v2)

## Context
Schema v1 let a posting target either an account or a category. That left no place for the FX-trading legs that docs/03 uses to balance a currency exchange per currency. The only workaround, a pseudo-category, would count FX as income or expense. v1 also kept `status` on the journal entry, but a statement reconciles one account: a transfer between checking and a card can be reconciled on one statement and not the other.

## Decision
- `posting.leg ∈ {account, category, fx}`, with an exclusive-arc `CHECK`.
  - An FX leg has no target row: its currency is the target.
  - In Rust this is `Leg::FxTrading`. `classify()` ignores FX legs, so exchanges between own accounts are transfers.
- An account leg's currency must equal its account's: a composite FK `(account_id, currency_code) → account(id, currency_code)`.
- `status` (`pending`, `cleared`, `reconciled`) and `reconciliation_id` exist on **account legs only**. A composite FK makes a leg reconcile against a statement of its own account. An entry's status, and the FR-AC-06 lock, are derived from its legs.
- `journal_entry.kind` records intent only. Expense, income and transfer are derived from the legs by `classify()`.

## Alternatives
- **System "FX trading" accounts per currency** (GnuCash). Rejected: they appear in account lists, balances and net worth, and would need hiding everywhere.
- **Status on the entry.** Rejected: it can't express a transfer that is half reconciled.

## Consequences
- ✅ Currency exchanges balance per currency without polluting income, expense or accounts.
- ✅ Reconciliation matches how statements work.
- ❌ Reading an entry's status needs its account legs; the core derives it.
