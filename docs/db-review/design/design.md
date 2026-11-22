# Mr Krabs — Audited redesign of the SQLite schema

> Mode: **audited redesign**. Inputs: [../analysis/summary.md](../analysis/summary.md) (score 6.0, gate not passed); `crates/mrk-store/migrations/0001_init.sql` (unchanged through PR 14); docs 01, 03, 04, 05, 06 and 09 **at PR 14 (`b660aa1`)**; ADR-0002, ADR-0003 and ADR-0005.
> Engine: SQLite 3.45+ (rusqlite 0.37 `bundled`). Revision 2: 2026-11-22.
>
> **Revision 2** replaces revision 1's two-table category model (`category_group` + leaf `category`). PR 14 made categories a user-owned tree of **any depth**, where **any node** takes postings and budgets (FR-BU-01, FR-BU-08), and added delete = merge (FR-BU-07) and accent-insensitive name matching (FR-IM-07). Revision 2 also adds the schema that PR 14's new requirements need (§5, D-19 to D-21).
>
> **Revision 3** (score push, same day) adds D-22 to D-25:
> - plain FK indexes where the only index was partial;
> - recurring templates as rows instead of `template_json`, plus the FR-BU-06 amount modes, both now in 0002;
> - the import parser split into typed columns;
> - account and entry kinds as reference tables;
> - `account.visibility` with no default.
>
> **Artifacts:**
> - [schema.sql](schema.sql) is the full **0002 target**: 34 tables, 71 indexes.
> - [schema-planned.sql](schema-planned.sql) is the **M4/M5/sync delta**: 7 tables.
> - [negative-tests.sql](negative-tests.sql) holds 28 rejected writes and the accepted edge cases.
>
> All three **were executed** together against `sqlite3 :memory:` (SQLite 3.51.0). All 28 negative tests failed as intended; `foreign_key_check` and `integrity_check` are clean.
> Migrations themselves (rebuild steps) are dbbuild's job, after acceptance.

## 1. Scope

Scope is the replicated household ledger schema owned by `mrk-store`. The redesign fixes the audit's High and Medium findings and answers the owner's question:

> **Can we manage categories as data?**

**Yes.** PR 14 now states it as a requirement: no code path names a category, the tree has any depth, every node is editable, deletable and mergeable, and the seed is only a starting point. The schema already stores categories as rows. What it still needs for that promise to hold on two synced devices:

1. **Stable identity for seeded rows.** Two devices must seed the *same* rows (D-03).
2. **A name key that matches FR-IM-07's matching rule.** Siblings must not share a normalized name, or import matching becomes ambiguous (D-05).
3. **A declared kind rule.** It holds by construction and survives merges because `kind` is immutable (D-01).
4. **Merge rules for a replicated tree:** concurrent moves that form a cycle (D-17), and late references to a merged-away category (D-18).
5. **A clear boundary.** Categories, tags, payees and rules are *user data*. Account kinds, entry kinds, leg types and statuses are *code-owned behaviour* and stay `CHECK` lists (D-03, last bullet).

## 2. Assumptions

These are commitments the design makes without confirmation. Each one has a matching open question in §3.

- **A-01** No production database holds rows yet (pre-M1). 0002 can rebuild tables, but still copies rows so that it stays correct on developer databases.
- **A-02** One database holds exactly one household (implicit in 0001).
- **A-03** A category's `kind` never changes after creation. To change it, the user creates a new category and merges into it. This is what makes the composite kind FK safe under concurrent moves.
- **A-04** `journal_entry.installment_plan_id` marks only the *purchase* entry, so the link is 1:1 (N-05).
- **A-05** Seed names are written in the household's locale at creation or adoption, and are user data from then on.
- **A-06** `name_key = normalize(name)`: trim, lowercase, strip combining accents and collapse inner whitespace. It is written by one `mrk-domain` function, the same one import matching uses (FR-IM-07).
- **A-07** Tags and payees keep ASCII `NOCASE` uniqueness. FR-IM-07's accent-insensitivity is specified for categories only.

