# mrkrabs (SQLite) — Indexes evaluation

> Evaluated against dbaudit/indexes.md (rubric v1.2).
> Source: `crates/mrk-store/migrations/0001_init.sql` at commit `f1ff160` (2026-11-19). Engine: SQLite 3.
> Coverage: 18 of 18 indexes read in full, cross-referenced against all 47 declared FKs in the same file. No query text exists yet (pre-M1), so predicate criteria have no in-scope object.

## Strengths

- **Partial indexes on `deleted = 0`** keep tombstones out of the hot access paths.
- **Unique constraints are expressed as partial unique indexes**, so a tombstoned row never blocks re-creation (`ux_fx_rate`, `ux_entry_share`, `ux_installment`, `ux_budget_line`).
- **Composite posting indexes** (`account_id, entry_id`) and (`category_id, entry_id`) lead with the FK and carry the join key to `journal_entry`.
- **No index duplicates a PK**, and none is a prefix of another.
- `change_log(device_id, hlc)` matches the protocol's "everything after X from origin D" scan exactly.

## Findings

| ID | Object | Anti-pattern | Conf. | Theory violated | Severity | Evidence (line) |
|---|---|---|---|---|---|---|
| I-01 | 21 FKs on transactional parents | **Missing FK index.** 21 of the 29 FKs whose parent is transactional lead no index. The most notable are `journal_entry.payee_id` (spending by payee, FR-RP-03), `entry_tag.tag_id` (filter by tag), `budget_line.category_id` (the budget matrix by category; `ux_budget_line` leads with `period`), `installment_plan.account_id`, `reconciliation.account_id`, `attachment.entry_id` and `entry_share.settled_entry_id`. Deletes are soft today. Compaction ("after all paired peers acknowledge", docs/05-sync.md:83) hard-deletes parents, and each such delete scans every un-indexed child table. | ✅ | Access paths and referential-check cost (§4.1, §7) | Medium | 0001_init.sql:115,136,142-143,179-181,189,197,214,240-242,251,265,274,295-296,322-325,333 |
| I-02 | `ix_category_parent`, `ix_entry_batch` | Not partial, unlike their siblings. Tombstoned rows stay in the index. This is harmless at the expected scale and only an inconsistency. | ✅ | Physical design consistency (§7) | Low | 0001_init.sql:110,149 |

## Signal counts

| Anti-pattern | Count |
|---|---:|
| FK not leading any index (all 47 FKs) | 37 |
| FK not leading any index (29 FKs on transactional parents) | 21 |
| Index duplicating PK | 0 |
| Prefix-redundant index | 0 |

## Needs-data flags (not scored)

- 🔬 **N-02 partial-index usage.** SQLite uses a partial index only when the query's `WHERE` implies the index predicate. `ix_account_scope`, `ix_entry_date`, `ix_posting_account`, `ix_posting_category` and `ix_installment_due` need every M1 query to repeat `deleted = 0` literally.
- 🔬 **N-03 account register.** An account register ordered by date has to join `posting` → `journal_entry.entry_date`, and `ix_posting_account (account_id, entry_id)` can't range on the date. Check it with `EXPLAIN QUERY PLAN` once M1 queries exist.
- 🔬 **N-07 compaction cost.** How long a hard delete takes without FK indexes depends on the row counts at compaction time.
- 🔬 `ix_account_scope (scope, owner_member_id)`: the leading column has low cardinality on a table of roughly 10 rows, so the planner probably never picks it.

## Checklist score

| # | Criterion | p | Result |
|---|---|--:|---|
| 1 | No index duplicates its table's PK | 18/18 = 1.00 | 🟢 |
| 2 | No prefix duplication | 18/18 = 1.00 | 🟢 |
| 3 | No heaps | n/a on SQLite (rowid/`WITHOUT ROWID` tables), left out | — |
| 4 | FK columns lead an index | 8/29 = 0.28 | 🔴 |
| 5 | Predicates are sargable | no query text in scope, left out | — |
| 6 | Fill factor matched to volatility | n/a on SQLite, left out | — |
| 7 | Composite order matches the predicates | no query text in scope, left out | — |

Base = (2 + 2 + 0) / (2 × 3) × 10 = **6.7**. No Critical or High finding, so no cap.

**File score: 6.7.**
