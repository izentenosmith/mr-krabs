# 00 · Vision & Scope

## Problem

Household finances in a couple are split across personal bank accounts, credit cards with installment purchases (*compras en cuotas*), shared expenses and a spreadsheet that nobody keeps up to date. Existing apps either require a cloud account (your bank data on someone else's server), don't model "mine / yours / ours", or don't understand Chilean banking (CLP without decimals, UF, *cartolas* and *estados de cuenta*).

## Product statement

**Mr Krabs** is a desktop app that gives each partner a private, accurate view of their own money **and** a shared, reconciled view of the household's money — with all data living only on the partners' own Macs, kept in sync directly over the home network.

## Goals

| ID | Goal |
|---|---|
| G1 | Exact, auditable money math (double-entry ledger, integer minor units). |
| G2 | Three lenses on the same data: **Personal**, **Household**, **Combined**. |
| G3 | Fast monthly routine: import statements → review → categorize → see budget vs. actual in minutes. |
| G4 | Two devices, offline-first, converge deterministically when they meet on the LAN. |
| G5 | Zero infrastructure cost, zero cloud dependency, private by default. |
| G6 | Optional AI that *proposes* parsed transactions from bank statements; humans *approve*. |

## Users

- **Primary user** (owner of device A) — sets up the household, manages personal + shared accounts.
- **Partner** (owner of device B) — same capabilities; sees household data and their own personal data. Personal data of the other partner is **visible only if shared** (see privacy rules in [Requirements](01-requirements.md#privacy--visibility)).

## Scope

### In scope (v1)
- Accounts: checking (*cuenta corriente*), savings, *cuenta vista / RUT*, cash, credit cards, loans/mortgages, investments (balance tracking).
- Transactions: income, expense, transfers, split transactions, installment purchases, shared expenses with settle-up.
- Categories, budgets (monthly), recurring transactions, net worth, cash flow.
- Importers: CSV/XLSX/PDF statements, with per-bank profiles; optional AI-assisted extraction.
- Multi-currency with configurable base currency (default **CLP**), including **UF** and **USD**.
- Two-device LAN sync with pairing and encryption.
- Local backups (encrypted file export) and CSV export.

### Out of scope (v1)
- Direct bank connections / open-banking APIs / scraping bank websites.
- Cloud sync, web or mobile clients (architecture keeps the door open).
- More than two synced devices (protocol designed for N, tested for 2).
- Tax filing, investment performance analytics (beyond balances), invoicing.

## Success criteria
- A full month for a two-person household can be imported, reviewed and reconciled in **< 15 minutes**.
- Account balances match bank statements **to the peso** after reconciliation.
- Two devices edited offline for a week converge with **no lost edits** and no duplicate transactions after one sync.
- App works with the network cable unplugged.
