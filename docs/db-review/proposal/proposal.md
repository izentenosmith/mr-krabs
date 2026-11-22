# Mr Krabs — Schema remediation proposal (before M1)

> Status: **accepted** by the owner on 2026-11-22, in the review session, together with these decisions: Q-02 tags and payees case-insensitive only; Q-05 kind rule declared as a composite FK; Q-06 installment-plan link 1:1. Migration 0002 is generated from this revision.
> Revision 3: 2026-11-22. It is re-based on the requirements merged in PR 14 (rev. 2), and adds the score-raising changes D-22 to D-25 (rev. 3).

## Sources

| Artifact | Path | Version |
|---|---|---|
| Source schema | `crates/mrk-store/migrations/0001_init.sql` | unchanged from `f1ff160` to `de5b7f3` / PR 14 `b660aa1` |
| Requirements | `docs/01`, `03`, `04`, `05`, `06`, `07`, `09` | PR 14 (`b660aa1`) |
| Audit | [../analysis/summary.md](../analysis/summary.md), [tables.md](../analysis/tables.md), [indexes.md](../analysis/indexes.md) | rubric v1.2, rev. 2 (2026-11-22) |
| Design | [../design/design.md](../design/design.md), [schema.sql](../design/schema.sql) (0002 target), [schema-planned.sql](../design/schema-planned.sql) (M4/M5/sync delta), [negative-tests.sql](../design/negative-tests.sql) | rev. 3 (2026-11-22) |
| Diagrams | [../diagrams/er-model.drawio](../diagrams/er-model.drawio), [relational-model.drawio](../diagrams/relational-model.drawio), generator [gen_diagrams.py](../diagrams/gen_diagrams.py) | generated from both schema files, rev. 3 |

Missing: no dbquery review and no dbarchitect record. Index decisions rest on the FK-coverage audit alone, because no M1 query exists yet to measure them.

## 1. Executive recommendation

- **Scope:** the replicated household-ledger schema in SQLite; the owner's question, *can categories be managed as data?*; and the schema impact of PR 14's new requirements.
- **Audit score: 6.0 (*Needs work*).** It is capped by two High findings, T-01 and T-02. Uncapped, it is 7.5 (*Good, minor debt*). The gate does not pass.
- **Posture: Refactor.** It follows the structure underneath the cap. Both Highs are local, and the rest of the schema is sound.
- **Ship the M1/M2 changes as one forward migration, `0002`, before M1 writes rows.** Every changed table is empty in any real install. Ship the M4/M5/sync tables in their own milestones from the already-tested shapes in `schema-planned.sql`.

**The answer on categories.** Yes, and PR 14 makes it a requirement. The schema already stores categories as rows. What this proposal adds is what keeps that true on two synced devices:

- **Any-depth tree with the kind rule declared.** A composite FK enforces that a child's kind equals its parent's. Kind is immutable, so the rule survives merges.
- **Deterministic ids for seeded rows** (`UUIDv5(seed_key)`). Both devices seed identical rows, with no duplicates after pairing.
- **A normalized `name_key`**, unique among live siblings. It is the same key FR-IM-07 import matching uses, so a name in a file maps to exactly one category.
- **`merged_into_id`** so delete-by-merge (FR-BU-07) also catches the partner's late postings.
- **A deterministic cycle-breaking rule** for concurrent moves.
- **A boundary between user data and code-owned behaviour.** Categories, tags, payees and rules are user data (synced). Account kinds and entry kinds are code-owned reference tables, installed by migrations and not synced. Closed domains (leg types, statuses) stay `CHECK` lists.

## 2. What is good, what needs attention

**Good, and kept as is:**

- `STRICT` tables
- integer minor units with a currency
- declared exclusive arcs
- tuple `CHECK`s
- tombstone-aware partial unique indexes
- uniform sync columns
- the no-SQL-logic rule, with its test
- the categorizer stays derived, not stored. This matches docs/03, and the design agrees.

**Needs attention:**

