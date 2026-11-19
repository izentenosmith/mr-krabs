# ADR-0003 · Double-entry ledger with integer minor units

**Status:** Accepted

## Context
Transfers must not double-count spending; credit-card payments, savings and settlements are movements, not expenses. Floating point is unacceptable for money.

## Decision
- Each transaction is a journal entry with postings that sum to zero per currency; categories are nominal accounts.
- Amounts are `i64` minor units with a currency code; currency metadata defines minor units (CLP 0, USD 2, CLF/UF 4). FX rates and interest use `rust_decimal` with explicit rounding.
- Splits and installment schedules use largest-remainder allocation so parts always sum exactly to the total.

## Consequences
- ✅ Transfers/settlements/savings are naturally excluded from expenses; balances are always explainable.
- ✅ Cross-currency movements record both sides explicitly.
- ❌ Slightly more complex writes; hidden behind `NewTxInput` variants (expense/income/transfer/split).
