# ADR-0005 · HLC + per-field LWW change log

**Status:** Accepted

## Context
Both partners edit offline. We need deterministic convergence without a server and with good auditability.

## Decision
- Hybrid Logical Clocks order all changes; ties broken by device ID.
- Every mutation appends to an append-only `change_log` in the same SQL transaction; per-field clocks implement last-writer-wins per field.
- Soft deletes (tombstones); domain-level invariants re-checked after merge, producing user-visible conflicts when meaning is at stake (unbalanced entry, edit vs delete, reconciled edits, duplicates).
- Derived values are never synced.

## Alternatives
- Full CRDT library (Automerge/Yrs): strong guarantees but opaque storage and poor fit for relational queries and accounting invariants.
- Row-level LWW: simpler but loses concurrent edits of different fields.

## Consequences
- ✅ Simple to reason about, relational data stays queryable, full audit trail for free.
- ❌ Requires careful grouping of entry + postings; covered by simulation property tests.