- **T-01 High:** the FX leg is missing.
- **T-02 High:** no natural keys on category, tag and payee.
- **T-26 / T-27, new:** cycles from concurrent moves, and late references to a merged category.
- **T-05 / T-06 / T-07 / T-08:** ledger state stored in the wrong place.
- **T-09, T-11 – T-13:** smaller modelling gaps.
- **I-01:** missing FK indexes.
- **T-28 / T-29:** value lists and tables that lag PR 14.

**Expected improvement:** both Highs closed, and every Medium is closed or accepted with a reason. The re-graded target scores **9.9** (tables 9.6, indexes 10.0); §5a shows the arithmetic. The gate still waits for M1 code and a query review (§5a).

## 3. Decision register

Band vocabulary: Reuse / Refactor / Rework / Rebuild.

| # | Object / group | Finding | Structure | Data | Logic | Decision |
|---|---|---|---|---|---|---|
| R-01 | `posting` leg target | T-01 (High) | Refactor | Reuse (empty) | Refactor | `leg ∈ {account, category, fx}` with the arc `CHECK` extended (D-14). `Leg::FxTrading`; `classify()` ignores FX legs. |
| R-02 | `category` tree | T-02 (High), T-03 | Refactor | Reuse (empty) | Refactor | Keep the adjacency list. Add `UNIQUE (id, kind)` and composite FKs `(parent_id, kind)` and `(merged_into_id, kind)`, plus `name_key` with a live-sibling unique key, `seed_key` and `merged_into_id` (D-01, D-05, D-18). Kind becomes immutable in Rust. |
| R-03 | Seed contract (categories, savings accounts, currencies) | T-02; *no audit finding* for currencies and accounts, driven by FR-CU-02 and the docs/01 seed note | Refactor | Rework | Rebuild | A Rust catalogue; ids `UUIDv5(MRK_SEED_NAMESPACE, seed_key)`; seeded on household create and adopt with `hlc = 0`, not change-logged (D-03). Needs **ADR-0008**. |
| R-04 | Tree merge rules | T-26, T-27 | Reuse | — | Rework | Merger: cycle-breaking (D-17), redirect via `merged_into_id` (D-18), name collision (D-05), `sync_conflict.kind` gains `tree_cycle` and `duplicate_name`. Built with sync (M-sync), designed now. |
| R-05 | Tag / payee / profile names | T-02 | Refactor | — | Reuse | Partial `UNIQUE … COLLATE NOCASE` (D-05; Q-02 decides whether these become accent-insensitive). |
| R-06 | Leg currency = account currency | T-05 | Refactor | — | Reuse | `account UNIQUE(id, currency_code)` + composite FKs (D-04). |
| R-07 | `journal_entry.kind`, `is_shared` | T-06, T-07, T-24 | Refactor | Reuse | Refactor | Intent-only `kind`; drop `is_shared`; 1:1 plan link (D-06, D-07). |
| R-08 | Reconciliation and lock | T-08; FR-AC-06 lock | Refactor | Reuse | Refactor | Status and reconciliation per account leg, composite FK to the same account. The lock is derived from `reconciled` (D-11). |
| R-09 | `entry_share.currency_code` | T-09 | Refactor | Reuse | Reuse | `NOT NULL` FK (D-15). |
| R-10 | Household settings | T-11 | Refactor | Reuse | Refactor | Typed columns on `household`; `household_setting` dropped (D-08). |
| R-11 | Card subtype | T-12 | Refactor | Reuse | Reuse | `CHECK`s (D-09). |
| R-12 | `account_valuation` (M4 / sync) | T-13, T-29 (FR-AC-07) | Refactor (new) | — | Refactor | One table for manual valuations and partner summaries (D-10). Shipped with M4. |
| R-13 | Sync apply order | T-14 | Reuse | — | Refactor | `PRAGMA defer_foreign_keys = ON` in the merger (D-16). |
| R-14 | Account and entry kinds | T-15 | Refactor | Reuse | Reuse | `account_kind` and `entry_kind` reference tables, installed by migrations; `kind` columns become FKs. A test checks enum ⇔ rows (D-24). |
| R-15 | Small guards | T-17–T-20, T-22, T-25 | Refactor | Reuse | Reuse | Date `GLOB`s, min ≤ max, payee_rule sets something, `sync_conflict.kind` domain, `UNIQUE public_key`; `account.visibility` has no default and `account_create` always sets it (D-12). |
| R-16 | FK indexes | I-01 | Refactor | — | — | Every transactional FK (38/38) leads a **non-partial** index; four plain indexes were added where the only index was partial (D-16, D-22). Re-measure in M1 (N-02, N-03). |
| R-17 | Rules engine (M5) | T-29 (FR-TX-10) | Rework | Rework | Rebuild | `rule` / `rule_condition` / `rule_action` with typed FKs replace `payee_rule` (D-19). Migrate the rows, then drop. |
| R-18 | Import staging (M5) | T-28, T-29 (FR-TX-07, FR-IM-06, FR-IM-07) | Refactor | — | Refactor | New `import_row` columns, `import_row_candidate`, `import_budget_row`, `import_category_mapping` (D-20). The `workbook` extractor and the typed `import_profile.parser_kind` / `bank` / `doc_type` land in 0002 (D-25). |
| R-19 | Recurring rules | T-10, T-29 (FR-BU-06) | Rework | Reuse (empty) | Refactor | In 0002: amount modes with one `CHECK` (D-21); template legs and shares as rows, `template_json` dropped (D-23). |
| R-20 | Deferred / accepted | T-16, T-21, T-23, I-02 | Reuse | — | — | As in design D-13. |