## 3. Open questions

| ID | Question | Blocks | Default if unanswered |
|---|---|---|---|
| Q-01 | ~~Is "Inversiones y ahorros" a category?~~ **Answered by PR 14:** seeded as accounts. | — | — |
| Q-02 | ~~Accent-insensitive names?~~ **Answered by FR-IM-07 for categories.** Should tags and payees follow (A-07)? | D-05 for tag/payee | `NOCASE` |
| Q-03 | Sync collision on a name key: rename the higher id "Name (2)" plus a `duplicate_name` conflict (recommended), or tombstone it? | D-05, merger (M5) | Rename + conflict |
| Q-04 | Accept an ADR amendment allowing **UUIDv5** for seeded rows, against "IDs are UUIDv7"? | D-03 | Required; ADR-0008 drafted in the proposal |
| Q-05 | docs/03 says kind and cycle checks run "in mrk-domain … not in SQL". Accept the **composite FK** that also declares the kind rule (defence in depth, still no SQL logic)? | D-01 | Declare it |
| Q-06 | Installment-plan link cardinality (A-04, N-05). | D-06 `ux_entry_plan` | 1:1 |
| Q-07 | Is budget rollover a category attribute (FR-BU-04 "per category toggle") or per month (T-16)? | M2 | Unchanged |
| Q-08 | ~~`template_json` → template rows?~~ **Decided in rev. 3** (D-23). Is a template with only some legs fixed (e.g. a variable bill amount) satisfied by `amount_mode` on the rule? | D-23 | Yes |
| Q-09 | ~~Default visibility?~~ **Decided in rev. 3:** no default, and Rust sets visibility per scope (D-12). | — | — |
| Q-10 | Merging two categories that both have a budget line for the same period/scope: sum them (recommended) or keep the target's? | FR-BU-07 command | Sum |
| Q-11 | Cycle-breaking rule (D-17): detach the node whose `parent_id` write has the highest `(hlc, device)` to the root, plus a `tree_cycle` conflict? | Merger | Yes |

## 4. Requirements dossier (category-relevant extract)

### 4.1 Business rules

Kinds: F = fundamental, P = policy. Sources are at PR 14.

| ID | Rule | Kind | Source |
|---|---|---|---|
| BR-01 | A category has one kind (income or expense) and at most one parent. Roots have no parent. | F | FR-BU-01 |
| BR-02 | The tree has any depth, and any node can take postings and budget lines. | F | FR-BU-01 |
| BR-03 | A child's kind equals its parent's. Kind never changes (A-03). | F | docs/03, A-03 |
| BR-04 | There are no cycles. | F | docs/03 |
| BR-05 | Live siblings (same kind and parent) have distinct normalized names. Live tags and payees have distinct names. | P | FR-IM-07, T-02 |
| BR-06 | The household starts with a default tree that is ordinary, editable data. No code depends on a specific category. | F | FR-BU-01, docs/03 |
| BR-07 | Both paired devices hold the same seeded tree, with no duplicates. | F | docs/05 convergence |
| BR-08 | A new app version may add seed categories without resurrecting deleted ones. | P | from BR-06/07 |
| BR-09 | Delete = merge. References move to a chosen category of the same kind, then the category is tombstoned. Late references follow the merge. | F | FR-BU-07 |
| BR-10 | Archived categories stay in history and reports and are hidden from pickers. | F | FR-BU-07 |
| BR-11 | Reports and budgets roll up through descendants. | F | FR-BU-08 |
| BR-12 | Transfers have no category leg. Savings and investments are accounts. | F | FR-TX-02, docs/01 seed note |
| BR-13 | FX exchange balances per currency through FX-trading legs. | F | FR-CU-04, docs/03 |
| BR-14 | An account leg's currency equals its account's. | F | docs/03 |
| BR-15 | Reconciliation clears account legs of the reconciled account, and reconciled transactions are locked until unlocked. | F | FR-AC-06 |
| BR-16 | Shares sum to the expense and carry a currency. | F | FR-HH-04, ADR-0003 |
| BR-17 | Credit-card attributes exist only on credit cards. | F | FR-AC-05 |
| BR-18 | Import resolves categories by normalized name. Nothing is committed with an unresolved category. Mappings are remembered per profile. | F | FR-IM-07 |
| BR-19 | Rules run in stages pre → default → post, by priority. Conditions are ANDed. Actions set category, payee, memo, tags or status. | P | FR-TX-10 |

