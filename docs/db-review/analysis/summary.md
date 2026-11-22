# mrkrabs (SQLite) — Database analysis summary

> Status: 🔬 greenfield. The schema exists but no code writes it yet (pre-M1).
> Object counts: 31 tables, 18 indexes, 0 views, 0 procedures, 0 triggers. There are no views, procedures or triggers by design (ADR-0002), and a test and a CI check enforce that.
> Source: `crates/mrk-store/migrations/0001_init.sql`, unchanged from `f1ff160` through `de5b7f3` / PR 14 (`b660aa1`). Requirements are read at PR 14. Rubric v1.2. Mode: database audit.
> **Revision 2 (2026-11-22):** re-read against PR 14's data-model and requirements changes. These changes are categories as user data in a tree of any depth, delete = merge, workbook import, the rules engine, match scores and manual valuations. The score is unchanged: the new findings are Medium/Low and the caps still hold.

## Scores

| tables | indexes | views | procedures | triggers | **Database** |
|-------:|--------:|------:|-----------:|---------:|-------------:|
| 6.0 (base 7.9) | 6.7 | n/a | n/a | n/a | **6.0** |

The weighted mean is (31 × 6.0 + 18 × 6.7) / 49 = 6.3. The database-level High cap brings it to **6.0**.
Without the cap: (31 × 7.9 + 18 × 6.7) / 49 = **7.5**. The cap does 1.5 points of the work. Two High findings, both fixable, separate this schema from "good, minor debt".

## Severity tally

| Critical | High | Medium | Low | Resolved |
|---------:|-----:|-------:|----:|---------:|
| 0 | 2 | 15 | 13 | 1 (T-04, in docs) |

## Needs-data flags (unscored, but tracked)

> A high score with many flags is **not** a clean bill of health.

| tables | indexes | views | procedures | triggers | Total objects flagged |
|-------:|--------:|------:|-----------:|---------:|------:|
| 5 (category, tag, payee, journal_entry, attachment) | 5 (`ix_account_scope`, `ix_entry_date`, `ix_posting_account`, `ix_posting_category`, `ix_installment_due`) | — | — | — | **10 of 49** |

The named flags are N-01 usage, N-02 partial-index usage, N-03 account register path, N-05 installment-plan cardinality, N-06 attachment storage and N-07 compaction cost. N-04 (name collation) is now **settled by requirement**: FR-IM-07 says matching is case- and accent-insensitive. It stays counted until a key implements it.

## Top findings (worst first)

1. **T-01 High.** The documented FX-trading posting leg has no place in `posting`. The only workaround, a pseudo-category, puts FX into income and expense. See [tables.md](tables.md), arc completeness.
2. **T-02 High.** `category`, `tag` and `payee` have no alternate key, and seeding the default tree on two devices duplicates it. FR-IM-07's name matching makes duplicate sibling names ambiguous, not just untidy. Entity integrity, §2.1.
3. **T-26 / T-27 Medium (new).** The category tree under sync has two problems. Concurrent moves can create a cycle that no device wrote, and a delete-by-merge leaves the partner's late postings pointing at a tombstone with no recorded target.
4. **T-03 Medium.** Kind consistency and cycles are Rust-only, although the kind rule can be declared.
5. **T-05 Medium.** The posting-currency = account-currency rule could be declared with a composite FK, but isn't.
6. **T-06, T-07, T-08 Medium.** `journal_entry` stores derived or misplaced state: `kind` overlaps `classify()`, `is_shared` duplicates the share rows, and `status` belongs on the posting (FR-AC-06 now also locks reconciled transactions).
7. **I-01 Medium.** 21 of the 29 transactional FKs lead no index.
8. **T-13, T-14 Medium.** Sync gaps: no table receives balance snapshots, and immediate FKs make batch apply order-dependent.

## Systemic issues for this database

- **Rust-only enforcement where declaration is free.** ADR-0002 puts business rules in Rust, and that stands. But PK/FK/UNIQUE/CHECK are explicitly allowed, and three integrity rules that need none of Rust's logic are still left undeclared (T-02, T-03a, T-05).
- **Sync shapes the constraints, and the any-depth tree makes it sharper.** Every `UNIQUE` interacts with per-field LWW merge. Every FK interacts with apply order and partial replication. Every tree edit interacts with concurrent moves (T-26) and merges (T-27). The schema and docs/05 don't yet say what the merger does in any of these cases.
- **Categories are already data.** PR 14 now says so in the docs (docs/03 "Categories are data"). What is still missing in the schema is the seed contract (stable identity of seeded rows across devices), name keys that match FR-IM-07, and the merge and cycle rules for a replicated tree.
- **The requirements have moved ahead of 0001 (T-29).** Planned forward migrations need tables for valuations, match candidates, staged budget lines, remembered category mappings and the staged rules engine.

## Overall grade

The structure is sound and deliberate for a greenfield ledger. It is `STRICT`, uses integer money, declares its exclusive arcs, keeps tuple `CHECK`s, has tombstone-aware uniqueness and no SQL logic. That earns a 7.5 base. Two High findings cap it at 6.0, *Needs work*. One is a ledger leg the documented model needs and the table can't store. The other is missing natural keys on the reference entities that a seeded, synced, name-matched category tree depends on. Both are cheap to fix before M1 writes any rows, and expensive after. PR 14 adds two category-tree risks that only appear with two devices (T-26, T-27). Each needs a merge rule more than a constraint.

## Gate — mrkrabs: does not pass

- Score ≥ 9.0: 6.0 (base 7.5 uncapped, capped by T-01 / T-02) — ❌
- No open Critical or High: 0 Critical, 2 High — ❌
- Not unused or a copy: greenfield, not yet exercised by code — 🔬
- 🔬 flags ≤ 10 % of objects: 10 of 49 (20 %) — ❌
