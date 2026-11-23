# mrkrabs (SQLite) — Tables evaluation

> Evaluated against dbaudit/tables.md (rubric v1.2).
> Source: `crates/mrk-store/migrations/0001_init.sql` at commit `f1ff160` (2026-11-19). Engine: SQLite 3 (`STRICT` tables, inferred from syntax).
> Coverage: 31 of 31 tables deep-read (file is 384 lines, below the triage threshold). Cross-read: `docs/01-requirements.md`, `docs/03-data-model.md`, `docs/04`, `docs/05-sync.md`, `docs/06`, `docs/09-prior-art.md`, `crates/mrk-domain/src/ledger.rs`.
> **Revision 2 (2026-11-22):** re-read against the requirements merged in PR 14 (`b660aa1`, same docs as `dev` `de5b7f3`). The DDL is unchanged, so every finding still stands on `0001_init.sql`. The changes are: T-03 is restated (FR-BU-01 now allows any depth and postings on any node), T-04 is resolved in the docs, and T-25 to T-29 are new.
> Context: greenfield schema (pre-M1). No repository code writes it yet, so every finding is about what the schema *allows*, not about data observed.

## Strengths

- **Every table is `STRICT`**, so a value of the wrong storage class is rejected instead of being coerced. This is SQLite's only real column typing.
- **Money is `INTEGER` minor units plus a currency** (ADR-0003) wherever the currency can't be inferred from a parent row. There are no `REAL` columns anywhere.
- **Exclusive arcs are declared**: `posting` has exactly one of `account_id` or `category_id` (line 161), and `entry_share` has exactly one of `member_id` or `participant_id` (line 183).
- **Tuple rules are declared as `CHECK`s**: scope ⇔ owner on `account` (line 95) and `budget_line` (line 258); visibility vs scope (line 96); `base_code <> quote_code` (line 30).
- **Uniqueness respects tombstones**: the partial unique indexes use `WHERE deleted = 0`, and the `ifnull()` expression keys (lines 185, 260) make nullable members of a key comparable.
- **Sync columns are uniform** (`hlc`, `hlc_device`, `deleted` with a 0/1 check) on every replicated table. Local-only tables are labelled in comments.
- **There is no SQL logic** (no triggers, views or UDFs, per ADR-0002), and a test checks this (`crates/mrk-store/src/lib.rs:178`).
- `field_clock` is `WITHOUT ROWID` with its natural composite PK. `change_log` pairs an `AUTOINCREMENT` local order with a globally unique `change_id`.

## Findings

