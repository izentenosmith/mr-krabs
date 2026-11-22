//! Category and seed writes. Storage only: what to write is decided in `mrk-domain`
//! (tree rules, merge plan) and `mr-krabs-core` (seed catalogue); this module reads rows
//! and applies those decisions in one transaction each.
//!
//! User edits are logged through [`Store::track`] (docs/05, ADR-0010); seed rows never are
//! (ADR-0008).

use rusqlite::{OptionalExtension, ToSql, Transaction, params};

use crate::{Result, Store, Visibility};

/// HLC timestamp and device a change is stamped with (issued by the local clock).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stamp {
    pub hlc: i64,
    pub device: String,
}

impl Stamp {
    /// The stamp of seeded rows: older than any user edit, so every edit wins (ADR-0008).
    pub fn seed() -> Self {
        Self {
            hlc: 0,
            device: "seed".into(),
        }
    }
}

/// A seeded category, parents listed before their children.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SeedCategory {
    pub id: String,
    pub parent_id: Option<String>,
    pub kind: &'static str,
    pub name: String,
    pub name_key: String,
    pub seed_key: String,
    pub sort_order: i64,
}

/// A seeded household account (savings and investments are accounts, docs/01 seed note).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SeedAccount {
    pub id: String,
    pub name: String,
    pub kind: &'static str,
    pub currency_code: String,
    pub sort_order: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SeedReport {
    pub categories_inserted: usize,
    /// Seed categories whose name a live user category already has in the same place;
    /// that category is used instead (its seeded children attach under it).
    pub categories_kept_existing: usize,
    pub accounts_inserted: usize,
}

/// The household's typed settings (schema v2 `household`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HouseholdSettings {
    pub id: String,
    pub base_currency_code: String,
    pub locale: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CategoryRow {
    pub id: String,
    pub parent_id: Option<String>,
    pub kind: String,
    pub deleted: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BudgetLineAmount {
    pub id: String,
    pub category_id: String,
    pub period: String,
    pub scope: String,
    pub member_id: Option<String>,
    pub amount_minor: i64,
    pub currency_code: String,
}

/// The writes of a category merge, as planned by `mrk_domain::plan_merge`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CategoryMergeWrite {
    pub from: String,
    pub into: String,
    pub reparent: Vec<String>,
    pub move_budget_lines: Vec<String>,
    /// `(target line id, new amount in minor units, absorbed line id)`.
    pub sum_budget_lines: Vec<(String, i64, String)>,
}

impl Store {
    /// The single household of this database, if one was created.
    pub fn household_settings(&self) -> Result<Option<HouseholdSettings>> {
        Ok(self
            .conn
            .query_row(
                "SELECT id, base_currency_code, locale FROM household WHERE deleted = 0 LIMIT 1",
                [],
                |r| {
                    Ok(HouseholdSettings {
                        id: r.get(0)?,
                        base_currency_code: r.get(1)?,
                        locale: r.get(2)?,
                    })
                },
            )
            .optional()?)
    }

    /// Inserts the default categories and accounts. Idempotent: a row whose id exists, live
    /// or deleted, is left alone, so deleted seed rows are never resurrected (ADR-0008).
    pub fn seed_defaults(
        &mut self,
        categories: &[SeedCategory],
        accounts: &[SeedAccount],
    ) -> Result<SeedReport> {
        let seed = Stamp::seed();
        let tx = self.conn.transaction()?;
        let mut report = SeedReport::default();
        // Seed id → id actually used (differs when a user category already holds the name).
        let mut resolved: std::collections::HashMap<String, String> = Default::default();
        for c in categories {
            let parent = c
                .parent_id
                .as_ref()
                .map(|p| resolved.get(p).cloned().unwrap_or_else(|| p.clone()));
            let existing: Option<String> = tx
                .query_row(
                    "SELECT id FROM category
                     WHERE deleted = 0 AND kind = ?1 AND ifnull(parent_id, '') = ifnull(?2, '')
                       AND name_key = ?3 AND id <> ?4",
                    params![c.kind, parent, c.name_key, c.id],
                    |r| r.get(0),
                )
                .optional()?;
            if let Some(id) = existing {
                resolved.insert(c.id.clone(), id);
                report.categories_kept_existing += 1;
                continue;
            }
            report.categories_inserted += tx.execute(
                "INSERT INTO category (id, parent_id, kind, name, name_key, seed_key, sort_order, hlc, hlc_device)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                 ON CONFLICT (id) DO NOTHING",
                params![
                    c.id, parent, c.kind, c.name, c.name_key, c.seed_key, c.sort_order, seed.hlc,
                    seed.device
                ],
            )?;
        }
        for a in accounts {
            report.accounts_inserted += tx.execute(
                "INSERT INTO account (id, name, kind, currency_code, scope, visibility, sort_order, hlc, hlc_device)
                 VALUES (?1, ?2, ?3, ?4, 'household', 'shared_full', ?5, ?6, ?7)
                 ON CONFLICT (id) DO NOTHING",
                params![a.id, a.name, a.kind, a.currency_code, a.sort_order, seed.hlc, seed.device],
            )?;
        }
        tx.commit()?;
        Ok(report)
    }