## 4. Target design and diagrams

- **Design.** [design.md](../design/design.md) rev. 3 is re-based on PR 14. Revision 1's two-table category model is withdrawn, because FR-BU-01 now allows any depth and postings on any node.
  - [schema.sql](../design/schema.sql) is the 0002 target (34 tables, 71 indexes).
  - [schema-planned.sql](../design/schema-planned.sql) adds 7 tables and 3 columns for M4/M5/sync.
  - Both ran on SQLite 3.51 `:memory:`. All 28 negative tests are rejected, and `foreign_key_check` and `integrity_check` are clean.
- **Barker ER model.** [er-model.drawio](../diagrams/er-model.drawio) shows 41 entities and 66 binary relationships, with role names, must/may halves, crow's feet and UID bars (ENTRY TAG, IMPORT ROW CANDIDATE).
  - Annotated: the recursive and merge relationships on CATEGORY, the posting, template-leg and rule-action arcs, the credit-card subtype, and the two code-owned reference lists (ACCOUNT KIND, ENTRY KIND).
  - Planned (M4/M5) entities are labelled. Parent-end *musts* the schema doesn't hold carry their Rust owner.
  - The relationships to CURRENCY are summarized in a note for legibility.
- **Relational model.** [relational-model.drawio](../diagrams/relational-model.drawio) shows all 41 tables with PK / FKn / AKn markers, types and nullability, partial unique indexes spelled out, CHECKs, and 80 FK edges.
- **Consistency.** Both diagrams are generated by introspecting the two schema files ([gen_diagrams.py](../diagrams/gen_diagrams.py)), so they match the design revision exactly. The XML validated, with no dangling edges and no duplicate ids, and both pages were rendered with the draw.io CLI. Some labels crowd on the busiest ER edges; that is cosmetic, and the files can be edited.
- **No mismatches to route.**

## 5. Migration sequence

**Consumers of the breaking changes.** These were found by searching `crates/`, `apps/` and `packages/` at `de5b7f3`. There is still no application code:

- `crates/mrk-store/src/lib.rs:159-198`: the table-list test, and a posting insert without `leg`/`status`.
- `crates/mrk-domain/src/ledger.rs:8-14,187-192`: the `Leg` enum and FX tests booked as `Posting::category("fx", …)`.
- `docs/03` (ER diagram, "Categories are data" wording on SQL vs Rust checks), `docs/05` (merge rules), `docs/04` (`category_delete`).
- Developer databases created by `docker compose` or `pnpm tauri dev`, which may hold synthetic rows.

