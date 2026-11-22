//! The change-log writer (docs/05, ADR-0010): the only code that writes `change_log` and
//! `field_clock`, called inside the mutation's transaction.
//!
//! `fields_json` holds the changed fields only, `{"<field>": {"old": v|null, "new": v}}`, with
//! each value as the column's SQLite value (design Q5). The local device's HLC stamps every
//! change; the clock is restored from the log at open, so stamps keep increasing across
//! restarts. Seeded rows (ADR-0008) and the local-only `device` table are never logged.

use std::cell::Cell;
use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use mrk_sync::{Hlc, HlcClock};
use rusqlite::types::Value;
use rusqlite::{OptionalExtension, Transaction, params};
use serde_json::{Map, Value as Json, json};

use crate::{Result, Stamp, Store, StoreError};

/// A row as column → SQLite value.
pub type Row = BTreeMap<String, Value>;

/// Who a change may be sent to at sync (docs/05 "Visibility filtering").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Visibility {
    Household,
    Private,
}

impl Visibility {
    fn as_str(self) -> &'static str {
        match self {
            Self::Household => "household",
            Self::Private => "private",
        }
    }
}

/// Columns that identify or stamp a row; never logged as fields.
const NOT_FIELDS: [&str; 3] = ["id", "hlc", "hlc_device"];

/// The local device and the last HLC it issued.
#[derive(Debug)]
pub(crate) struct LocalClock {
    device: String,
    last: Cell<Hlc>,
}

impl LocalClock {
    /// Issues the next stamp inside `tx`. The log is re-read first, so a change another
    /// connection committed is never out-stamped by a stale in-memory clock.
    fn tick(&self, tx: &Transaction<'_>) -> Result<Stamp> {
        let logged = max_logged_hlc(tx)?;
        if logged > self.last.get() {
            self.last.set(logged);
        }
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);
        let mut clock = HlcClock::new(self.last.get());
        let hlc = clock.tick(now_ms).map_err(StoreError::Clock)?;
        self.last.set(hlc);
        Ok(Stamp {
            hlc: i64::try_from(hlc.as_u64())
                .map_err(|_| StoreError::Clock(mrk_sync::HlcError::CounterOverflow))?,
            device: self.device.clone(),
        })
    }
}

impl Store {
    /// Creates the local device (UUIDv7, no key, no member yet) unless one exists, and
    /// restores the clock from the log. Returns the local device id.
    ///
    /// `device` is a local-only trust store, so this write is not logged.
    pub fn ensure_local_device(&mut self) -> Result<String> {
        // INSERT OR IGNORE: a device another connection created first is adopted.
        self.conn.execute(
            "INSERT OR IGNORE INTO device (id, member_id, name, public_key, is_local) \
             VALUES (?1, NULL, 'local', NULL, 1)",
            [uuid::Uuid::now_v7().to_string()],
        )?;
        let device: String =
            self.conn
                .query_row("SELECT id FROM device WHERE is_local = 1", [], |r| r.get(0))?;
        let last = max_logged_hlc(&self.conn)?;
        self.local = Some(LocalClock {
            device: device.clone(),
            last: Cell::new(last),
        });
        Ok(device)
    }

    /// The local device id, once [`Store::ensure_local_device`] ran.
    pub fn local_device_id(&self) -> Option<&str> {
        self.local.as_ref().map(|l| l.device.as_str())
    }

    /// Reads one row of a replicated table by id.
    pub fn row(tx: &Transaction<'_>, entity: &str, id: &str) -> Result<Option<Row>> {
        check_entity(entity, id)?;
        let mut stmt = tx.prepare(&format!("SELECT * FROM {entity} WHERE id = ?1"))?;
        let names: Vec<String> = stmt.column_names().iter().map(|n| n.to_string()).collect();
        Ok(stmt
            .query_row([id], |r| {
                names
                    .iter()
                    .enumerate()
                    .map(|(i, n)| Ok((n.clone(), r.get::<_, Value>(i)?)))
                    .collect()
            })
            .optional()?)
    }