| ID | Object | Anti-pattern | Conf. | Theory violated | Severity | Evidence (line) |
|---|---|---|---|---|---|---|
| T-01 | `posting` | **The ledger's FX-trading leg can't be stored.** The documented model has three leg targets (`Leg::FxTrading(CurrencyCode)`, and the `fx:CLP` / `fx:USD` legs of an `fx_exchange` entry), but the `CHECK` allows only account XOR category. The only storable workaround is a pseudo-category, which the domain tests already use (`Posting::category("fx", …)`). A category's `kind` is `income` or `expense`, so FX legs would enter `classify()` and cash flow as income or expense. | ✅ | Arc completeness (er-modeling §A2.5); domain integrity (§7); SSOT between documented model and schema (§6) | **High** | 0001_init.sql:155-161; docs/03-data-model.md:56,79; crates/mrk-domain/src/ledger.rs:187-192 |
| T-02 | `category`, `tag`, `payee` | **Surrogate key with no alternate key on reference entities.** A category's natural identifier, (parent, name), carries no `UNIQUE`, and neither does `tag.name` or `payee.name`. The same category can be stored twice under two UUIDs. This is likely, not hypothetical: if each device seeds the default tree independently, pairing doubles every category. | ✅ structural / ⚠️ natural key | Entity integrity of the thing (theory §2.1) | **High** | 0001_init.sql:100-123 |
| T-03 | `category` | **Tree rules not enforced in the schema.** (a) A child's `kind` can differ from its parent's. A composite FK could declare this, but docs/03 places it in `mrk-domain` only. (b) Cycles are storable. *(Rev. 2: the former part (c), "postings on a group", is withdrawn. FR-BU-01 now allows any depth and postings or budgets on any node.)* | ⚠️ | Model rule with no enforcement (§7.1); recursive relationship rules (er-modeling §A2.2) | Medium | 0001_init.sql:102-103; docs/03-data-model.md "Categories are data" |
| T-04 | `category` (seed contract) | ~~The seed tree has a group the table can't represent ("Inversiones y ahorros").~~ **Resolved in the docs** by PR 14: those rows are now seeded as savings/investment accounts. | ✅ | — | Resolved | docs/01-requirements.md (seed note) |
| T-05 | `posting` ↔ `account` | **The currency of an account leg isn't tied to the account's currency.** `validate()` requires `posting.currency_code = account.currency_code`. That rule is cross-row but declarable: `UNIQUE(id, currency_code)` on `account` plus a composite FK from `posting(account_id, currency_code)`. | ✅ | Referential integrity / integrity independence (Codd rule 10, §7.1) | Medium | 0001_init.sql:81,155,158; docs/03-data-model.md:95 |
| T-06 | `journal_entry.kind` | **Derived classification is stored next to its source.** `expense`, `income` and `transfer` are derivable from the postings by `classify()`, while `opening_balance`, `settlement`, `fx_exchange` and `installment_purchase` are user *intent*. One column mixes both, so `kind='transfer'` with a category leg is storable. | ⚠️ | Stored derived attribute (er-modeling §A1.4); SSOT (§6) | Medium | 0001_init.sql:133-134; ledger.rs:110-127 |
| T-07 | `journal_entry.is_shared` | **Redundant flag.** "Is shared" is equivalent to "has live `entry_share` rows", so `is_shared=1` with no shares, or shares with `is_shared=0`, are both storable. | ⚠️ | Uncontrolled redundancy (§6, §7) | Medium | 0001_init.sql:139,175-185 |
| T-08 | `journal_entry.status`, `reconciliation` | **Reconciliation is per account, but status is per entry.** A transfer between checking and a card can be reconciled on one statement and not the other. `status` depends on (entry, account), that is the posting, not the entry. Nothing links a `reconciliation` to the postings it cleared. | ⚠️ | Functional dependency on the wrong determinant (§2, §3); unmodelled relationship | Medium | 0001_init.sql:135,272-279 |
| T-09 | `entry_share.amount_minor` | **Money with no currency.** An entry can be multi-currency (USD card sections, `fx_exchange`), so a share amount's currency is undetermined. | ⚠️ | Domain integrity: Money = (minor, currency) (ADR-0003, §7) | Medium | 0001_init.sql:180 |
| T-10 | `recurring_rule.template_json` | **Hidden foreign keys in JSON.** The serialized `NewEntry` template holds account, category and payee IDs the database can't check. Archiving or tombstoning a category leaves templates dangling. | ⚠️ | 1NF, atomic attributes (§3); relationship by convention (§7) | Medium | 0001_init.sql:229 |
| T-11 | `local_setting`, `household_setting` | **Key/value settings bag (EAV-lite).** `base_currency` is a currency reference with no FK, and `budget_mode` is a closed domain with no check. Values are untyped `TEXT`. | ⚠️ | Data/metadata separation (§10) | Medium | 0001_init.sql:35-45 |
| T-12 | `account` | **Hidden subtype.** `credit_limit_minor`, `statement_day`, `due_day` and `linked_account_id` are credit-card attributes, but no `CHECK` ties them to `kind = 'credit_card'`. | ⚠️ | Hidden subtypes in a single table (er-modeling §A3.2 step 6); NULL as "not applicable" (§3.2) | Medium | 0001_init.sql:79-91 |
| T-13 | (missing table) | **Unmodelled sync artefact.** The sync design emits `account_balance_snapshot` changes for `shared_summary` accounts, but no table receives them on the partner's device. | ✅ | Completeness of the model against the requirements (er-modeling §A0) | Medium | docs/05-sync.md:86; FR-PV-01 |
| T-14 | all FKs | **Immediate (non-deferrable) FKs make sync apply depend on order.** There is a cycle `journal_entry.installment_plan_id` ↔ `installment.paid_entry_id`, plus the self-reference `category.parent_id`. A batch has to be applied in topological order or it fails. | ⚠️ | Atomicity of the multi-row apply (§5); constraint timing (§7.1) | Medium | 0001_init.sql:142,214,102; docs/05-sync.md:112 |
| T-15 | `account.kind`, `journal_entry.kind` | **Volatile value lists frozen in a `CHECK` and duplicated in Rust enums.** SQLite can't `ALTER` a `CHECK`. Adding a value such as a prepaid card or APV means a 12-step rebuild of a table that 10+ FKs reference. | ⚠️ | Data/metadata separation (§10) | Medium | 0001_init.sql:79-80,133-134 |
| T-16 | `budget_line.rollover` | **2NF candidate.** FR-BU-04 calls rollover a *per-category* toggle, yet it is stored on every (period, category, scope, member) row: it depends on part of the alternate key. Whether that is wrong depends on whether rollover can vary by month. | ⚠️ | 2NF, partial dependency (§3) | Low | 0001_init.sql:256; docs/01-requirements.md:113 |
| T-17 | 14 date/month columns | Civil dates (`YYYY-MM-DD`) and months (`YYYY-MM`) are `TEXT` with no format `CHECK`. Ordering and range scans assume the format. | ✅ | Domain integrity (§7) | Low | 0001_init.sql:26,132,198,203,212,250,268,275,302-305 |
| T-18 | `recurring_rule` | No `CHECK (expected_min_minor <= expected_max_minor)`, and nothing requires `currency_code` when an amount is present. | ✅ | Domain integrity (§7) | Low | 0001_init.sql:226-228 |
| T-19 | `payee_rule` | A rule that sets nothing is storable: both `set_payee_id` and `set_category_id` can be NULL. | ✅ | Model rule with no enforcement (§7.1) | Low | 0001_init.sql:241-242 |
| T-20 | `sync_conflict.kind` | The domain lives only in a comment (3 values), and the sync doc adds `unbalanced_entry`, so it has already drifted. | ✅ | Domain integrity (§7) | Low | 0001_init.sql:379; docs/05-sync.md:112 |
| T-21 | `change_log`, `field_clock`, `sync_conflict` | **Generic arc** (`entity`, `entity_id`). The rubric grades this High. Downgraded deliberately: this is replication metadata about rows (ADR-0005), not a business relationship, and it has to outlive the rows it describes. Recorded so the deviation is visible. | ✅ | Data/metadata (§10) | Low (deviation noted) | 0001_init.sql:348-349,360-362,376-377 |
| T-22 | `device.public_key` | Not `UNIQUE`: the same pinned key can be trusted twice under two device IDs. | ✅ | Entity integrity (§2.1) | Low | 0001_init.sql:67 |
| T-23 | `household` | A multi-row table in a database that is implicitly single-household: accounts, categories and settings carry no `household_id`. | ⚠️ | Predicate mismatch (er-modeling §A0) | Low | 0001_init.sql:49-53 |
| T-24 | `journal_entry.installment_plan_id` | The intent is probably 1:1 (one purchase entry per plan), but there is no `UNIQUE`. Cardinality is 🔬. | ⚠️ | Mis-mapped 1:1 (er-modeling §A3.2 step 4) | Low | 0001_init.sql:142 |
| T-25 | `account.visibility` | **The default contradicts the scope `CHECK`.** The default `'shared_summary'` fails `CHECK (visibility = 'shared_full' OR scope = 'personal')` for every household account. Found while executing the target design. | ✅ | Domain integrity (§7) | Low | 0001_init.sql:84,96 |
| T-26 | `category.parent_id` under sync | **A tree edited by LWW can form a cycle that no single device wrote.** Device A moves X under Y while B moves Y under X. Each device passes the Rust cycle check, and the merged `parent_id` values form a loop. Any depth (FR-BU-01) makes concurrent moves likely. No merge rule exists in docs/05. | ✅ | Isolation, write skew across replicas (§5.2); recursive relationship rules (er-modeling §A2.2) | Medium | 0001_init.sql:102; docs/03 "Categories are data"; docs/05-sync.md:108-115 |
| T-27 | `category` delete = merge | **Late references to a merged category have nowhere to go.** FR-BU-07 re-points postings and budgets, then tombstones the category, in one local transaction. A posting the partner created offline in that category still arrives afterwards and references a tombstone. The row stores no `merged_into` target, so the merger can't redirect it deterministically. | ✅ | SSOT / referential integrity over replicas (§6, §7) | Medium | 0001_init.sql:100-109; docs/01 FR-BU-07; docs/04 `category_delete` |
| T-28 | `import_batch.extractor` | **The value list has already drifted.** docs/04 adds `"workbook"` (FR-IM-06), but the `CHECK` allows only `mapping`, `builtin` and `ai`. | ✅ | Data/metadata separation (§10) | Low | 0001_init.sql:299; docs/04 `import_stage` |
| T-29 | (missing tables and columns) | **Requirements added in PR 14 that the schema doesn't model yet.** These are expected for later milestones, and forward migrations are planned. Manual valuations (FR-AC-07); match candidates with scores (FR-TX-07); the category name read from a file and the suggestion source (FR-IM-07, docs/06 Enrich); staged budget lines from the workbook (FR-IM-06); category mappings remembered per profile (FR-IM-07); the staged rules engine (FR-TX-10); approximate (± %) amounts (FR-BU-06). | ✅ | Completeness of the model (er-modeling §A0) | Low | docs/01 FR-AC-07, FR-TX-07, FR-TX-10, FR-BU-06, FR-IM-06, FR-IM-07 |

