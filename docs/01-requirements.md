# 01 · Requirements

Requirement IDs are stable and referenced from code, tests and the roadmap. Priority: **M** = must (v1), **S** = should, **C** = could.

## Feature catalogue (from the household finance spreadsheet)

<!-- FEATURE-CATALOGUE:START -->
Mr Krabs replaces the household's current spreadsheet template (*Plantilla contabilidad casa*). The template has **one annual summary sheet** (*Resumen*) and **twelve monthly sheets** (*Enero … Diciembre*), each a grid of **category rows × day-of-month columns (1–31) + monthly total**. The summary shows, per month, a **planned** and a **real** column per category, a **monthly cash flow** line and **total expenses**.

> No personal data from the spreadsheet is reproduced here — only its structure. Names are replaced by *Member A / Member B*; amounts are omitted.

### What the spreadsheet does today → how Mr Krabs covers it

| # | Spreadsheet feature | Mr Krabs feature | FR-IDs |
|---|---|---|---|
| F1 | **Two incomes, one per partner**, each labelled with its expected pay day (e.g. "on the 28th", "between the 20th and 25th"). | Income per member into their own account; **expected income schedule** with a pay day or pay-day window; "income not yet received" indicator. | FR-HH-01, FR-TX-09, FR-RP-04, FR-IN-01 |
| F2 | **Savings & investments** block (e.g. emergency fund) counted as an outflow of monthly cash flow but not as an expense. | Savings = **transfer** to a savings/investment account (never an expense); cash-flow report shows a separate **"Saved/invested"** line; savings goals (emergency fund). | FR-TX-02, FR-BU-05, FR-RP-02 |
| F3 | **Fixed expenses** (rent, *gastos comunes*, gas, electricity, water, internet, household help) annotated with an **expected amount or range**. | Recurring bills with **expected amount or min–max range**; flag when actual falls outside range; reminder of unpaid bills this month. | FR-TX-09, FR-BU-06, FR-RP-04 |
| F4 | **Variable expenses** (household purchases, home repairs). | Expense categories with monthly budget. | FR-BU-01, FR-BU-02 |
| F5 | **Food** (supermarket, delivery, restaurants). | Expense categories; payee rules auto-categorize delivery apps/supermarkets from statements. | FR-BU-01, FR-TX-10 |
| F6 | **Transport** (fuel, **car maintenance per partner**). | Per-member attribution of expenses (category + member), e.g. "Car maintenance — Member A". | FR-HH-07 |
| F7 | **Subscriptions**, including one **split among three people** (cost ÷ 3 per person). | Subscriptions as recurring rules; **split with participants outside the household** — only the household's share counts as expense, the rest is a receivable until reimbursed. | FR-TX-09, FR-HH-08 |
| F8 | **Leisure** (travel, cinema). | Expense categories; trips can be tagged to group spending. | FR-BU-01, FR-TX-01 |
| F9 | **Debts** block (free rows for loans/cards). | Loan, mortgage and credit-card accounts; payments are transfers (principal) + expense (interest/fees); installment plans (*cuotas*). | FR-AC-01, FR-TX-04, FR-TX-05 |
| F10 | **Daily grid**: amounts typed per day (columns 1–31) per category. | Transactions carry a date; **month calendar view** showing daily spend per category (read-only projection of the ledger), with quick-add from a day cell. | FR-TX-01, FR-RP-06 |
| F11 | **Monthly total per category** (`SUM` of the day columns). | Computed in Rust from the ledger. | FR-BU-03 |
| F12 | **Annual summary: Plan vs Real per month per category**, with group subtotals. | **Budget vs. actual matrix**: rows = category groups/categories, columns = 12 months × (planned, actual, Δ); subtotals per group. | FR-BU-02, FR-BU-03, FR-RP-07 |
| F13 | **Monthly cash flow** = income − savings − all expense groups. | Cash-flow report: income, saved/invested, expenses, **free cash flow**. | FR-RP-02 |
| F14 | **Total expenses** per month. | Same report, plus trend chart. | FR-RP-02 |
| F15 | Free (unnamed) rows in each group to add new items. | User-defined categories within groups; create category inline. | FR-BU-01 |
| F16 | One workbook per year, copied manually. | Continuous ledger; any period selectable; year-over-year comparisons. | FR-RP-05 |

### Problems in the spreadsheet that Mr Krabs must prevent
- **Broken cross-sheet references**: some summary rows point to the wrong row of the monthly sheets (row offsets drifted when rows were inserted), so a category's "real" value can silently show another category's total. → Mr Krabs never references by position; categories are entities with IDs.
- **No link to actual bank movements**: everything is typed by hand. → Statement import (FR-IM-*).
- **No notion of who paid / whose account**. → Members, scopes, settle-up (FR-HH-*).
- **Savings vs. expense ambiguity**. → Transfers are never expenses (FR-TX-02).
- **No audit trail**. → Change log (NFR-10).