### 4.2 Volumetrics

The volume is about 60–150 categories, a few hundred payees, tens of rules and about 5,000 entries a year. Every category constraint is effectively free. Roll-up over a tree of 150 nodes is trivial in Rust.

## 5. Decisions

### D-01 — Any-depth adjacency list, with the kind rule declared

*Traces to: T-03, BR-01–04, Q-05.*

`category(id, parent_id, kind, …)` with `UNIQUE (id, kind)` and `FOREIGN KEY (parent_id, kind) REFERENCES category(id, kind)`, plus `CHECK (parent_id <> id)`.

- BR-03 is held by the engine (tier 3). Because `kind` is immutable (A-03), no concurrent move can break the rule: a move only ever targets a parent of the same kind, and kinds never change.
- Revision 1's group/leaf split is **withdrawn**, because BR-02 now forbids it.
- The adjacency list is the default encoding for trees (physical-design §5.4). Roll-ups (FR-BU-08) run in Rust over the loaded tree. No closure table is needed at this size.

### D-03 — Seed contract: deterministic ids + `seed_key`

*Traces to: T-02, BR-06–08.*

- The default tree is a **catalogue in Rust** (`mr-krabs-core::seed`): `{seed_key, parent_key, kind, names{es-CL,en}, icon, sort}`. docs/01's table is a copy of it, and savings/investment *accounts* live in the same catalogue (docs/01 seed note).
- A seeded row's id is `UUIDv5(MRK_SEED_NAMESPACE, seed_key)`. Every device computes the same id, so seeding before or after pairing converges (BR-07). This needs the `uuid` `v5` feature and an ADR amendment (Q-04).
- `seed_key` is `UNIQUE` and NULL for user rows. Seeding uses `INSERT … ON CONFLICT(id) DO NOTHING`, so a tombstoned seed row stays deleted (BR-08). The seeder skips a key whose `name_key` collides with a live user sibling.
- **When:** `household_create` and household adoption. Seed rows have `hlc = 0` and `hlc_device = 'seed'`, and are not change-logged, because each device produces them itself. Any user edit wins LWW.
- **Not in a SQL migration:** a migration can't know the locale, would make the seed uncorrectable, and goes against ADR-0002's "SQL is storage". The same contract seeds `currency`, whose natural PK already converges.
- **Data vs code:** categories, tags, payees, rules and currencies are **user data** (synced tables).
  - **Account and entry kinds** are code-owned, but the business can grow them. Rev. 3 stores them as **reference tables installed by migrations** (D-24).
  - **Closed, two- to four-value domains** stay `CHECK` lists: `posting.leg`, statuses, sources, extractors, rule stages.

### D-04 — Leg currency = account currency, declared

*Traces to: T-05, BR-14.*

`account UNIQUE (id, currency_code)`, with composite FKs from `posting`, `installment_plan` and `account_valuation`.

### D-05 — Alternate keys on reference data

*Traces to: T-02, BR-05, FR-IM-07.*