## Signal counts

| Anti-pattern | Count |
|---|---:|
| Surrogate PK with no `UNIQUE` (of 25 surrogate-keyed tables) | 19 |
| …of which reference/master entities with a clear natural key | 3 (category, tag, payee) |
| `CHECK … IN (…)` value lists | 22 columns |
| JSON / encoded-string columns | 7 (`template_json`, `mapping_json`, `raw_json`, `issues_json`, `fields_json`, `details_json`, `parser`) |
| Money column with no determinable currency | 3 (`entry_share.amount_minor`, `import_batch.statement_closing_minor` when `account_id` is NULL, `recurring_rule.expected_*` when `currency_code` is NULL) |
| Generic arcs | 3 (sync metadata) |
| Relationships by convention (no FK) | `hlc_device` / `device_id` → device (by design); `household_setting['base_currency']` → currency; `template_json` → account/category/payee |

## Needs-data flags (not scored)

- 🔬 **N-01 usage**: the schema is greenfield and not yet written by any repository code (pre-M1).
- 🔬 **N-04 name collation**: should "Supermercado", "supermercado" and "Supermércado" count as the same category, tag or payee? This decides whether uniqueness needs normalization in Rust or `COLLATE NOCASE` (which folds ASCII only).
- 🔬 **N-05 cardinality** of `journal_entry.installment_plan_id`: one purchase entry per plan, or every installment payment too?
- 🔬 **N-06 attachments**: where the bytes live and whether they sync (`attachment` stores only `sha256`, name and mime).