### Default category tree (seed, es-CL)
Groups mirror the spreadsheet so the household recognizes it on day one:

| Group (kind) | Seed categories |
|---|---|
| Ingresos (income) | Sueldo, Otros ingresos |
| Inversiones y ahorros (*transfer targets, not expenses*) | Fondo de emergencia, Ahorro, Inversiones |
| Gastos fijos | Arriendo / Dividendo, Gastos comunes, Gas, Luz, Agua, Internet, Servicio doméstico |
| Gastos variables | Compras casa, Arreglos casa |
| Comida | Supermercado, Delivery, Restaurantes |
| Transporte | Bencina, Mantención auto, Transporte público, Estacionamiento/TAG |
| Suscripciones | Streaming, Software, Otras |
| Ocio | Viajes, Cine, Salidas |
| Deudas | Intereses y comisiones (principal payments are transfers) |
<!-- FEATURE-CATALOGUE:END -->

## Functional requirements

### Household, members & scopes
| ID | P | Requirement |
|---|---|---|
| FR-HH-01 | M | A **household** has exactly two **members** in v1 (the protocol allows N). |
| FR-HH-02 | M | Every account has a **scope**: `personal` (owned by one member) or `household` (owned jointly). |
| FR-HH-03 | M | Three views: **Personal** (my personal accounts + my share of household), **Household** (household accounts + shared expenses), **Combined** (everything visible to me). |
| FR-HH-04 | M | A transaction paid from a personal account can be marked **shared** with a split (default 50/50, custom % or fixed amounts). |
| FR-HH-05 | M | **Settle-up**: Rust computes who owes whom from shared expenses; recording a settlement is a transfer between members' accounts. |
| FR-HH-06 | S | Per-member contribution targets for household (e.g. proportional to income). |
| FR-HH-07 | M | An expense can be **attributed to a member** (e.g. "car maintenance — Member A") independently of which account paid it. |
| FR-HH-08 | S | Split an expense with **external participants** (friends/family outside the household): only the household share is an expense; the remainder is tracked as a receivable until reimbursed. |

### Income
| ID | P | Requirement |
|---|---|---|
| FR-IN-01 | M | Expected income per member with a pay day or pay-day window (e.g. day 20–25); shows expected vs. received for the month. |

### Privacy & visibility
| ID | P | Requirement |
|---|---|---|
| FR-PV-01 | M | A personal account is `private` (only owner sees transactions; partner sees nothing) or `shared-summary` (partner sees balance only) or `shared-full`. Default: `shared-summary`. |
| FR-PV-02 | M | Private data is **not synced** to the partner's device at all (filtered at the sync layer, not hidden in the UI). |
| FR-PV-03 | S | App lock with Touch ID / macOS login via Keychain. |

### Accounts
| ID | P | Requirement |
|---|---|---|
| FR-AC-01 | M | Account types: checking, savings, *cuenta vista*, cash, credit card, loan/mortgage, investment, other asset/liability. |
| FR-AC-02 | M | Each account has one currency; balances are derived from the ledger, never stored as truth. |
| FR-AC-03 | M | Opening balance entry with date. |
| FR-AC-04 | M | Archive (hide) an account without deleting history. |
| FR-AC-05 | M | Credit cards: credit limit, billing cycle (*fecha de facturación*), due date (*fecha de pago*), national (CLP) and international (USD) sections. |
| FR-AC-06 | S | Reconciliation: mark a statement balance at a date as reconciled; flag differences. |

### Transactions
| ID | P | Requirement |
|---|---|---|
| FR-TX-01 | M | Record income, expense and transfer with date, amount, account, category, payee, memo, tags. |
| FR-TX-02 | M | **Transfers between own accounts never count as income or expense** (no double-counting). |
| FR-TX-03 | M | Split a transaction across multiple categories. |
| FR-TX-04 | M | **Installment purchases (*compras en cuotas*)**: record total, number of installments, installment amount, interest; generate the schedule; budget impact by month of installment (configurable: at purchase vs. per installment). |
| FR-TX-05 | M | Credit card payment = transfer from checking to card (not an expense). |
| FR-TX-06 | M | Status: `pending`, `cleared`, `reconciled`. |
| FR-TX-07 | M | Duplicate detection on import (fingerprint: account, date ±2d, amount, normalized description). |
| FR-TX-08 | M | Soft delete with undo; full audit history of edits. |
| FR-TX-09 | S | Recurring transactions (rent, *gastos comunes*, subscriptions, salary) with auto-suggest. |
| FR-TX-10 | S | Payee rules: auto-categorize by description pattern. |
| FR-TX-11 | C | Attachments (receipt images) stored locally. |

### Categories & budgets
| ID | P | Requirement |
|---|---|---|
| FR-BU-01 | M | Two-level categories (group → category), separate income and expense trees; seed with a Chilean-household default set. |
| FR-BU-02 | M | Monthly budget per category per scope (personal / household). |
| FR-BU-03 | M | Budget vs. actual, remaining, and % used — computed in Rust. |
| FR-BU-04 | S | Rollover of unused budget (per category toggle). |
| FR-BU-05 | S | Savings goals (target amount + date) linked to an account. |
| FR-BU-06 | M | Recurring/fixed bills can carry an **expected amount or min–max range**; actuals outside the range are flagged. |

