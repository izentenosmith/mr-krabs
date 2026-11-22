# ADR-0011 · Opening balances balance against an FX-trading leg

**Status:** Accepted (2026-11-22, owner decision DEC-4 during M1)

## Context
FR-AC-03 needs an opening balance with a date for every new account. In double entry, that amount must balance against something. Schema v2 has three legs (`account`, `category`, `fx`, ADR-0009) and no equity leg. A category counter would report the opening balance as income or expense. A system "Opening balances" account would appear in account lists and net worth, which is why ADR-0009 rejected system accounts for FX.

## Decision
- An opening-balance entry has `kind = opening_balance` and two legs:
  - the account leg, carrying the amount;
  - an FX-trading leg (`Leg::FxTrading`) of the opposite amount, in the account's currency.
- `classify()` ignores FX legs, so the entry is a transfer: never income or expense.
- No migration is needed.

## Alternatives
- **New `equity` leg** (GnuCash "Opening Balances"). The honest double entry, but it needs a posting-table rebuild during M1. Rejected for now.
- **Seeded "Saldo inicial" category.** Opening balances would count as income or expense (FR-TX-02). Rejected.

## Consequences
- ✅ Opening balances balance per currency with no schema change, and never pollute income, expense or the account list.
- ❌ "FX leg" means two things: a currency-exchange position, and the counter of an opening balance. Any FX-position or FX-gain computation (M4) must exclude `kind = opening_balance` entries.
- Reversible: these legs are identified by their entry kind, so a later migration can turn them into an `equity` leg mechanically.
- What would change our mind: M4's FX reporting needs a clean trading position, or a second use of a balancing leg appears (adjustments, valuations). Then add the `equity` leg.