## Checklist score

| # | Criterion | p | Result |
|---|---|--:|---|
| 1 | Primary key present and minimal | 31/31 = 1.00 | 🟢 |
| 2 | Surrogate key where natural key is volatile | 31/31 = 1.00 (currency ISO code and setting keys are stable) | 🟢 |
| 3 | No magic-value sentinels; correct nullability | 31/31 = 1.00 (`''` defaults for free text are documented, one meaning) | 🟢 |
| 4 | Right-sized types, one type per concept | 28/31 = 0.90 (T-09 plus 2 money-without-currency cases) | 🟡 |
| 5 | No deprecated LOB types | n/a on SQLite, left out of base | — |
| 6 | No polymorphic/CSV/JSON columns or repeating groups | 29/31 = 0.94 (`recurring_rule` T-10, `import_profile.parser`) | 🟡 |
| 7 | Single, consistent collation | 31/31 = 1.00 (BINARY throughout) | 🟢 |
| 8 | No shadow/backup tables | 31/31 = 1.00 | 🟢 |
| 9 | FK declared for real relationships, mapped to model | 24/31 = 0.77 (posting T-01, household_setting, recurring_rule, reconciliation T-08, 3 sync tables T-21) | 🟡 |
| 10 | Consistent naming | 31/31 = 1.00 | 🟢 |
| 11 | Keys irreducible; every candidate key declared | 20/25 surrogate-keyed tables = 0.80 (category, tag, payee, import_profile, device) | 🟡 |
| 12 | Data and metadata separate | 27/31 = 0.87 (two settings bags T-11; `account` and `journal_entry` value lists T-15) | 🟡 |
| 13 | Constraints hold what they say | 31/31 = 1.00 | 🟢 |

