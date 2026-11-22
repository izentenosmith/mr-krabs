# 09 · Prior art

What we took from other open-source finance apps, and what we left out. Based on each project's README and public docs (reviewed November 2026), not their code.

| | [GnuCash](https://github.com/gnucash/gnucash) | [Sure](https://github.com/we-promise/sure) | [Actual](https://github.com/actualbudget/actual) |
|---|---|---|---|
| Model | Desktop, true double entry (GPL) | Rails + Postgres + Redis server you self-host (AGPL) | Local-first Node app; multi-device sync needs its sync server (MIT) |
| Offline | Yes (XML or SQLite file) | No, it is a server | Yes |
| Household | Not documented | Users grouped into a "family" | One shared budget file; no personal vs. shared split |
| Categories | Income/expense accounts in a tree | Not documented | User-defined groups → categories |
| Multi-currency | Yes | Exchange-rate jobs | No |
| Excel (XLSX) import | No | Not documented | No |

## Adopted

| Idea | From | Where it lives |
|---|---|---|
| Categories as nominal accounts in a tree of any depth | GnuCash | FR-BU-01, FR-BU-08, [03](03-data-model.md#categories-are-data) |
| Delete a category by merging it into another | Actual | FR-BU-07 |
| Local learned categorizer (description words → category), no AI | GnuCash | FR-TX-12 |
| Duplicate match scores with a proposed action per row | GnuCash | FR-TX-07 |
| Editable view of everything the app has learned | GnuCash | FR-IM-08 |
| Rules engine with stages, regex/one-of/between conditions, learned rules, bulk apply | Actual | FR-TX-10 |
| Schedules with approximate or ranged amounts, recurring-pattern detection | Actual | FR-BU-06, FR-TX-09 |
| "Since last run" list of recurring items that came due | GnuCash | FR-TX-09 |
| Lock reconciled transactions | Actual | FR-AC-06 |
| Split "distribute remainder" | Actual | FR-TX-03 |
| Manually valued assets in net worth | Sure | FR-AC-07 |
| Cash-flow Sankey | Sure, Actual | FR-RP-08 |

## Our own (none of the three has it)

- Import of the household's own Excel workbook and of any XLSX transaction list (FR-IM-01, FR-IM-06).
- Personal vs. household scopes, shared expenses and settle-up between two members (FR-HH-*).
- Device-to-device LAN sync with no server at all (FR-SY-*).

## Left out

- Bank-sync providers (Plaid, SimpleFIN, GoCardless, …): they need the internet and third-party accounts.
- Any server component, hosted API or MCP endpoint.
- GnuCash's small-business features (invoices, A/R, A/P, payroll).
- Envelope-budgeting mechanics ("to budget", "hold for next month"): our budget model is the spreadsheet's plan vs. real. Revisit if the household wants it.