    /// Logs the change from `before` to `after` of one row: one `change_log` row with the
    /// changed fields only, one `field_clock` upsert per changed field, and the row stamped.
    /// Returns the stamp, or `None` when nothing changed.
    pub fn record_change(
        &self,
        tx: &Transaction<'_>,
        entity: &str,
        visibility: Visibility,
        before: Option<&Row>,
        after: &Row,
    ) -> Result<Option<Stamp>> {
        let local = self.local.as_ref().ok_or(StoreError::NoLocalDevice)?;
        let id = match after.get("id") {
            Some(Value::Text(id)) => id.as_str(),
            _ => {
                return Err(StoreError::NotTracked {
                    entity: entity.into(),
                    id: String::new(),
                });
            }
        };
        check_entity(entity, id)?;
        let mut fields = Map::new();
        for (field, new) in after {
            if NOT_FIELDS.contains(&field.as_str()) {
                continue;
            }
            let old = before.and_then(|b| b.get(field)).unwrap_or(&Value::Null);
            if old != new {
                let value = |v| to_json(v, entity, field);
                fields.insert(
                    field.clone(),
                    json!({"old": value(old)?, "new": value(new)?}),
                );
            }
        }
        if fields.is_empty() {
            return Ok(None);
        }
        let deleted =
            |r: Option<&Row>| r.and_then(|r| r.get("deleted")) == Some(&Value::Integer(1));
        let op = if deleted(Some(after)) && !deleted(before) {
            "delete"
        } else {
            "upsert"
        };

        let stamp = local.tick(tx)?;
        tx.execute(
            &format!("UPDATE {entity} SET hlc = ?1, hlc_device = ?2 WHERE id = ?3"),
            params![stamp.hlc, stamp.device, id],
        )?;
        tx.execute(
            "INSERT INTO change_log \
             (change_id, hlc, device_id, entity, entity_id, op, fields_json, visibility, applied_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
            params![
                uuid::Uuid::now_v7().to_string(),
                stamp.hlc,
                stamp.device,
                entity,
                id,
                op,
                Json::Object(fields.clone()).to_string(),
                visibility.as_str(),
            ],
        )?;
        let mut clock = tx.prepare_cached(
            "INSERT INTO field_clock (entity, entity_id, field, hlc, device_id) \
             VALUES (?1, ?2, ?3, ?4, ?5) \
             ON CONFLICT (entity, entity_id, field) \
             DO UPDATE SET hlc = excluded.hlc, device_id = excluded.device_id \
             WHERE (excluded.hlc, excluded.device_id) > (field_clock.hlc, field_clock.device_id)",
        )?;
        for field in fields.keys() {
            clock.execute(params![entity, id, field, stamp.hlc, stamp.device])?;
        }
        Ok(Some(stamp))
    }

    /// Runs `mutate` on one row and logs what it changed (see [`Store::record_change`]).
    pub fn track<T>(
        &self,
        tx: &Transaction<'_>,
        entity: &str,
        id: &str,
        visibility: Visibility,
        mutate: impl FnOnce(&Transaction<'_>) -> Result<T>,
    ) -> Result<T> {
        let before = Self::row(tx, entity, id)?;
        let out = mutate(tx)?;
        let after = Self::row(tx, entity, id)?.ok_or_else(|| StoreError::NotTracked {
            entity: entity.into(),
            id: id.into(),
        })?;
        self.record_change(tx, entity, visibility, before.as_ref(), &after)?;
        Ok(out)
    }
}

/// The highest stamp in the log, from any device (`ix_change_log_hlc`).
fn max_logged_hlc(conn: &rusqlite::Connection) -> Result<Hlc> {
    let max: Option<i64> = conn.query_row("SELECT max(hlc) FROM change_log", [], |r| r.get(0))?;
    Ok(Hlc::from_u64(
        max.and_then(|m| u64::try_from(m).ok()).unwrap_or(0),
    ))
}

/// Table names are spliced into SQL, so only plain identifiers pass; `device` is local-only.
fn check_entity(entity: &str, id: &str) -> Result<()> {
    let ident = !entity.is_empty() && entity.chars().all(|c| c.is_ascii_lowercase() || c == '_');
    if ident && entity != "device" {
        Ok(())
    } else {
        Err(StoreError::NotTracked {
            entity: entity.into(),
            id: id.into(),
        })
    }
}

fn to_json(v: &Value, entity: &str, field: &str) -> Result<Json> {
    match v {
        Value::Null => Ok(Json::Null),
        Value::Integer(i) => Ok(json!(i)),
        Value::Text(t) => Ok(json!(t)),
        _ => Err(StoreError::UnloggableValue {
            entity: entity.into(),
            field: field.into(),
        }),
    }
}