- **Categories:** `ux_category_sibling_name (kind, ifnull(parent_id,''), name_key) WHERE deleted = 0`. `name_key` is controlled redundancy: SQLite has no accent stripping, and UDFs are banned (ADR-0002). One Rust function writes it, and a test asserts `name_key = normalize(name)` for every row.
- **Tags, payees and import profiles:** `(name COLLATE NOCASE) WHERE deleted = 0`.
- **Sync collision rule** (Q-03): if a remote row breaks a name key on apply, the higher-id row is renamed "Name (2)" by a local write and `sync_conflict(kind = 'duplicate_name')` is raised. Both devices compute the same outcome.

### D-06 — `journal_entry.kind` holds intent only

*Traces to: T-06, T-24.*

The values are `regular`, `opening_balance`, `adjustment`, `settlement`, `installment_purchase` and `fx_exchange`. Expense, income, transfer and split are derived by `classify()`, which ignores FX legs. `ux_entry_plan` plus a `CHECK` make the purchase link 1:1.

### D-07 — Drop `journal_entry.is_shared`

*Traces to: T-07.*

"Shared" ⇔ the entry has live `entry_share` rows.

### D-08 — Typed household settings

*Traces to: T-11.*

`household.base_currency_code → currency`, `budget_mode` and `locale` replace `household_setting`.

### D-09 — Credit-card subtype `CHECK`

*Traces to: T-12.*

### D-10 — `account_valuation`

*Traces to: T-13, FR-AC-07, in schema-planned.sql.*

One table for "a balance asserted at a date": manual asset values (FR-AC-07, M4) and the partner's `shared_summary` snapshots (docs/05, `source = 'partner_summary'`). It is keyed by (account, date) and carries the account's currency through the composite FK. This consolidates revision 1's `account_balance_snapshot`.

### D-11 — Reconciliation state per account leg

*Traces to: T-08, BR-15.*

`posting.status` and `posting.reconciliation_id` exist on account legs only, with a composite FK to the *same* account's reconciliation. FR-AC-06's lock is derived: an entry is locked iff any of its legs is `reconciled`. Unlock sets that leg back to `cleared`. No extra column is needed.

### D-12 — Small integrity guards

*Traces to: T-17–T-20, T-22, T-25.*

- date `GLOB`s
- recurring min ≤ max and a currency when amounts are set
- `payee_rule` must set something
- the `sync_conflict.kind` domain, now with `unbalanced_entry`, `duplicate_name` and `tree_cycle`
- `UNIQUE device.public_key`
- T-25: `account.visibility` has **no default** (rev. 3). `account_create` always sets it, and household accounts use `shared_full`.

### D-13 — Deferred and accepted

- **Deferred:** rollover (T-16, Q-07).
- **Accepted:** generic arcs in sync tables (T-21, ADR-0005) and single household (T-23).

### D-14 — FX-trading leg

*Traces to: T-01, BR-13.*

`posting.leg ∈ {account, category, fx}`, with the arc `CHECK` extended. An FX leg's target is its currency. System trading accounts were rejected because they would pollute account lists and net worth.

### D-15 — Currency on `entry_share`

*Traces to: T-09.*

### D-16 — FK indexes and apply order

*Traces to: I-01, T-14.*

Every transactional FK leads an index. The merger applies batches under `PRAGMA defer_foreign_keys = ON`.

### D-17 — Cycle-breaking for concurrent moves

*Traces to: T-26, BR-04. Merger rule, no DDL.*

After applying a batch, the merger walks the `parent_id` chain of every category whose `parent_id` changed. On a cycle, it detaches the cycle member whose `parent_id` field clock `(hlc, device)` is highest, making it a root of its kind, and raises `sync_conflict(kind = 'tree_cycle')`. The input is the converged `field_clock`, so both devices pick the same node. The local Rust check (BR-04) still rejects cycles on every user write.

### D-18 — `category.merged_into_id`

*Traces to: T-27, BR-09.*

A category deleted by merge records its target: `CHECK (merged_into_id IS NULL OR (deleted = 1 AND merged_into_id <> id))`, with a composite FK `(merged_into_id, kind)` so a merge stays within its kind.