With no deployed consumer, expand/contract isn't needed, and the contract happens in the same migration. N-1 safety comes from the existing guard: a v1 app opening a v2 file stops with `SchemaTooNew` (lib.rs:99), and peers sync only at equal `user_version` (docs/05).

| Step | Change | Finding | Pre-check | Validation | Rollback | Blast radius |
|---|---|---|---|---|---|---|
| 0 | Accept this proposal, answer Q-02–Q-11, accept **ADR-0008** (UUIDv5 for seeded rows) and the docs/03 wording change for the declared kind FK (Q-05) | T-02, T-03 | — | ADR merged in `docs/adr/` | revert the ADR PR | none |
| 1 | `migrate()` copies a **file** database to `<db>.v<N>.bak` before applying any pending migration (mrk-store) | — (safety for step 3) | free disk ≥ 2× file size | the backup opens; `integrity_check` = ok; row counts equal | delete the backup | none |
| 2 | Pre-checks at the top of 0002, before any DDL. Count: (a) category cycles; (b) child kind ≠ parent kind; (c) duplicate live sibling names after normalization; duplicate live tag / payee / profile names; (d) `entry_share` on multi-currency entries; (e) `is_shared = 1` without shares; (f) household accounts without `shared_full`; (g) entries marked `reconciled`; (h) `account.kind` / `journal_entry.kind` values outside the reference rows; (i) `recurring_rule.template_json` that can't be parsed into legs.  Any non-zero count from (a) to (f), (h) or (i) aborts the migration through a failing insert into a temp guard table, with no trigger. (g) is reported only. | T-02, T-03, T-07, T-09, T-25 | — | all counts 0 on a fresh DB and on developer DBs | automatic: the migration transaction rolls back | the developer DB that fails, until it is fixed or reset |
| 3 | `0002_ledger_legs_and_category_keys.sql`, in one transaction with `PRAGMA defer_foreign_keys = ON`. Insert the 4 seed currencies (`INSERT OR IGNORE`). For each changed table, parents before children (account_kind / entry_kind (create + seed rows) → category → account → household → reconciliation → installment_plan → recurring_rule (+ recurring_rule_posting / _share from `template_json`) → import_profile (parser split) → import_batch → journal_entry → posting → entry_share → payee_rule → the rest): `CREATE <t>_new` from schema.sql, `INSERT … SELECT` with the mapping below, `DROP <t>`, `ALTER TABLE <t>_new RENAME TO <t>`. Drop `household_setting`. Create all indexes **after** the copies. | T-01, T-02, T-03, T-05–T-12, T-15, T-17–T-20, T-22, T-25, T-27, T-28, I-01 | step 2 counts = 0; step 1 backup exists | `foreign_key_check` returns no rows; `integrity_check` = ok; **schema-equivalence test**: `sqlite_master` after 0001+0002 equals schema.sql applied fresh (normalized); row counts equal per table | the transaction rolls back on error. After commit, restore `<db>.v1.bak` | every table, on one device, with the app closed during migrate |
| 4 | Port negative-tests.sql into `mrk-store` Rust tests (one constraint per test, schema.sql part). Update `lib.rs` tests for the posting `leg`/`status`. Add the `name_key = normalize(name)` test. | — (proof for step 3) | — | `cargo test -p mrk-store` green | revert the test commit | tests only |
| 5 | mrk-domain: `Leg::FxTrading(CurrencyCode)`; `classify()` ignores FX legs; `normalize()` for names (shared with import matching); kind immutability and cycle checks on category writes | T-01, T-03 | — | `cargo test -p mrk-domain` green; proptest: classification is unchanged by FX legs; random move sequences never produce a cycle locally | revert the commit | ledger classification, category commands |
| 6 | mr-krabs-core: seed catalogue (categories + savings/investment accounts + currencies, es-CL and en), seeder with `ON CONFLICT DO NOTHING`, `hlc = 0`, no change_log, skipping name collisions. Enable the `uuid` `v5` feature (Cargo.toml + Cargo.lock together). `category_delete` sets `merged_into_id`, re-points references and sums colliding budget lines (Q-10) in one transaction. | T-02, T-27 | ADR-0008 accepted | seeding twice gives identical rows; two independently seeded stores produce byte-identical `category` tables; a tombstoned seed row is not resurrected; merge leaves no live reference to the merged id | revert the commit | onboarding, category management (M2) |
| 7 | Docs: docs/01 points to the Rust seed catalogue as the source; docs/03 ER diagram, legs, status per leg, and "kind rule declared + checked in Rust"; docs/05 merge rules (`duplicate_name`, `tree_cycle`, the `merged_into_id` redirect, `defer_foreign_keys`); a new ADR for status per leg | T-08, T-14, T-26, T-27 | — | docs review | revert | documentation |
| 8 | **M4** migration from schema-planned.sql: `account_valuation` | T-13, T-29 | M4 started | schema-equivalence test for 0001…000N | restore the pre-migration backup (step 1 mechanism) | net worth, partner summaries |
| 9 | **M5** migration from schema-planned.sql: rules engine tables, with `payee_rule` rows copied (1 condition + 1–2 actions each), then `payee_rule` dropped; import staging tables and columns | T-29 | count `payee_rule` rows | copied rules = original count; every action FK valid; equivalence test | restore the backup | rules, import |
| 10 | **Sync milestone:** merger rules D-05, D-16, D-17 and D-18, covered by the docs/05 convergence property test (concurrent same-name creates, concurrent moves forming a cycle, a posting into a category merged on the other device) | T-14, T-26, T-27 | Q-03, Q-11 answered | 2–3 simulated peers converge; no live reference to a merged category; no cycle | revert the merger change | sync only |
| 11 | Re-run dbaudit on 0001+0002. After M1, run dbquery on the real queries (N-02, N-03). | — | steps 3–6 merged | gate re-evaluated | — | none |