    /// Every category row, live and deleted, for the tree rules.
    pub fn category_rows(&self) -> Result<Vec<CategoryRow>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, parent_id, kind, deleted FROM category ORDER BY id")?;
        let rows = stmt
            .query_map([], |r| {
                Ok(CategoryRow {
                    id: r.get(0)?,
                    parent_id: r.get(1)?,
                    kind: r.get(2)?,
                    deleted: r.get::<_, i64>(3)? == 1,
                })
            })?
            .collect::<std::result::Result<_, _>>()?;
        Ok(rows)
    }

    /// Live budget lines of the given categories.
    pub fn live_budget_lines(&self, category_ids: &[&str]) -> Result<Vec<BudgetLineAmount>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, category_id, period, scope, member_id, amount_minor, currency_code
             FROM budget_line WHERE deleted = 0 AND category_id = ?1 ORDER BY id",
        )?;
        let mut out = Vec::new();
        for id in category_ids {
            let rows = stmt.query_map([id], |r| {
                Ok(BudgetLineAmount {
                    id: r.get(0)?,
                    category_id: r.get(1)?,
                    period: r.get(2)?,
                    scope: r.get(3)?,
                    member_id: r.get(4)?,
                    amount_minor: r.get(5)?,
                    currency_code: r.get(6)?,
                })
            })?;
            for row in rows {
                out.push(row?);
            }
        }
        Ok(out)
    }

    /// How many live postings, budget lines and child categories reference a category.
    pub fn category_reference_count(&self, id: &str) -> Result<i64> {
        Ok(self.conn.query_row(
            "SELECT (SELECT count(*) FROM posting     WHERE deleted = 0 AND category_id = ?1)
                  + (SELECT count(*) FROM budget_line WHERE deleted = 0 AND category_id = ?1)
                  + (SELECT count(*) FROM category    WHERE deleted = 0 AND parent_id   = ?1)",
            [id],
            |r| r.get(0),
        )?)
    }

    /// Soft-deletes a category nothing live references, and logs it.
    pub fn delete_category(&mut self, id: &str) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        self.track(&tx, "category", id, Visibility::Household, |tx| {
            tx.execute("UPDATE category SET deleted = 1 WHERE id = ?1", [id])?;
            Ok(())
        })?;
        tx.commit()?;
        Ok(())
    }

    /// Applies a merge in one transaction (FR-BU-07): every reference to `from` moves to
    /// `into`, then `from` is tombstoned with `merged_into_id = into` so references that
    /// arrive later (sync) can follow it (design D-18). Every touched row is logged.
    pub fn apply_category_merge(&mut self, m: &CategoryMergeWrite) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        let set_as = |visibility,
                      tx: &Transaction<'_>,
                      table: &str,
                      column: &str,
                      id: &str,
                      value: &dyn ToSql| {
            self.track(tx, table, id, visibility, |tx| {
                tx.execute(
                    &format!("UPDATE {table} SET {column} = ?2 WHERE id = ?1"),
                    params![id, value],
                )?;
                Ok(())
            })
        };
        let set = |tx: &Transaction<'_>, table: &str, column: &str, id: &str, value: &dyn ToSql| {
            set_as(Visibility::Household, tx, table, column, id, value)
        };
        for child in &m.reparent {
            set(&tx, "category", "parent_id", child, &m.into)?;
        }
        for line in &m.move_budget_lines {
            set(&tx, "budget_line", "category_id", line, &m.into)?;
        }
        for (target, amount, absorbed) in &m.sum_budget_lines {
            set(&tx, "budget_line", "deleted", absorbed, &1)?;
            set(&tx, "budget_line", "amount_minor", target, amount)?;
        }
        // A posting is private when its entry touches a private account (docs/05).
        let postings: Vec<(String, bool)> = {
            let mut stmt = tx.prepare(
                "SELECT p.id, EXISTS (SELECT 1 FROM posting q JOIN account a ON a.id = q.account_id
                                      WHERE q.entry_id = p.entry_id AND a.visibility = 'private')
                 FROM posting p WHERE p.category_id = ?1 ORDER BY p.id",
            )?;
            stmt.query_map([&m.from], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<std::result::Result<_, _>>()?
        };
        for (id, private) in postings {
            let visibility = if private {
                Visibility::Private
            } else {
                Visibility::Household
            };
            set_as(visibility, &tx, "posting", "category_id", &id, &m.into)?;
        }
        for (table, column) in [
            ("payee", "default_category_id"),
            ("payee_rule", "set_category_id"),
            ("recurring_rule_posting", "category_id"),
        ] {
            let ids: Vec<String> = {
                let mut stmt = tx.prepare(&format!(
                    "SELECT id FROM {table} WHERE {column} = ?1 ORDER BY id"
                ))?;
                stmt.query_map([&m.from], |r| r.get(0))?
                    .collect::<std::result::Result<_, _>>()?
            };
            for id in ids {
                set(&tx, table, column, &id, &m.into)?;
            }
        }
        // Staged import rows are local-only and carry no sync columns: not logged.
        tx.execute(
            "UPDATE import_row SET suggested_category_id = ?2 WHERE suggested_category_id = ?1",
            params![m.from, m.into],
        )?;
        self.track(&tx, "category", &m.from, Visibility::Household, |tx| {
            tx.execute(
                "UPDATE category SET deleted = 1, merged_into_id = ?2 WHERE id = ?1",
                params![m.from, m.into],
            )?;
            Ok(())
        })?;
        tx.commit()?;
        Ok(())
    }
}
