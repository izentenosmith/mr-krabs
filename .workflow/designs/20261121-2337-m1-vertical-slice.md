# Design: M1 vertical slice — record an expense and see the balance

**Status:** resolved
**Solving:** The core can parse money, seed defaults and merge categories, but nothing can create a household, an account or a transaction, and the UI has no screen beyond health. Without these the app cannot hold the household's data.
**Why now:** M1 is next on the roadmap, and M2–M9 all build on it: categories, budgets, the household, cards, import and sync all need entries written through one validated path. Schema v2 (#16) is merged, so the storage the slice writes to is fixed.
**Done looks like:** On a fresh install, onboarding creates the household in CLP. You create a checking account, record expenses and a transfer to savings, and the sidebar balances are correct. The transfer is not counted as an expense. All money logic is covered by Rust tests, and the slice ships as several PRs into `dev` that can be built in parallel.
**Calibration:** large — the first slice that crosses every layer (domain, store, core, both shells, api-client, UI), split into parallel PRs
**Questions:** ~13 to resolve (~12 → ~13: decomposition counted separately) — 13 resolved, 0 deferred (all recommendations accepted by the owner, 2026-11-22: "follow architect's recommendation for all questions")

## Decisions
- **Policy vs details — what already exists:** schema v2 stays as is; `household_create` calls `seed_household_defaults`; `validate_balanced`/`classify` exist and M1 adds `JournalEntry::validate` + `account_balances`. [inferred] from #16 on dev (ADR-0002/0003/0008/0009 binding).
- **Scope — household & transaction shape (Q1):** one member (me); accounts default `household`/`shared_full`, `personal` allowed with me as owner; transactions are 2-leg expense/income/transfer + opening balance; `NewTxInput` is a tagged union `{type: expense|income|transfer}` expanded to postings in core. No splits, sharing, tags or payees. — thinnest slice that meets "done when"; fields are additive later. Rejected: N-leg splits + payee now (largest UI surface, cheap to add later).
- **Data model — sync-readiness (Q2):** full change log from M1: local device UUIDv7 + `mrk-sync` HLC issue every stamp; every replicated mutation writes `change_log` + `field_clock` in the same tx through one `Store::record_change`; a new migration (number assigned at merge) makes `device.public_key` and `device.member_id` nullable (the device row exists from first open; `household_create` links it, Q-sync-stamps-1). — NFR-10 from the first entry, M7 starts from real history. Rejected: stamps-only + M7 backfill (loses pre-M7 history). ADR-0010.
- **Dependency direction — runtime (Q3):** keep the synchronous `App` behind the shells' `Mutex`; each service method = one call, one SQL transaction; writer actor + read pool arrive with M7 behind the same `App` API; docs/02 amended. — no M1 feature needs concurrency; contained refactor later. Rejected: async actor now (cost in every M1 PR, speculative until sync).
- **API contracts — DTOs to TypeScript (Q4):** adopt `specta` in a contract PR: DTOs derive `specta::Type`, a core test exports `packages/api-client/src/bindings.ts` via `specta-typescript`, CI fails on a stale file; commands stay hand-registered in both shells (no tauri-specta macros in core). Every M1 command lands in `COMMANDS` first as a stub returning `NOT_IMPLEMENTED`, so UI and core build against one contract. — one source of types while PRs run in parallel. Rejected: hand-written `types.ts` (drift across parallel PRs). Fallback: a type specta can't express stays hand-written, named in the PR.
- **Data model — `fields_json` (Q5):** changed fields only, `{"<field>": {"old": v|null, "new": v}}`, values as the column's SQLite value (money as integer `amount_minor`); create has `old: null`, delete is `op = delete` with the deleted flag; the diff lives in `Store::record_change`, property-tested for create → update → delete. — serves per-field LWW (`new`) and NFR-10 (`old`). Rejected: full after-row snapshot (no before, no changed-field set).
- **Failure modes — edits and errors (Q6):** `tx_update` edits legs in place by role (account leg, counter leg), keeping posting ids stable; a type change converts the counter leg in place. `tx_delete` soft-deletes entry + postings in one tx, `tx_restore` reverses it. Errors: archived account → `VALIDATION`; currency ≠ account's → `CURRENCY_MISMATCH`; unknown id → `NOT_FOUND`; unbalanced → `UNBALANCED_ENTRY`; editing a reconciled leg → `CONFLICT`; not-yet-built command → `NOT_IMPLEMENTED`. Field errors carry `ApiError.fields` (field → i18n key). — concurrent edits merge into one 2-leg entry under docs/05 rule 3. Rejected: replace legs with new ids (4 live legs after an offline merge).
- **Security (Q7):** trigger fires (money values, two entry points). Resolved below in **Security**. — [inferred] devserver already binds `127.0.0.1:7878` unless `MRK_BIND` is set.
- **Scale & performance (Q8):** `tx_list` is keyset-paginated on `(entry_date DESC, id DESC)` with an opaque cursor, `limit` ≤ 500 (default 100); balances are computed in Rust from live account-leg rows per account (`SELECT` by indexed `account_id`), no SQL `SUM`; the transactions table virtualizes (TanStack Virtual). Target from NFR-05: 10k rows scroll at 60fps; balances for 100k postings < 100 ms. — matches docs/03's scale; no cache needed. Rejected: offset pagination (unstable under inserts), stored balances (ADR-0003: derived, never truth).
- **Backwards-compatibility (Q9):** existing dev databases at v2 upgrade through the new device migration (table rebuild under `Store::migrate`'s FK-off + `foreign_key_check`); `app_health`, `money_parse` and their wire snapshots are unchanged; `category_delete`'s caller-supplied `Stamp` parameter is removed (it had no external caller) and the clock issues stamps. — nothing shipped; only dev DBs exist. Rejected: none live.
- **Observability (Q10):** one `tracing` span per command (`command`, duration, result code) at `info`; entity ids at `debug`; amounts, descriptions and names never at `info` (CLAUDE.md); migration start/end at `info`. No metrics (no backend, NFR-02). — enough to debug from the local log file. Rejected: none live.
- **Testability (Q11):** domain rules are pure functions with unit + proptest tests (balance, classify, expansion, leg matching, `account_balances`); store on in-memory SQLite; acceptance tests go through `commands::dispatch` (the boundary both shells share) on `App::open_in_memory`; one devserver HTTP smoke for the journey; UI with Vitest + Testing Library against a mocked `api`. No test needs Tauri. — every rule is checkable without a shell. [inferred] from existing tests.
- **Migration & rollout (Q12):** no feature flag; PRs merge into `dev` in dependency order (below), each green on its own; stub commands return `NOT_IMPLEMENTED` until their slice lands, and the UI shows a not-yet-available state for them; `dev → main` only after the journey passes, with the owner's approval. — `dev` is an integration branch, nothing ships from it. Rejected: one big M1 PR (no parallelism, unreviewable).
- **Decomposition (Q13):** eight slices in three waves, below. — splits by reason to change (clock/log, ledger rules, wire contract, core services, screens) with acyclic dependencies toward the stable side (domain, contract). Rejected: layer-by-layer PRs (each needs all others to show anything) and per-command PRs (too many conflicts in `commands.rs`).

## Security
- **Trust boundary:** the OS user. The app is single-user and local; there is no authentication in M1. The two entry points are Tauri IPC (in-process) and the devserver (dev only, never shipped, `127.0.0.1` by default; `MRK_BIND` is the developer's choice).
- **Abuse/misuse cases:** malformed or oversized input → `VALIDATION`, never a panic (`serde` + explicit checks; descriptions ≤ 500 chars, memo ≤ 2 000); ids not UUIDs → `VALIDATION`; amounts parsed only by `mrk_money::parse_es_cl`, overflow → `VALIDATION`; SQL only through bound parameters (no string-built SQL with input).
- **Authorisation rule:** none beyond the OS user in M1; visibility (`private`, `shared_*`) is stored and defaulted (household → `shared_full`, personal → `shared_summary`) but only enforced at sync (FR-PV-02, M7).
- **Never logged at `info`:** amounts, descriptions, memos, names (household, member, account).
- **Fail closed:** any validation failure writes nothing (one SQL transaction per command, rolled back on error).

## Operability
- **Must not break:** `app_health`/`money_parse` wire snapshots; v2 dev databases open and migrate; CI guardrails (no SQL logic, migrations in order, FK check).
- **Logged:** per-command span with result code; migration start/end.
- **Rollout:** merge order below; no flag; `NOT_IMPLEMENTED` stubs until each slice lands. Undo = revert the slice's squash commit; the device migration is forward-only, so a revert after it merged keeps the migration and reverts only code.

## Decomposition
Wave 1 — start together, no dependencies:
- **S0 `fix/compose-schema-step`** — `compose.yaml` `test-back` applies migrations in order to one DB, as CI does (red on dev today); after nothing.
- **S1 `feat/sync-stamps`** — `mrk-sync` persisted HLC clock; local device identity (UUIDv7) created at first open; device-key migration (placeholder name, number at merge); `Store::record_change` (`change_log` + `field_clock`, Q5 format); `category_delete` retrofitted to use them; after nothing.
- **S2 `feat/ledger-domain`** — `mrk-domain`: `JournalEntry` + `validate(&LedgerContext)` (archived account, currency per account, balanced, transfer has no category legs), `NewTx` expansion (expense/income/transfer/opening balance → postings), in-place leg matching for update (Q6), `account_balances`; pure, property-tested; after nothing.
- **S3 `feat/api-contract`** — `specta` + generated `bindings.ts` + staleness check; all M1 DTOs and error codes (`NOT_IMPLEMENTED`, `CURRENCY_MISMATCH`, `UNBALANCED_ENTRY`, `CONFLICT`, `fields`); every M1 command in `COMMANDS` as a stub; Tauri + devserver wiring; `createApi` methods; after nothing.

Wave 2 — after S1, S2, S3 (S6/S7 need only S3):
- **S4 `feat/household-accounts`** — core+store: `app_bootstrap`, `household_create` (household, me, local device link, seed), `settings_get`, `account_list` (with balances) / `account_get` / `account_create` (with opening balance entry) / `account_update` / `account_archive`, and the store's entry insert path; after S1, S2, S3.
- **S6 `feat/ui-onboarding-accounts`** — app shell layout, onboarding (household, base currency CLP), accounts sidebar with balances, account create/edit; against the S3 contract; after S3.
- **S7 `feat/ui-transactions`** — transactions table (TanStack Table + Virtual, keyset paging), add/edit expense/income/transfer dialog, delete + undo; adds its route to S6's shell (rebases on S6 if S6 merges first); after S3.

Wave 3:
- **S5 `feat/transactions`** — core+store: `tx_create` / `tx_get` / `tx_list` / `tx_update` / `tx_delete` / `tx_restore`; after S4.
- **S8 `feat/m1-journey`** — the end-to-end journey through the devserver HTTP shell, docs status (roadmap M1 ✅, docs/04 implemented list), last; after S4, S5, S6, S7.

Later milestones — parallel tracks once M1 is done (owner decides when; CLAUDE.md: no new milestone unless asked):
- **Ledger track:** M2 → { M3 ‖ M4 } → M5. M3's members/scopes/settle-up don't need budgets; M4 needs M2's categories and budget mode; M5 needs M2 (category resolution, workbook budgets) and M4 (card statements).
- **Sync track:** M8's transport (mDNS, SPAKE2 pairing, Noise XX in `mrk-sync`) needs only M1's device identity and can start right after M1; M7's merge engine can start after M1 on the change log and gains per-entity rules as M2–M4 add tables. This changes the roadmap's "sync only after the single-device ledger is trustworthy" ordering, so it is the owner's decision.
- **Edges track:** M6's PDF parsers (pure, `mrk-import`, golden tests on synthetic PDFs) and the `mrk-ai` provider can start after M1; their staging commit waits for M5. M9 packaging (signing/notarization CI, `.dmg`) is independent; backups wait for the schema to settle (after M5).

## Deferred
- none — every dimension resolved.

## N/A
- none.

## Records
- docs/adr/0010-change-log-from-m1.md — change log + field clock written from M1; device key nullable until pairing (Q2)
- docs/adr/0011-opening-balance-fx-leg.md — opening balance counter is an FX-trading leg (DEC-4, from Q-ledger-domain-1)
- docs/02-architecture.md — amended: synchronous core until M7 (Q3)
- docs/07-roadmap.md — amended: M1 slices and later parallel tracks (Q13)