> **As built (dbbuild, 2026-11-22):** [`0002_ledger_legs_and_category_keys.sql`](../../../crates/mrk-store/migrations/0002_ledger_legs_and_category_keys.sql). The structure is unchanged from this revision. The mechanics are simpler than the plan below in two places:
> 1. **No Rust step.** No code before M1 writes `category` or `recurring_rule`, so guards G1/G2 require both tables to be empty, and abort otherwise. This replaces the planned Rust hook for `name_key` and `template_json`.
> 2. **Foreign keys.** `Store::migrate()` runs each migration with `foreign_keys` off and requires an empty `PRAGMA foreign_key_check` before commit. That is SQLite's documented rebuild procedure; `defer_foreign_keys` can't survive a `DROP TABLE` of a referenced parent.
>
> Guards G3/G4 implement pre-checks (d)/(e). Every other pre-check is enforced by the new tables' own constraints during the copy, which aborts the migration.
>
> **Steps 4–7 as built (2026-11-22):**
> - **Step 4.** The negative tests are mrk-store Rust tests. `name_key = normalize(name)` is asserted for every seeded and user category in mr-krabs-core.
> - **Step 5.** In mrk-domain: `Leg::FxTrading` (a unit variant: the posting's own currency is the target, so leg and amount can't disagree); `classify()` ignores FX legs, with a property test; `normalize_name()`; `validate_parent` (kind and cycle, with a property test); `validate_kind_change`; `plan_merge` (budget sums in `Money`).
> - **Step 6.** In mr-krabs-core: the seed catalogue (`seed.rs`, es-CL and English names, UUIDv5 ids), `App::seed_household_defaults` and `App::category_delete` (merge or plain delete). The store writes are in `mrk-store/src/categories.rs`. The `uuid` `v5` feature is enabled.
>   - **Not wired yet:** `household_create`, household adoption and a `category_delete` shell command don't exist before M1/M2, so the seeder and delete are core methods for now. `category_delete` takes an explicit HLC `Stamp` until the device clock arrives, and writes no change-log entry until the sync milestone.
> - **Step 7.** ADR-0008 (seeded ids) and ADR-0009 (posting legs and per-leg status); docs/01 (catalogue as source), docs/03 (schema v2, ER diagram, legs and status, categories), docs/05 (merge rules 7–11) and CLAUDE.md (ID exception).

