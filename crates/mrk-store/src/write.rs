//! One SQL transaction per command (design Q3): core runs its reads and writes inside
//! [`Store::write`], and every replicated row written through [`Write`] is logged by
//! [`Store::track`] (ADR-0010). Dropping the transaction on an error rolls everything back,
//! so a refused command writes nothing.

use rusqlite::{ToSql, Transaction, params_from_iter};

use crate::{Result, Store, StoreError, Visibility};

/// A column and the value written to it. Column names come from this crate's code, never
/// from input.
pub(crate) type Col<'v> = (&'static str, &'v dyn ToSql);

/// An open write transaction.
pub struct Write<'a> {
    store: &'a Store,
    tx: Transaction<'a>,
}

impl Store {
    /// Runs `f` in one transaction, committed only when it returns `Ok`.
    pub fn write<T, E: From<StoreError>>(
        &self,
        f: impl FnOnce(&Write<'_>) -> std::result::Result<T, E>,
    ) -> std::result::Result<T, E> {
        let tx = self
            .conn
            .unchecked_transaction()
            .map_err(StoreError::from)?;
        let w = Write { store: self, tx };
        let out = f(&w)?;
        w.tx.commit().map_err(StoreError::from)?;
        Ok(out)
    }
}

impl Write<'_> {
    pub(crate) fn tx(&self) -> &Transaction<'_> {
        &self.tx
    }

    /// Checks foreign keys at commit instead of per statement, for a change that moves a
    /// composite key in two steps (an account's currency and its postings' currency).
    pub fn defer_foreign_keys(&self) -> Result<()> {
        self.tx.pragma_update(None, "defer_foreign_keys", true)?;
        Ok(())
    }

    /// Inserts one replicated row and logs it as created.
    pub(crate) fn insert(
        &self,
        entity: &str,
        id: &str,
        visibility: Visibility,
        cols: &[Col<'_>],
    ) -> Result<()> {
        let names: Vec<&str> = cols.iter().map(|(c, _)| *c).collect();
        let marks: Vec<String> = (1..=cols.len()).map(|i| format!("?{i}")).collect();
        let sql = format!(
            "INSERT INTO {entity} ({}, hlc, hlc_device) VALUES ({}, 0, '')",
            names.join(", "),
            marks.join(", ")
        );
        self.store.track(&self.tx, entity, id, visibility, |tx| {
            tx.execute(&sql, params_from_iter(cols.iter().map(|(_, v)| *v)))?;
            Ok(())
        })
    }

    /// Sets columns of one replicated row and logs the fields that changed (none: no log).
    pub(crate) fn update(
        &self,
        entity: &str,
        id: &str,
        visibility: Visibility,
        cols: &[Col<'_>],
    ) -> Result<()> {
        let sets: Vec<String> = cols
            .iter()
            .enumerate()
            .map(|(i, (c, _))| format!("{c} = ?{}", i + 1))
            .collect();
        let sql = format!(
            "UPDATE {entity} SET {} WHERE id = ?{}",
            sets.join(", "),
            cols.len() + 1
        );
        self.store.track(&self.tx, entity, id, visibility, |tx| {
            let values = cols.iter().map(|(_, v)| *v).chain([&id as &dyn ToSql]);
            match tx.execute(&sql, params_from_iter(values))? {
                1 => Ok(()),
                _ => Err(StoreError::NotTracked {
                    entity: entity.into(),
                    id: id.into(),
                }),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{HouseholdRow, Store, StoreError};

    fn store() -> Store {
        let mut s = Store::open_in_memory().unwrap();
        s.migrate().unwrap();
        s.ensure_local_device().unwrap();
        s
    }

    fn household() -> HouseholdRow {
        HouseholdRow {
            id: "h".into(),
            name: "Casa".into(),
            base_currency_code: "CLP".into(),
            budget_mode: "per_installment".into(),
            locale: "es-CL".into(),
        }
    }

    fn count(s: &Store, table: &str) -> i64 {
        s.connection()
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn an_error_rolls_back_every_write_and_its_log_rows() {
        let s = store();
        let r: Result<(), StoreError> = s.write(|w| {
            w.insert_household(&household())?;
            Err(StoreError::NoLocalDevice)
        });
        assert!(r.is_err());
        assert_eq!((count(&s, "household"), count(&s, "change_log")), (0, 0));
        s.write(|w| w.insert_household(&household())).unwrap();
        assert_eq!((count(&s, "household"), count(&s, "change_log")), (1, 1));
    }

    #[test]
    fn updating_a_missing_row_is_an_error_and_an_unchanged_row_logs_nothing() {
        let s = store();
        let missing = s.write(|w| {
            w.update_entry(
                &crate::EntryRow {
                    id: "nope".into(),
                    entry_date: "2026-11-01".into(),
                    kind: "regular".into(),
                    description: String::new(),
                    memo: String::new(),
                    deleted: false,
                },
                crate::Visibility::Household,
            )
        });
        assert!(matches!(missing, Err(StoreError::NotTracked { .. })));
        let h = household();
        s.write(|w| w.insert_household(&h)).unwrap();
        s.write(|w| {
            w.update(
                "household",
                &h.id,
                crate::Visibility::Household,
                &[("name", &h.name)],
            )
        })
        .unwrap();
        assert_eq!(count(&s, "change_log"), 1);
    }
}