Base = (7 × 2 + 5 × 1) / (2 × 12) × 10 = 19 / 24 × 10 = **7.9**. Cap: T-01 and T-02 are open High findings, so the cap is 6.0.

**File score: 6.0** (base 7.9, capped at 6.0 by T-01 / T-02 High).

## Notes

- Most Medium findings are rules the project chose to enforce in Rust (ADR-0002), and that choice is legitimate. Three of them, though, can be declared with plain PK/FK/UNIQUE/CHECK, which ADR-0002 allows: T-03a (kind consistency), T-05 (leg currency) and T-02 (alternate keys). For these, the schema leaves integrity on the table for no gain.
- **Uniqueness under LWW sync is the cross-cutting risk.** Any `UNIQUE` added for T-02 makes the merge path hit constraint violations when two devices create the same name concurrently. The design has to decide how the merger resolves that (rename, merge, or raise a conflict) before the constraint lands.

## Recovered model

### Predicates

- `currency`: Currency *code* is called *name*, printed as *symbol*, has *minor_units* decimals, and is of *kind* (fiat or unit of account).
- `fx_rate`: On *rate_date*, 1 unit of *base_code* was worth *rate* units of *quote_code*, according to *source*.
- `local_setting`: On this device, setting *key* has *value*.
- `household_setting`: In this household, setting *key* has *value*.
- `household`: Household *id* is named *name*.
- `member`: Member *id* of household *household_id* is shown as *display_name* in *color*.
- `device`: Device *id*, used by member *member_id*, is named *name*, authenticates with *public_key*, is *is_local*, was paired at *paired_at* and revoked at *revoked_at*.
- `account`: Account *id*, named *name*, is a *kind* account in *currency_code*, with *scope* and *owner_member_id* and *visibility*, at *institution*, masked *number_mask* … **and, if it is a credit card**, has limit *credit_limit_minor*, statement day *statement_day*, due day *due_day* and linked section *linked_account_id*. That OR clause is the hidden subtype of T-12.
- `category`: Category *id* is an *income* or *expense* node named *name* with icon *icon* under parent *parent_id* (NULL = a root of its kind's tree), archived *is_archived*, ordered *sort_order*. Since Rev. 2, any node may take postings and budgets, so the predicate holds a single meaning.
- `payee`: Payee *id* is named *name* and suggests category *default_category_id*.
- `tag`: Tag *id* is named *name*.
- `journal_entry`: On *entry_date*, transaction *id* of *kind*, *status*, paid to *payee_id*, described *description* / *memo*, shared *is_shared*, paid by *payer_member_id* for *attributed_member_id*, came from *installment_plan_id* / *recurring_rule_id* / *import_batch_id*, with dedup *fingerprint*.
- `posting`: Leg *id* of entry *entry_id* moves *amount_minor* of *currency_code* on account *account_id* **or** category *category_id*, with *memo*. An FX leg has no place in this predicate (T-01).
- `external_participant`: Outside person *id* is named *name*.
- `entry_share`: Of entry *entry_id*, member *member_id* **or** participant *participant_id* bears *amount_minor* (currency unknown, T-09), settled by entry *settled_entry_id*.
- `entry_tag`: Entry *entry_id* carries tag *tag_id*.
- `installment_plan`: On *purchase_date*, a purchase of *total_minor* *currency_code* on account *account_id* was split into *installments* at *interest_rate*, starting *first_due_month*, budgeted *budget_mode*.
- `installment`: Installment *number* of plan *plan_id* is due in *due_month* for *amount_minor* and was paid by entry *paid_entry_id*.
- `recurring_rule`: Rule *id* named *name* of *kind* recurs per *rrule* ± *window_days*, expecting *expected_min_minor*–*expected_max_minor* *currency_code*, produces *template_json*, next on *next_date*, auto-posts *auto_post*.
- `payee_rule`: With *priority*, descriptions matching *pattern* by *match_kind* on account *account_id* (or any account) get payee *set_payee_id* and category *set_category_id*.
- `budget_line`: For *period*, category *category_id* in *scope* (member *member_id*) is budgeted *amount_minor* *currency_code*, rolling over *rollover*.
- `savings_goal`: Goal *id* named *name* targets *target_minor* *currency_code* by *target_date* in account *account_id*.
- `reconciliation`: On *statement_date*, account *account_id*'s statement showed *statement_balance_minor* and reconciling it is *status*.
- `import_profile`: Profile *id* named *name* for bank *bank* reads *format* files with *parser* and *mapping_json*.
- `import_batch`: File *file_name* (*file_sha256*) for account *account_id*, read with profile *profile_id* by *extractor* / *ai_provider*, covering *statement_period_start*–*statement_period_end*, closing at *statement_closing_minor*, is *status*, created *created_at*.
- `import_row`: Row *row_no* of batch *batch_id* (raw *raw_json*) reads as *txn_date*, *description*, *amount_minor* *currency_code*, installment *installment_no*/*installment_total*, suggests *suggested_category_id* / *suggested_payee_id* at *confidence*, may duplicate *duplicate_of_entry_id*, has decision *decision* and issues *issues_json*.
- `attachment`: File *file_name* (*sha256*, *mime*) is attached to entry *entry_id*.
- `change_log`: Change *change_id* (local *seq*), at *hlc* from *device_id*, did *op* on *entity* *entity_id* with *fields_json*, visible to *visibility*, applied *applied_at*.
- `field_clock`: Field *field* of *entity* *entity_id* was last written at *hlc* by *device_id*.
- `peer_state`: The highest change seen from origin *origin_device_id* is *max_hlc*.
- `sync_conflict`: Conflict *id* of *kind* on *entity* *entity_id* with *details_json*, created *created_at*, is *resolved*.

### Relationships (Barker, both directions)

All declared unless marked.

1. Each FX RATE **must be** quoted from **one and only one** CURRENCY (base). Each CURRENCY **may be** the base of **one or more** FX RATEs.
2. Each FX RATE **must be** quoted into **one and only one** CURRENCY (quote). Each CURRENCY **may be** the quote of **one or more** FX RATEs.
3. Each MEMBER **must be** part of **one and only one** HOUSEHOLD. Each HOUSEHOLD **may be** composed of **one or more** MEMBERs.
4. Each DEVICE **must be** used by **one and only one** MEMBER. Each MEMBER **may be** the user of **one or more** DEVICEs.
5. Each ACCOUNT **must be** denominated in **one and only one** CURRENCY. Each CURRENCY **may be** the denomination of **one or more** ACCOUNTs.
6. Each ACCOUNT **may be** owned by **one and only one** MEMBER. Each MEMBER **may be** the owner of **one or more** ACCOUNTs.
7. Each ACCOUNT **may be** a section linked to **one and only one** ACCOUNT. Each ACCOUNT **may be** the parent of **one or more** linked ACCOUNTs.
8. Each CATEGORY **may be** grouped under **one and only one** CATEGORY. Each CATEGORY **may be** the group of **one or more** CATEGORYs.
9. Each PAYEE **may be** defaulted to **one and only one** CATEGORY. Each CATEGORY **may be** the default of **one or more** PAYEEs.
10. Each JOURNAL ENTRY **may be** paid to **one and only one** PAYEE. Each PAYEE **may be** paid in **one or more** JOURNAL ENTRYs.
11. Each JOURNAL ENTRY **may be** paid by **one and only one** MEMBER. Each MEMBER **may be** the payer of **one or more** JOURNAL ENTRYs.
12. Each JOURNAL ENTRY **may be** attributed to **one and only one** MEMBER. Each MEMBER **may be** attributed **one or more** JOURNAL ENTRYs.
13. Each JOURNAL ENTRY **may be** the purchase of **one and only one** INSTALLMENT PLAN. Each INSTALLMENT PLAN **may be** recorded by **one or more** JOURNAL ENTRYs. (🔬 probably one, T-24)
14. Each JOURNAL ENTRY **may be** generated by **one and only one** RECURRING RULE. Each RECURRING RULE **may be** the generator of **one or more** JOURNAL ENTRYs.
15. Each JOURNAL ENTRY **may be** committed from **one and only one** IMPORT BATCH. Each IMPORT BATCH **may be** the source of **one or more** JOURNAL ENTRYs.
16. Each POSTING **must be** a leg of **one and only one** JOURNAL ENTRY. Each JOURNAL ENTRY **may be** composed of **one or more** POSTINGs. (The parent-side *must, two or more* is enforced in Rust only.)
17. Each POSTING **may be** booked to **one and only one** ACCOUNT. Each ACCOUNT **may be** booked by **one or more** POSTINGs. (arc with 18)
18. Each POSTING **may be** booked to **one and only one** CATEGORY. Each CATEGORY **may be** booked by **one or more** POSTINGs. (arc with 17)
19. Each POSTING **must be** denominated in **one and only one** CURRENCY. Each CURRENCY **may be** the denomination of **one or more** POSTINGs.
20. Each ENTRY SHARE **must be** a share of **one and only one** JOURNAL ENTRY. Each JOURNAL ENTRY **may be** split into **one or more** ENTRY SHAREs.
21. Each ENTRY SHARE **may be** borne by **one and only one** MEMBER. Each MEMBER **may be** the bearer of **one or more** ENTRY SHAREs. (arc with 22)
22. Each ENTRY SHARE **may be** borne by **one and only one** EXTERNAL PARTICIPANT. Each EXTERNAL PARTICIPANT **may be** the bearer of **one or more** ENTRY SHAREs. (arc with 21)
23. Each ENTRY SHARE **may be** settled by **one and only one** JOURNAL ENTRY. Each JOURNAL ENTRY **may be** the settlement of **one or more** ENTRY SHAREs.
24. Each ENTRY TAG **must be** about **one and only one** JOURNAL ENTRY. Each JOURNAL ENTRY **may be** tagged by **one or more** ENTRY TAGs.
25. Each ENTRY TAG **must be** the use of **one and only one** TAG. Each TAG **may be** used in **one or more** ENTRY TAGs.
26. Each INSTALLMENT PLAN **must be** charged to **one and only one** ACCOUNT. Each ACCOUNT **may be** charged **one or more** INSTALLMENT PLANs.
27. Each INSTALLMENT PLAN **must be** denominated in **one and only one** CURRENCY. Each CURRENCY **may be** the denomination of **one or more** INSTALLMENT PLANs.
28. Each INSTALLMENT **must be** part of **one and only one** INSTALLMENT PLAN. Each INSTALLMENT PLAN **may be** scheduled as **one or more** INSTALLMENTs. (The parent-side *must* lives in Rust.)
29. Each INSTALLMENT **may be** paid by **one and only one** JOURNAL ENTRY. Each JOURNAL ENTRY **may be** the payment of **one or more** INSTALLMENTs.
30. Each RECURRING RULE **may be** denominated in **one and only one** CURRENCY. Each CURRENCY **may be** the denomination of **one or more** RECURRING RULEs.
31. Each PAYEE RULE **may be** limited to **one and only one** ACCOUNT. Each ACCOUNT **may be** the scope of **one or more** PAYEE RULEs.
32. Each PAYEE RULE **may be** the setter of **one and only one** PAYEE. Each PAYEE **may be** set by **one or more** PAYEE RULEs.
33. Each PAYEE RULE **may be** the setter of **one and only one** CATEGORY. Each CATEGORY **may be** set by **one or more** PAYEE RULEs.
34. Each BUDGET LINE **must be** for **one and only one** CATEGORY. Each CATEGORY **may be** budgeted by **one or more** BUDGET LINEs.
35. Each BUDGET LINE **may be** for **one and only one** MEMBER. Each MEMBER **may be** budgeted by **one or more** BUDGET LINEs.
36. Each BUDGET LINE **must be** denominated in **one and only one** CURRENCY. Each CURRENCY **may be** the denomination of **one or more** BUDGET LINEs.
37. Each SAVINGS GOAL **may be** held in **one and only one** ACCOUNT. Each ACCOUNT **may be** the holder of **one or more** SAVINGS GOALs.
38. Each SAVINGS GOAL **must be** denominated in **one and only one** CURRENCY. Each CURRENCY **may be** the denomination of **one or more** SAVINGS GOALs.
39. Each RECONCILIATION **must be** of **one and only one** ACCOUNT. Each ACCOUNT **may be** reconciled by **one or more** RECONCILIATIONs.
40. Each IMPORT BATCH **may be** for **one and only one** ACCOUNT. Each ACCOUNT **may be** imported by **one or more** IMPORT BATCHes.
41. Each IMPORT BATCH **may be** read with **one and only one** IMPORT PROFILE. Each IMPORT PROFILE **may be** used by **one or more** IMPORT BATCHes.
42. Each IMPORT ROW **must be** part of **one and only one** IMPORT BATCH. Each IMPORT BATCH **may be** composed of **one or more** IMPORT ROWs.
43. Each IMPORT ROW **may be** denominated in **one and only one** CURRENCY. Each CURRENCY **may be** the denomination of **one or more** IMPORT ROWs.
44. Each IMPORT ROW **may be** suggested **one and only one** CATEGORY. Each CATEGORY **may be** suggested for **one or more** IMPORT ROWs.
45. Each IMPORT ROW **may be** suggested **one and only one** PAYEE. Each PAYEE **may be** suggested for **one or more** IMPORT ROWs.
46. Each IMPORT ROW **may be** a duplicate of **one and only one** JOURNAL ENTRY. Each JOURNAL ENTRY **may be** duplicated by **one or more** IMPORT ROWs.
47. Each ATTACHMENT **may be** attached to **one and only one** JOURNAL ENTRY. Each JOURNAL ENTRY **may be** documented by **one or more** ATTACHMENTs.
48. *(by convention, no FK)* Each replicated row **must be** last written by **one and only one** DEVICE (`hlc_device`). Each DEVICE **may be** the last writer of **one or more** rows.
49. *(by convention, no FK)* Each HOUSEHOLD SETTING 'base_currency' **must be** the code of **one and only one** CURRENCY. Each CURRENCY **may be** the household base currency.
50. *(by convention, inside JSON)* Each RECURRING RULE **must be** templated on one or more ACCOUNTs and CATEGORYs. Each ACCOUNT or CATEGORY **may be** referenced by **one or more** RECURRING RULEs.
51. *(not modelled)* Each RECONCILIATION **should be** the clearing of **one or more** POSTINGs. Each POSTING **may be** cleared by **one and only one** RECONCILIATION. (T-08)

### Never modelled

- The FX trading leg (T-01).
- The reconciliation ↔ posting link (T-08).
- Account balance snapshots for `shared_summary` accounts (T-13).
- The credit-card subtype of ACCOUNT (T-12).
- Where a merged category's references go (T-27).
- The tables required by PR 14 (T-29).
- The references inside `template_json` (T-10).