**0002 data mapping** (a no-op on empty databases; correct on developer databases):

- **`account_kind` / `entry_kind`:** created and filled by the migration's own `INSERT`s, before the tables that reference them.
- **`recurring_rule`:** amounts map to `amount_mode` (`range` when min and max are set, otherwise `exact`, or NULL). The `template_json` legs and shares become `recurring_rule_posting` / `recurring_rule_share` rows. The Rust migration hook parses the JSON, the same hook that backfills `name_key`.
- **`import_profile`:** `parser` `'mapping'` / `'ai'` → `parser_kind`; `'builtin:<bank>:<doc>'` → `parser_kind = 'builtin'`, `bank`, `doc_type`.
- **`category`:** copied as is, plus `name_key = normalize(name)`. That is computed by the Rust migration hook, because SQL can't strip accents. 0002 therefore runs as `apply SQL → Rust backfill name_key → validate` inside one transaction in `migrate()`. `seed_key` and `merged_into_id` stay NULL for existing rows.
- **`posting`:**
  - `leg` = `account` if `account_id` is set, otherwise `category`.
  - Account legs get `status` from `journal_entry.status`, and `reconciliation_id` stays NULL. An entry previously marked `reconciled` becomes `cleared` and is listed by pre-check (g) for re-reconciling.
- **`journal_entry.kind`:** expense, income, transfer and split → `regular`; the others are copied. `is_shared` and `status` are dropped.
- **`entry_share.currency_code`:** the entry's single posting currency, guaranteed by pre-check (d).
- **`household`:** `base_currency_code`, `budget_mode` and `locale` come from `household_setting`, defaulting to `'CLP'`, `'per_installment'` and `'es-CL'`.

**Dependencies and risks:**

- **Order.** Step 3 depends on 1–2, step 6 on 0. Steps 8–10 wait for their milestones.
- **`name_key` backfill and template parsing.** Because of them, 0002 is the first migration with a **Rust step**. `MIGRATIONS` gains an optional post-SQL hook. This is the smallest change that keeps normalization in one Rust function, rather than duplicating an accent table in SQL.
- **`ALTER TABLE … RENAME` rewrites FK text that names `<t>_new`.** No such text exists. The schema-equivalence test guards this.
- **`PRAGMA foreign_keys` can't toggle inside a transaction.** Hence `defer_foreign_keys`, which is transaction-scoped.
- **CI's `check-no-sql-logic.sh`.** The step-2 guard uses a failing insert, not a trigger.
- **Alternative B, not recommended:** squash into 0001. It is simpler, but it breaks "never edit an applied migration" for developer databases. Only take it if the owner explicitly waives the rule pre-release.

## 5a. Projected score of the target

This is the same rubric (v1.2) and formula, applied to the 0002 target (`schema.sql`: 34 tables, 71 indexes) by the same mechanical checks the audit used. The M4/M5 delta (`schema-planned.sql`) is graded when its migrations exist.

**Tables (34).**

| Criterion | Result | Note |
|---|---|---|
| 1, 2, 3, 4, 7, 8, 10, 11, 13 | 🟢 | |
| 6, JSON / lists | 🟢 (p = 33/34) | `template_json` and the parser string are gone. `issues_json` (local staging) remains. |
| 9, FKs for real relationships | 🟡 (p = 31/34) | `change_log`, `field_clock` and `sync_conflict` are generic arcs by design (ADR-0005, T-21) |
| 12, data / metadata | 🟢 (p = 33/34) | kinds are now rows. `local_setting` remains. |
| 5 | n/a | |