### Reports
| ID | P | Requirement |
|---|---|---|
| FR-RP-01 | M | Net worth over time (assets − liabilities), per scope, in base currency. |
| FR-RP-02 | M | Cash flow per month (income, expense, net, savings rate). |
| FR-RP-03 | M | Spending by category / payee for a period. |
| FR-RP-04 | S | Upcoming obligations: card due dates, future installments, recurring bills. |
| FR-RP-05 | C | Year-over-year comparison. |
| FR-RP-06 | M | **Month calendar view**: daily spend per category for a month (the spreadsheet's day grid), with quick-add on a day. |
| FR-RP-07 | M | **Annual plan-vs-real matrix**: categories × 12 months × (planned, actual, difference), with group subtotals, monthly cash flow and total expenses rows. |

### Currency
| ID | P | Requirement |
|---|---|---|
| FR-CU-01 | M | Base (reporting) currency configurable; **default CLP**. |
| FR-CU-02 | M | Supported out of the box: **CLP** (0 decimals), **USD** (2), **EUR** (2), **UF** (*Unidad de Fomento*, 4 decimals, as a unit of account). Users can add ISO-4217 currencies. |
| FR-CU-03 | M | Exchange rates entered manually or imported from a CSV; stored per date; conversions done in Rust with `rust_decimal` and explicit rounding (banker's rounding, configurable). |
| FR-CU-04 | M | Cross-currency transfers record both amounts (no implied rate). |
| FR-CU-05 | C | Optional rate fetch from a free public source (e.g. mindicador.cl) — opt-in, the only outbound call besides AI. |

### Import
| ID | P | Requirement |
|---|---|---|
| FR-IM-01 | M | Import CSV / XLSX statements with saved column-mapping **profiles** per bank/account. |
| FR-IM-02 | M | Import PDF *estados de cuenta* for supported banks via deterministic parsers. |
| FR-IM-03 | M | All imports go to a **staging area**; user reviews, edits, dedups, then commits. |
| FR-IM-04 | S | **Optional AI extraction** for unsupported formats — see [06](06-statement-ingestion.md). Off by default. |
| FR-IM-05 | M | Import batches are traceable and reversible (undo a whole import). |

### Sync
| ID | P | Requirement |
|---|---|---|
| FR-SY-01 | M | Discover partner device on LAN via mDNS/Bonjour. |
| FR-SY-02 | M | Explicit pairing with a short code / QR; mutual authentication. |
| FR-SY-03 | M | End-to-end encrypted transport. |
| FR-SY-04 | M | Automatic sync when both are reachable; manual "Sync now". |
| FR-SY-05 | M | Deterministic convergence; no lost edits; conflicts surfaced when semantically meaningful. |
| FR-SY-06 | M | Unpair / revoke a device. |

### Data management
| ID | P | Requirement |
|---|---|---|
| FR-DM-01 | M | Encrypted backup to a user-chosen file/folder (e.g. iCloud Drive folder — user's choice, no app server). |
| FR-DM-02 | M | Restore from backup. |
| FR-DM-03 | M | Export transactions to CSV. |
| FR-DM-04 | S | Language: Spanish (es-CL) and English UI; es-CL number/date formats by default. |

## Non-functional requirements

| ID | Category | Requirement |
|---|---|---|
| NFR-01 | Cost | $0 infrastructure. No servers, no paid services required to run. |
| NFR-02 | Privacy | No telemetry. No network calls except LAN sync and explicitly opted-in features (AI provider, FX rates). |
| NFR-03 | Correctness | No `f32`/`f64` in money paths (enforced by `clippy::disallowed_types`). Property tests for ledger invariants. |
| NFR-04 | Architecture | No SQL triggers / procedures / views with logic (CI check). No `@tauri-apps/api` imports outside the API client package (ESLint rule). |
| NFR-05 | Performance | Cold start < 2s; list of 10k transactions scrolls at 60fps (virtualized); monthly report < 200ms on 100k transactions. |
| NFR-06 | Reliability | SQLite in WAL mode; every mutation in a single transaction; crash-safe. |
| NFR-07 | Security | DB encryption at rest via SQLCipher (key in macOS Keychain) — *S priority*; device keys in Keychain; AI API keys in Keychain. |
| NFR-08 | Portability | Core crate has no Tauri/macOS dependency; builds and tests on Linux CI. |
| NFR-09 | Testability | Core ≥ 85% line coverage; golden-file tests for each importer. |
| NFR-10 | Auditability | Every entity change recorded in the change log with device, HLC timestamp and before/after. |
| NFR-11 | Accessibility | Keyboard-first navigation, VoiceOver labels (shadcn/Radix primitives). |
