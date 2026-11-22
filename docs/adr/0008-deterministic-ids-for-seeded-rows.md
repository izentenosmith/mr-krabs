# ADR-0008 · Deterministic ids for seeded rows

**Status:** Accepted (2026-11-22, with the schema review proposal rev. 3)

## Context
The household starts with a default category tree and default savings/investment accounts (docs/01). Each paired device creates them on its own: on household creation, and on adoption when a second device joins. With random UUIDv7 ids, two devices would produce two copies of every default row, and pairing would duplicate the whole tree.

## Decision
- Seeded rows use `id = UUIDv5(MRK_SEED_NAMESPACE, seed_key)`.
  - The namespace is a fixed constant in `mr-krabs-core::seed` and never changes.
  - The `seed_key` (e.g. `expense.food.supermarket`) is never reused for different content.
- The catalogue in `crates/mr-krabs-core/src/seed.rs` is the single source of the default tree. docs/01's table describes it.
- Seed writes:
  - carry `hlc = 0` and `hlc_device = 'seed'`, and are not appended to `change_log`;
  - use `ON CONFLICT (id) DO NOTHING`, so seeding is idempotent and a deleted seed row is never resurrected;
  - skip a seed category whose normalized name a live user category already has in the same place, and use that row instead.
- Names are written in the household's locale at seeding time, and are user data from then on.
- User-created rows keep UUIDv7. Currencies are seeded by migration 0002, where the natural primary key (`code`) already converges.

## Alternatives
- **Seed only on the first device and sync the rows.** This fails when both devices were used before pairing, and the rows would need change-log entries.
- **Seed in a SQL migration.** A migration can't know the household's locale, and its content can never be corrected (forward-only migrations).

## Consequences
- ✅ Two devices seed byte-identical rows; pairing never duplicates them.
- ✅ Any user edit (hlc > 0) beats the seed under per-field LWW.
- ❌ An exception to "IDs are UUIDv7" that every reviewer must know about. A test pins the namespace.
- ❌ A new default in a later release needs a new `seed_key`; changing an old seed's content does not reach existing households.
