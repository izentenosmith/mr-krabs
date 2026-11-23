# ADR-0012 · Sync visibility is decided at send time

**Status:** Accepted (2026-11-22, owner decision DEC-9 during M1, from finding X-5)

## Context
From M1 every change-log row carries `visibility ∈ {household, private}` (ADR-0010), and FR-PV-02 requires that private data never reaches the partner's device. docs/05 sends only balance snapshots for `shared_summary` accounts, but it doesn't say how their posting rows are labelled. Personal budget lines and payee rules scoped to a private account have no rule either. An account's visibility can also change after its rows were logged, so any label fixed at write time can go stale.

## Decision
- The `change_log.visibility` written at mutation time is a **hint**. The M7 sender re-derives each change's visibility when it sends, from the row's current account and scope:
  - postings, entries and reconciliations of `private` and `shared_summary` accounts are never sent;
  - a `shared_summary` account sends balance snapshots only;
  - `scope = personal` budget lines follow their member's account visibility.
- M1 keeps the writer's labels as they are: rows of private accounts are `private`, everything else is `household`. No migration.

## Alternatives
- **Exact labels at write time** (a third `summary` value from M1). It needs another CHECK migration, and labels still go stale when visibility changes, so the sender would need send-time checks anyway. Rejected.
- **Defer to M7.** Leaves M1's labels looking authoritative to the M7 implementer. Rejected in favour of recording the rule now.

## Consequences
- ✅ A visibility change never leaks earlier rows; FR-PV-02 is enforced at the only point data leaves the device.
- ✅ No schema change in M1.
- ❌ The sender has to look up current visibility for each outgoing change (an indexed read per entity at sync time, and our volumes are small).
- What would change our mind: a sync design in which the receiver filters (it must not; private data never leaves).
