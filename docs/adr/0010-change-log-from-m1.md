# ADR-0010 · Change log written from M1

**Status:** Accepted (2026-11-21, from the M1 design review)

## Context
docs/05 needs every replicated change in `change_log` and `field_clock`, written in the same SQL transaction as the mutation. M1 is the first milestone that writes user data: households, accounts and journal entries. Sync itself only arrives in M7/M8. Today `category_delete` takes a caller-supplied `Stamp` and writes no change log. `device.public_key` is `NOT NULL`, but the Noise keypair is created at pairing (M8).

## Decision
- From M1, every mutation of a replicated table appends `change_log` rows and updates `field_clock`, in the same transaction. One store-level writer, `Store::record_change`, is the only code that writes them.
- The local device gets a UUIDv7 id at first launch, and an HLC (`mrk-sync`) issues every stamp. Callers no longer supply stamps.
- Migration `0003` makes `device.public_key` nullable until pairing sets it. No placeholder key is ever written.
- Seeded rows keep ADR-0008's exception: `hlc = 0`, no change-log entry.

## Alternatives
- **Stamps only, change log from M7 with a backfill.** Smaller M1, but no edit history before M7 (NFR-10), and the backfill can't recover before/after values. Rejected.
- **Placeholder public key.** Avoids a migration, but M8 pairing would have to recognise a fake key. Rejected.

## Consequences
- ✅ Audit trail (NFR-10) from the first entry; M7 starts from real history.
- ❌ The `fields_json` format is frozen into existing rows from M1. Changing it later means M7 reads two formats or migrates the log.
- ❌ Every repository write must call the writer. A test asserts that each mutating command leaves change-log rows.
- What would change our mind: the household choosing not to use M1 for real data before M7.
