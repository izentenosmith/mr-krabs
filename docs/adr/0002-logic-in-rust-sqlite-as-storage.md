# ADR-0002 · All logic in Rust; SQLite as storage only

**Status:** Accepted

## Context
Logic split across SQL triggers/views, Tauri handlers and React components becomes untestable and inconsistent (e.g. rounding in two places). The current spreadsheet shows the failure mode: formulas referencing shifted rows silently report wrong totals.

## Decision
- `mr-krabs-core` (Rust) owns validation, aggregation, FX, budgets, reports, import and sync.
- SQLite holds data with integrity constraints only (PK/FK/UNIQUE/NOT NULL/simple CHECK). No triggers, views, procedures, or side-effecting UDFs. Enforced by a CI script.
- Tauri commands only delegate. React only renders DTOs.

## Consequences
- ✅ One place to test every rule; core is portable (Linux CI, future mobile/CLI).
- ✅ Sync merge logic sees every write (no hidden trigger side effects).
- ❌ Some aggregation work SQL could do is done in Rust — acceptable at household scale (≈100k postings).