- When a posting, budget line, rule action or mapping arrives referencing a merged category, the merger follows `merged_into_id` to the live end and re-points the row with a local write.
- Chains are resolved transitively. A chain that ends in a non-merged tombstone raises `edit_vs_delete`.
- Budget lines that collide on the merge target are summed (Q-10).

### D-19 — Rules engine

*Traces to: FR-TX-10, docs/03 planned additions, T-29; in schema-planned.sql for M5.*

`rule(stage, priority, is_enabled, origin)`, `rule_condition(field, op, typed value columns)` and `rule_action(action, typed target columns)`.

- Every referenced payee, account, category or tag is a declared FK. There is no JSON, which avoids repeating T-10.
- A per-field `CHECK` requires exactly the value columns its `field`/`op` needs.
- "One of" is several condition rows. Learned rules are `origin = 'learned'` (FR-IM-08 lists them).
- The M5 migration copies `payee_rule` into these tables and drops it.
- The local categorizer (FR-TX-12) needs no table, as docs/03 says. The design agrees: it is derived from the ledger, and storing it would be uncontrolled redundancy.

### D-20 — Import staging

*Traces to: FR-TX-07, FR-IM-06, FR-IM-07, docs/06, T-28, T-29; in schema-planned.sql for M5.*

- `import_row` gains `source_category` (the file's text), `suggestion_source` (`file`, `rule`, `categorizer` or `ai`, per docs/06 Enrich) and `transfer_account_id` (workbook savings rows become transfers).
- `import_row_candidate (row_id, entry_id, score)` holds one row per duplicate candidate.
- `import_budget_row` stages the workbook's planned values.
- `import_category_mapping (profile_id, source_name_key → category_id)` stores remembered mappings. It is replicated with its profile, and is unique per profile among live rows.
- `import_batch.extractor` gains `workbook` (a table rebuild).

### D-21 — Recurring amounts

*Traces to: FR-BU-06; moved into 0002 in rev. 3.*

`amount_mode` (`exact`, `approximate` or `range`), `expected_minor`, `tolerance_pct` and `expected_min/max_minor` store what the user typed. One `CHECK` requires exactly the columns each mode needs, and a currency whenever there is an amount. Rust derives the accepted band.

### D-22 — Plain indexes for FKs covered only by partial indexes

*Traces to: I-01. Rev. 3.*

SQLite can't use a partial index (`WHERE deleted = 0`) for the FK lookup a parent delete triggers. Four FKs had only partial coverage, so each now gets a plain index:

- `posting(category_id)`
- `entry_share(entry_id)`
- `installment(plan_id)`
- `journal_entry(installment_plan_id)`

Every one of the 38 transactional FKs now leads a usable index.

### D-23 — Recurring templates as rows

*Traces to: T-10. Rev. 3.*

`recurring_rule_posting` holds the template legs: the same `leg` arc as `posting` and the same composite currency FK, but no status. `recurring_rule_share` holds the template shares, with the same arc as `entry_share`. The rule itself gains `payee_id`, `description`, `payer_member_id` and `attributed_member_id`. `template_json` is gone, so every reference a template makes is a declared FK, and archiving or merging a category reaches templates too (D-18 redirect). Posting an occurrence copies the template rows into `journal_entry` + `posting` + `entry_share`. The "≥ 2 legs, balanced" rule is checked by the same `validate_balanced()`.

### D-24 — Account and entry kinds as reference tables

*Traces to: T-15. Rev. 3.*

`account_kind(code, sort_order)` and `entry_kind(code, sort_order)` are installed and extended **only by migrations**: an `INSERT` in the migration that ships the Rust code for the new value. They are identical on every device at the same `user_version`, which docs/05 already requires for sync, so they carry no sync columns.

- `account.kind` and `journal_entry.kind` become FKs to them.
- **Gain:** a new kind no longer needs SQLite's 12-step rebuild of `account` or `journal_entry`, two tables that 10+ FKs point at.
- **Guard:** the Rust enum stays the single source of behaviour. A `mr-krabs-core` test asserts that enum and table match in both directions.
- **No behaviour flags:** the tables deliberately hold no flags (asset vs liability, in cash flow, …), because those would duplicate Rust.

### D-25 — Typed import parser

*Traces to: no separate finding id. The audit counts `import_profile.parser` against tables criterion 6 (tables.md checklist). Rev. 3.*

`parser_kind ∈ {mapping, workbook, builtin, ai}`, with `bank` and `doc_type` required for `builtin` only. `mapping_json` is required for `mapping`, and `workbook` requires `xlsx`. The intelligent string `'builtin:<bank>:<doc>'` is gone. `import_batch.extractor` gains `workbook` in 0002 (T-28), so the M5 rebuild is no longer needed.

## 6. Conceptual model (Barker), changed and new relationships

Unchanged relationships are as recovered in [../analysis/tables.md](../analysis/tables.md#relationships-barker-both-directions).

- Each CATEGORY **may be** placed under **one and only one** CATEGORY. Each CATEGORY **may be** the parent of **one or more** CATEGORYs. *(recursive, optional at both ends; same kind, declared)*
- Each CATEGORY **may be** merged into **one and only one** CATEGORY. Each CATEGORY **may be** the target of **one or more** merged CATEGORYs. *(only when deleted)*
- Each POSTING **may be** booked to **one and only one** ACCOUNT, or **may be** booked to **one and only one** CATEGORY, or neither for an FX leg. *(arc A1 on `leg`)* Each ACCOUNT / CATEGORY **may be** booked by **one or more** POSTINGs.
- Each POSTING **may be** cleared by **one and only one** RECONCILIATION. Each RECONCILIATION **may be** the clearing of **one or more** POSTINGs. *(same account, composite FK)*
- Each ACCOUNT VALUATION **must be** of **one and only one** ACCOUNT. Each ACCOUNT **may be** valued by **one or more** ACCOUNT VALUATIONs.
- Each HOUSEHOLD **must be** reported in **one and only one** CURRENCY. Each CURRENCY **may be** the base of **one or more** HOUSEHOLDs.
- Each ENTRY SHARE **must be** denominated in **one and only one** CURRENCY. Each CURRENCY **may be** the denomination of **one or more** ENTRY SHAREs.
- Each JOURNAL ENTRY **may be** the purchase of **one and only one** INSTALLMENT PLAN. Each INSTALLMENT PLAN **may be** recorded by **one and only one** JOURNAL ENTRY. *(1:1, A-04)*
- Each RULE CONDITION **must be** part of **one and only one** RULE. Each RULE **may be** guarded by **one or more** RULE CONDITIONs.
- Each RULE ACTION **must be** part of **one and only one** RULE. Each RULE **must be** the source of **one or more** RULE ACTIONs. *(parent-end must, Rust)*
- Each RULE ACTION **may be** the setter of **one and only one** CATEGORY / PAYEE / TAG. Each of those **may be** set by **one or more** RULE ACTIONs. *(arc on `action`)*
- Each IMPORT ROW CANDIDATE **must be** about **one and only one** IMPORT ROW, and **must be** a match for **one and only one** JOURNAL ENTRY. Each IMPORT ROW **may be** matched by **one or more** IMPORT ROW CANDIDATEs.
- Each IMPORT BUDGET ROW **must be** part of **one and only one** IMPORT BATCH. Each IMPORT BATCH **may be** composed of **one or more** IMPORT BUDGET ROWs.
- Each IMPORT CATEGORY MAPPING **must be** remembered for **one and only one** IMPORT PROFILE, and **must be** resolved to **one and only one** CATEGORY. Each IMPORT PROFILE **may be** remembering **one or more** IMPORT CATEGORY MAPPINGs.

## 7. Dependencies, keys and normal forms (changed and new relations)

| Relation | FDs (natural attributes; F/P) | Candidate keys | NF | Notes |
|---|---|---|---|---|
| category | id → parent_id, kind, name, name_key, …; (kind, parent_id, name_key) → id (P); seed_key → id; name → name_key (F) | id; (kind, parent_id, name_key)*; seed_key | 3NF, controlled redundancy | `name → name_key` has a non-key left side and is written by one Rust function (A-06), checked by a test. `(id, kind)` is a superkey declared `UNIQUE` only as an FK target. |
| posting | id → …; account_id → currency_code; reconciliation_id → account_id | id | 3NF, controlled redundancy | both FDs enforced by composite FKs (D-04, D-11) |
| journal_entry | id → all | id; installment_plan_id* | BCNF | |
| entry_share | id → …; (entry_id, holder, currency) → amount, settled_entry_id | id; (entry, holder, currency)* | BCNF | |
| household | id → name, base_currency_code, budget_mode, locale | id | BCNF | |
| account_valuation | (account_id, as_of_date) → value_minor, source; account_id → currency_code | (account_id, as_of_date) | 2NF, deliberate | currency carried for the composite FK |
| rule / rule_condition / rule_action | id → all | id | BCNF | the value columns are a typed arc, guarded by a `CHECK` |
| import_row_candidate | (row_id, entry_id) → score | (row_id, entry_id) | BCNF | |
| import_category_mapping | (profile_id, source_name_key) → category_id | id; (profile_id, source_name_key)* | BCNF | |

\* among live rows (partial unique index).

## 8. Rule → mechanism map

| Rule | Tier | Mechanism |
|---|---|---|
| BR-01 | 1/3 | `kind CHECK`; nullable self-FK |
| BR-02 | structural | posting and budget FKs reference any category |
| BR-03 | 3 | composite FK `(parent_id, kind)`; `kind` immutable in Rust (A-03) |
| **BR-04 no cycles** | **5** | **Rust check on write + merger D-17 + `tree_cycle` conflict** |
| BR-05 | 2 + merger | partial unique indexes on `name_key` / `NOCASE` + D-05 collision rule |
| BR-06–08 | 5 + 2 | seed catalogue; UUIDv5 + `seed_key UNIQUE`; `ON CONFLICT DO NOTHING` |
| **BR-09 merge** | **5 + 1/3** | **`category_delete` command (one transaction) + `merged_into_id` `CHECK`/FK + merger redirect (D-18)** |
| **BR-10, BR-11** | **5** | **Rust (pickers, roll-up)** |
| **BR-12 transfers have no category leg** | **5** | **Rust `validate()`** |
| BR-13 | 1 | `posting.leg` + arc `CHECK` |
| BR-14 | 3 | composite FK |
| BR-15 | 1 + 3 | status/reconciliation `CHECK`s + composite FK; lock derived |
| **BR-16 shares sum** | **4 → Rust** | **`validate()` + property test** |
| BR-17 | 1 | account `CHECK` |
| **BR-18 import categories resolved** | **5 + 2** | **commit gate in Rust; mapping unique key** |
| **BR-19 rule evaluation** | **5** | **Rust rules engine; shapes held by `CHECK`s** |
| **Ledger balanced per currency; ≥ 2 legs** | **4 → Rust** | **`validate_balanced()` + proptest** |
| **`name_key = normalize(name)`** | **5** | **single writer function + verification test** |

Bold rules are **not** held by the schema.

## 9. Data dictionary (new and changed columns)

| Table.column | Type | Null | Meaning |
|---|---|---|---|
| category.parent_id | TEXT | yes | Parent node of the same kind. NULL = a root of that kind's tree. |
| category.name_key | TEXT | no | `normalize(name)`, the uniqueness and import-matching key (A-06) |
| category.seed_key | TEXT | yes | Stable catalogue key of a seeded row. NULL = user-created. Never edited. |
| category.merged_into_id | TEXT | yes | Set only on a category deleted by merge: where late references go |
| posting.leg | TEXT | no | `account` \| `category` \| `fx` |
| posting.status | TEXT | iff account leg | `pending` \| `cleared` \| `reconciled` for this leg |
| posting.reconciliation_id | TEXT | yes | The statement that reconciled this leg; set iff `reconciled` |
| journal_entry.kind | TEXT | no | User intent; `regular` = classify from the postings |
| entry_share.currency_code | TEXT | no | Currency of `amount_minor` |
| household.base_currency_code / budget_mode / locale | TEXT | no | Former `household_setting` keys, typed |
| account_valuation.source | TEXT | no | `manual` (FR-AC-07) \| `partner_summary` (docs/05) |
| rule.stage / priority / is_enabled / origin | — | no | Evaluation order and provenance (FR-TX-10, FR-IM-08) |
| rule_condition.* value columns | — | per `field` | Exactly the columns the field/op needs (`CHECK`) |
| import_row.source_category / suggestion_source / transfer_account_id | TEXT | yes | File's category text; where the suggestion came from; savings-row transfer target |
| import_row_candidate.score | TEXT | no | Decimal 0..1 match score (FR-TX-07) |
| import_category_mapping.source_name_key | TEXT | no | `normalize(file text)` remembered per profile |
| recurring_rule.amount_mode / expected_minor / tolerance_pct | — | yes | What the user typed (FR-BU-06) |

## 10. Audit traceability

| Finding | Resolution |
|---|---|
| T-01 High | D-14 |
| T-02 High | D-03, D-05 |
| T-03 | D-01, D-17 |
| T-04 | resolved in the docs (PR 14) |
| T-05 | D-04 |
| T-06 | D-06 |
| T-07 | D-07 |
| T-08 | D-11 |
| T-09 | D-15 |
| T-10 | D-23 |
| T-11 | D-08 |
| T-12 | D-09 |
| T-13 | D-10 |
| T-14 | D-16 |
| T-15 | D-24 |
| T-16 | deferred (Q-07) |
| T-17–T-20, T-22, T-25 | D-12 |
| T-21, T-23 | accepted |
| T-24 | D-06 |
| T-26 | D-17 |
| T-27 | D-18 |
| T-28 | D-20, D-25 |
| T-29 | D-10, D-19, D-20, D-21 |
| I-01 | D-16, D-22 |
| `import_profile.parser` (audit criterion 6) | D-25 |
| I-02 | unchanged (Low) |

**Behaviour changes** (intentional):

- Entry status becomes derived.
- `kind` no longer stores expense/income/transfer.
- Duplicate sibling names are rejected.
- Seeded rows have deterministic ids.
- A category's kind is immutable.
- `payee_rule` is replaced in M5.

## 11. Validation checklist

- [x] Every table has a PK. Every reference is a declared FK to a PK or a declared `UNIQUE`.
- [x] Every nullable column has one documented meaning (§9).
- [x] Changed relations are BCNF, or 3NF/2NF with documented, engine- or test-enforced redundancy (§7).
- [x] Tier-4/5 rules are named with their Rust owner (§8).
- [x] The DDL runs top to bottom on SQLite 3.51 (`:memory:`): schema.sql, then schema-planned.sql. All 28 negative tests are rejected. These are accepted: depth-3 trees, postings on parent nodes, the `workbook` extractor, a builtin profile, an approximate-amount rule with template legs and shares.
- [x] Every transactional FK leads a non-partial index (38/38). No index duplicates a PK or prefixes another.
- [ ] A test asserts that the `AccountKind` / `EntryKind` Rust enums equal the `account_kind` / `entry_kind` rows.
- [ ] Owner answers Q-02–Q-11.
- [ ] Merger rules D-05, D-17 and D-18 covered by the docs/05 convergence property test.
- [ ] `classify()` ignores FX legs; `Leg` gains `FxTrading` (mrk-domain).
- [ ] A test asserts `name_key = normalize(name)` for every category row.
