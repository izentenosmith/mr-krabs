//! The change-log writer (docs/05, ADR-0010): the only code that writes `change_log` and
//! `field_clock`, called inside the mutation's transaction.

use std::collections::BTreeMap;

use rusqlite::Transaction;
use rusqlite::types::Value;

use crate::{Result, Stamp, Store};

/// A row as column → SQLite value.
pub type Row = BTreeMap<String, Value>;

/// Who a change may be sent to at sync (docs/05 "Visibility filtering").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Visibility {
    Household,
    Private,
}

impl Store {
    /// Creates the local device (UUIDv7, no key, no member yet) unless one exists, and
    /// restores the clock from the log. Returns the local device id.
    pub fn ensure_local_device(&mut self) -> Result<String> {
        todo!("S1: ensure_local_device")
    }

    /// The local device id, once [`Store::ensure_local_device`] ran.
    pub fn local_device_id(&self) -> Option<&str> {
        todo!("S1: local_device_id")
    }

    /// Reads one row of a replicated table by id.
    pub fn row(_tx: &Transaction<'_>, _entity: &str, _id: &str) -> Result<Option<Row>> {
        todo!("S1: row")
    }

    /// Logs the change from `before` to `after` of one row: one `change_log` row with the
    /// changed fields only, one `field_clock` upsert per changed field, and the row stamped.
    /// Returns the stamp, or `None` when nothing changed.
    pub fn record_change(
        &self,
        _tx: &Transaction<'_>,
        _entity: &str,
        _visibility: Visibility,
        _before: Option<&Row>,
        _after: &Row,
    ) -> Result<Option<Stamp>> {
        todo!("S1: record_change")
    }

    /// Runs `mutate` on one row and logs what it changed.
    pub fn track<T>(
        &self,
        _tx: &Transaction<'_>,
        _entity: &str,
        _id: &str,
        _visibility: Visibility,
        _mutate: impl FnOnce(&Transaction<'_>) -> Result<T>,
    ) -> Result<T> {
        todo!("S1: track")
    }
}