Base = (11 × 2 + 1) / 24 × 10 = **9.6**.

**Indexes (71).** Criterion 1 🟢 (0 PK duplicates), criterion 2 🟢 (0 prefix duplicates), criterion 4 🟢 (38/38 FKs lead a non-partial index). Criteria 3, 5, 6 and 7 are n/a or have no query in scope. Base = **10.0**.

**Database.** (34 × 9.6 + 71 × 10.0) / 105 = **9.9**. No Critical or High finding is open, so no cap applies.

**Read this number carefully:**

- **The tables score (9.6) is the better measure of model quality.** The database score weights by object count, and 71 indexes outweigh 34 tables.
- **It holds only if the T-21 deviation stands.** The rubric grades generic arcs High. The audit downgraded the sync-metadata arcs to Low, because they are replication bookkeeping (ADR-0005), not business relationships. An auditor who rejects that would cap the score at 6.0. The deviation is documented in tables.md.
- **Two index criteria aren't graded yet.** Sargability and composite order enter the base once M1 has queries, and can lower it until dbquery reviews them.
- **A 9.9 alone doesn't pass the gate.** The gate also needs:
  - the schema in use (after M1);
  - 🔬 flags ≤ 10 %. N-04 is settled by `name_key`, N-05 by Q-06. N-02 and N-03 need dbquery on M1 queries. N-06 needs the attachment decision.

## 6. Open questions

| ID | Question | Blocks |
|---|---|---|
| Q-02 | Should tags and payees also be accent-insensitive, like categories (FR-IM-07)? | Step 3 indexes (R-05) |
| Q-03 | Name collision on sync: rename the higher id + conflict (recommended), or tombstone? | Step 10 |
| Q-04 | Accept ADR-0008 (UUIDv5 for seeded rows)? | Steps 0, 6 |
| Q-05 | Accept declaring the kind rule as a composite FK, which changes docs/03's "not in SQL" wording? | Steps 0, 3 |
| Q-06 | Is the installment plan ↔ purchase entry link 1:1 (N-05)? | `ux_entry_plan` in step 3 |
| Q-07 | Rollover per category or per month (T-16)? | M2 budgets |
| Q-08 | Is a variable bill (amount unknown until it arrives) fully described by the rule's `amount_mode`, so the template legs carry the expected amount? | Step 3 (D-23) |
| Q-10 | Merging categories with colliding budget lines: sum (recommended)? | Step 6 |
| Q-11 | Cycle-breaking: detach the highest-HLC move to the root + `tree_cycle` conflict? | Step 10 |
| N-02, N-03 | Partial-index usage; account-register access path | Index set, re-check in step 11 |
| N-06 | Attachment storage and sync | Not blocking (FR-TX-11 is C) |
| N-07 | Compaction hard-delete cost | Not blocking once the FK indexes exist |

### ADR-0008 draft (for step 0)

> **Context:** reference rows seeded by the app (the default category tree, savings/investment accounts) are created independently on each paired device. Random UUIDv7 ids duplicate them on sync.
>
> **Decision:** seeded rows use `UUIDv5(MRK_SEED_NAMESPACE, seed_key)`. The namespace is a fixed constant in mr-krabs-core and never changes. Seed writes carry `hlc = 0` and `hlc_device = 'seed'`, and are not change-logged. User-created rows keep UUIDv7. A seeded row is ordinary user data afterwards: it can be renamed, moved, merged or deleted.
>
> **Consequences:**
> - Seeding is idempotent and convergent.
> - Any user edit beats the seed.
> - A deleted seed row is never resurrected.
> - A `seed_key` is never reused for different content.

## 7. Acceptance

This proposal needs explicit acceptance before **dbbuild** generates DDL from it, starting with migration `0002`. Changes to its structure go back through dbdesign and dbdiagram, and the revised proposal needs acceptance again.
