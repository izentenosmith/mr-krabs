//! Acceptance tests for slice S1 (sync stamps) at the store boundary: the device migration
//! (REQ-8) and the change-log writer (REQ-7). Written from the brief before the code.

use mrk_store::{MIGRATIONS, Store, Visibility};
use proptest::prelude::*;
use rusqlite::params;
use rusqlite::types::ValueRef;
use serde_json::{Map, Value as Json, json};

fn count(s: &Store, sql: &str) -> i64 {
    s.connection().query_row(sql, [], |r| r.get(0)).unwrap()
}

fn json_of(v: ValueRef<'_>) -> Json {
    match v {
        ValueRef::Null => Json::Null,
        ValueRef::Integer(i) => json!(i),
        ValueRef::Text(t) => json!(String::from_utf8(t.to_vec()).unwrap()),
        other => panic!("unexpected value {other:?}"),
    }
}

/// A store at schema v2, built the way `Store::migrate` applies migrations.
fn at_v2() -> Store {
    let s = Store::open_in_memory().unwrap();
    let c = s.connection();
    for (i, (_, sql)) in MIGRATIONS.iter().enumerate().take(2) {
        c.pragma_update(None, "foreign_keys", false).unwrap();
        c.execute_batch(&format!(
            "BEGIN; {sql}; PRAGMA user_version = {}; COMMIT;",
            i + 1
        ))
        .unwrap();
        c.pragma_update(None, "foreign_keys", true).unwrap();
    }
    assert_eq!(s.schema_version().unwrap(), 2);
    s
}

const FIXTURE: &str = "
    INSERT INTO household (id,name,base_currency_code,hlc,hlc_device) VALUES ('h','Casa','CLP',1,'a');
    INSERT INTO member (id,household_id,display_name,hlc,hlc_device) VALUES ('m','h','A',1,'a');
    INSERT INTO category (id,kind,name,name_key,hlc,hlc_device) VALUES ('c','expense','Comida','comida',1,'a');
";

fn ready() -> Store {
    let mut s = Store::open_in_memory().unwrap();
    s.migrate().unwrap();
    s.connection().execute_batch(FIXTURE).unwrap();
    s.ensure_local_device().unwrap();
    s
}

#[test]
fn v2_database_migrates_with_nullable_device_key_and_member() {
    let mut s = at_v2();
    s.connection()
        .execute_batch(&format!(
            "{FIXTURE}
             INSERT INTO device (id,member_id,name,public_key,is_local) VALUES ('d1','m','Mac',x'0102',1);
             INSERT INTO device (id,member_id,name,public_key,is_local) VALUES ('d2','m','Air',x'0304',0);"
        ))
        .unwrap();
    assert!(Store::latest_version() > 2, "no migration after v2");
    assert_eq!(s.migrate().unwrap(), Store::latest_version());
    let rows: Vec<String> = {
        let mut st = s
            .connection()
            .prepare("SELECT id || ':' || member_id || ':' || hex(public_key) || ':' || is_local FROM device ORDER BY id")
            .unwrap();
        st.query_map([], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    };
    assert_eq!(rows, ["d1:m:0102:1", "d2:m:0304:0"]);
    assert_eq!(
        count(&s, "SELECT count(*) FROM pragma_foreign_key_check"),
        0
    );
    assert_eq!(count(&s, "PRAGMA foreign_keys"), 1);
    s.connection()
        .execute(
            "INSERT INTO device (id,member_id,name,public_key,is_local) VALUES ('d3',NULL,'new',NULL,0)",
            [],
        )
        .unwrap();
}

#[test]
fn creating_the_local_device_writes_no_change_log() {
    let s = ready();
    let id = s.local_device_id().unwrap().to_owned();
    assert_eq!(
        count(
            &s,
            "SELECT count(*) FROM device WHERE is_local = 1 AND public_key IS NULL AND member_id IS NULL"
        ),
        1
    );
    assert_eq!(uuid::Uuid::parse_str(&id).unwrap().get_version_num(), 7);
    assert_eq!(count(&s, "SELECT count(*) FROM change_log"), 0);
    assert_eq!(count(&s, "SELECT count(*) FROM field_clock"), 0);
    // Contrast: a replicated write does leave a row.
    let tx = s.connection().unchecked_transaction().unwrap();
    s.track(&tx, "tag", "t", Visibility::Household, |tx| {
        tx.execute(
            "INSERT INTO tag (id,name,hlc,hlc_device) VALUES ('t','x',0,'')",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    tx.commit().unwrap();
    assert_eq!(count(&s, "SELECT count(*) FROM change_log"), 1);
}

#[test]
fn an_unchanged_row_records_nothing() {
    let s = ready();
    let tx = s.connection().unchecked_transaction().unwrap();
    s.track(&tx, "category", "c", Visibility::Household, |tx| {
        tx.execute("UPDATE category SET name = 'Comida' WHERE id = 'c'", [])?;
        Ok(())
    })
    .unwrap();
    let row = Store::row(&tx, "category", "c").unwrap().unwrap();
    assert_eq!(
        s.record_change(&tx, "category", Visibility::Household, Some(&row), &row)
            .unwrap(),
        None
    );
    tx.commit().unwrap();
    assert_eq!(count(&s, "SELECT count(*) FROM change_log"), 0);
    assert_eq!(count(&s, "SELECT count(*) FROM field_clock"), 0);
    assert_eq!(count(&s, "SELECT hlc FROM category WHERE id = 'c'"), 1);
    // Contrast: a real change is recorded.
    let tx = s.connection().unchecked_transaction().unwrap();
    s.track(&tx, "category", "c", Visibility::Household, |tx| {
        tx.execute("UPDATE category SET name = 'Comidas' WHERE id = 'c'", [])?;
        Ok(())
    })
    .unwrap();
    tx.commit().unwrap();
    assert_eq!(count(&s, "SELECT count(*) FROM change_log"), 1);
}

#[test]
fn a_rolled_back_mutation_leaves_no_change_log() {
    let s = ready();
    {
        let tx = s.connection().unchecked_transaction().unwrap();
        s.track(&tx, "category", "c", Visibility::Household, |tx| {
            tx.execute("UPDATE category SET name = 'X' WHERE id = 'c'", [])?;
            Ok(())
        })
        .unwrap();
        assert_eq!(count(&s, "SELECT count(*) FROM change_log"), 1);
        // dropped without commit
    }
    assert_eq!(count(&s, "SELECT count(*) FROM change_log"), 0);
    assert_eq!(count(&s, "SELECT count(*) FROM field_clock"), 0);
    // A failing mutation propagates its error.
    let tx = s.connection().unchecked_transaction().unwrap();
    let err = s.track(&tx, "category", "c", Visibility::Household, |tx| {
        tx.execute("UPDATE category SET kind = 'bogus' WHERE id = 'c'", [])?;
        Ok(())
    });
    assert!(err.is_err());
}

#[derive(Clone, Debug, PartialEq)]
struct Line {
    period: String,
    amount: i64,
    rollover: i64,
    member: Option<&'static str>,
}

fn line() -> impl Strategy<Value = Line> {
    (1u8..=9, 0i64..1_000_000_000, 0i64..=1, any::<bool>()).prop_map(
        |(m, amount, rollover, mem)| Line {
            period: format!("2026-0{m}"),
            amount,
            rollover,
            member: mem.then_some("m"),
        },
    )
}

fn track_sql(s: &Store, sql: &str, p: &Line) {
    let tx = s.connection().unchecked_transaction().unwrap();
    s.track(&tx, "budget_line", "b", Visibility::Household, |tx| {
        tx.execute(sql, params![p.period, p.amount, p.rollover, p.member])?;
        Ok(())
    })
    .unwrap();
    tx.commit().unwrap();
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// Q5: create → update → delete; replaying `new` in order rebuilds the row, each `old`
    /// is the previous state, and only changed fields appear.
    #[test]
    fn create_update_delete_round_trips_fields_json(a in line(), b in line()) {
        let s = ready();
        track_sql(&s, "INSERT INTO budget_line (id,period,category_id,scope,amount_minor,currency_code,rollover,member_id,hlc,hlc_device) \
                       VALUES ('b',?1,'c',CASE WHEN ?4 IS NULL THEN 'household' ELSE 'personal' END,?2,'CLP',?3,?4,0,'')", &a);
        track_sql(&s, "UPDATE budget_line SET period=?1, amount_minor=?2, rollover=?3, member_id=?4, scope=CASE WHEN ?4 IS NULL THEN 'household' ELSE 'personal' END WHERE id='b'", &b);
        let tx = s.connection().unchecked_transaction().unwrap();
        s.track(&tx, "budget_line", "b", Visibility::Household, |tx| {
            tx.execute("UPDATE budget_line SET deleted = 1 WHERE id = 'b'", [])?;
            Ok(())
        }).unwrap();
        tx.commit().unwrap();

        let device = s.local_device_id().unwrap().to_owned();
        let log: Vec<(i64, String, String, String)> = {
            let mut st = s.connection().prepare(
                "SELECT hlc, device_id, op, fields_json FROM change_log WHERE entity='budget_line' AND entity_id='b' ORDER BY seq").unwrap();
            st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).unwrap().map(Result::unwrap).collect()
        };
        let ops: Vec<&str> = log.iter().map(|l| l.2.as_str()).collect();
        if a == b {
            prop_assert_eq!(ops, ["upsert", "delete"]);
        } else {
            prop_assert_eq!(ops, ["upsert", "upsert", "delete"]);
        }
        let mut state = Map::new();
        let mut last_hlc = 0;
        let mut field_hlc: Map<String, Json> = Map::new();
        for (i, (hlc, dev, _, fields)) in log.iter().enumerate() {
            prop_assert!(*hlc > last_hlc);
            last_hlc = *hlc;
            prop_assert_eq!(dev, &device);
            let fields: Map<String, Json> = serde_json::from_str(fields).unwrap();
            prop_assert!(!fields.is_empty());
            for (f, change) in &fields {
                prop_assert!(!["id", "hlc", "hlc_device"].contains(&f.as_str()), "{} logged", f);
                let old = state.get(f).cloned().unwrap_or(Json::Null);
                prop_assert_eq!(&change["old"], &old, "old of {} in change {}", f, i);
                prop_assert_ne!(&change["old"], &change["new"], "unchanged {} logged", f);
                state.insert(f.clone(), change["new"].clone());
                field_hlc.insert(f.clone(), json!(hlc));
            }
        }
        // The final row, read back, equals the replay.
        let mut st = s.connection().prepare("SELECT * FROM budget_line WHERE id='b'").unwrap();
        let names: Vec<String> = st.column_names().iter().map(|n| n.to_string()).collect();
        let row: Vec<Json> = st.query_row([], |r| Ok((0..names.len()).map(|i| json_of(r.get_ref(i).unwrap())).collect())).unwrap();
        for (name, value) in names.iter().zip(row) {
            match name.as_str() {
                "id" => {}
                "hlc" => prop_assert_eq!(value, json!(last_hlc)),
                "hlc_device" => prop_assert_eq!(value, json!(device)),
                _ => prop_assert_eq!(state.get(name).cloned().unwrap_or(Json::Null), value, "{}", name),
            }
        }
        // field_clock holds the stamp of the last change to each field.
        for (f, hlc) in &field_hlc {
            let (h, d): (i64, String) = s.connection().query_row(
                "SELECT hlc, device_id FROM field_clock WHERE entity='budget_line' AND entity_id='b' AND field=?1",
                [f], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
            prop_assert_eq!(json!(h), hlc.clone());
            prop_assert_eq!(&d, &device);
        }
        prop_assert_eq!(count(&s, "SELECT count(*) FROM field_clock WHERE entity_id='b'"), field_hlc.len() as i64);
    }
}

// ── Remediation slices (ledger iteration 1) ──

struct TempDb(std::path::PathBuf);

impl TempDb {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("mrk-s1-store-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        Self(dir)
    }
    fn path(&self) -> std::path::PathBuf {
        self.0.join("mrkrabs.sqlite")
    }
}

impl Drop for TempDb {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn rename(s: &Store, name: &str) {
    let tx = s.connection().unchecked_transaction().unwrap();
    s.track(&tx, "category", "c", Visibility::Household, |tx| {
        tx.execute("UPDATE category SET name = ?1 WHERE id = 'c'", [name])?;
        Ok(())
    })
    .unwrap();
    tx.commit().unwrap();
}

/// F1: a stale in-memory clock never out-stamps a change another connection committed.
#[test]
fn a_second_store_on_the_same_file_stamps_after_the_first() {
    let db = TempDb::new("two");
    let mut a = Store::open(&db.path()).unwrap();
    a.migrate().unwrap();
    a.connection().execute_batch(FIXTURE).unwrap();
    a.ensure_local_device().unwrap();
    let mut b = Store::open(&db.path()).unwrap();
    b.ensure_local_device().unwrap();
    assert_eq!(a.local_device_id(), b.local_device_id());

    // A's clock runs a day ahead of the wall clock; B loaded its clock before that.
    rename(&a, "A");
    let ahead: i64 = count(&a, "SELECT max(hlc) FROM change_log") + ((24 * 3_600_000_i64) << 16);
    a.connection()
        .execute("UPDATE change_log SET hlc = ?1", [ahead])
        .unwrap();
    a.connection()
        .execute("UPDATE field_clock SET hlc = ?1", [ahead])
        .unwrap();
    rename(&b, "B");
    let (hlc, clock): (i64, i64) = b
        .connection()
        .query_row(
            "SELECT (SELECT max(hlc) FROM change_log WHERE fields_json LIKE '%\"B\"%'), \
                    (SELECT hlc FROM field_clock WHERE entity_id = 'c' AND field = 'name')",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert!(hlc > ahead, "{hlc} <= {ahead}");
    assert_eq!(clock, hlc);
}

/// F5: a local device another connection created first is adopted.
#[test]
fn a_local_device_created_by_another_connection_is_adopted() {
    let db = TempDb::new("adopt");
    let mut a = Store::open(&db.path()).unwrap();
    a.migrate().unwrap();
    let mut b = Store::open(&db.path()).unwrap();
    let id = b.ensure_local_device().unwrap();
    assert_eq!(a.ensure_local_device().unwrap(), id);
    assert_eq!(count(&a, "SELECT count(*) FROM device"), 1);
}

/// F8: restoring the clock reads an index, not the whole log.
#[test]
fn the_clock_restore_uses_an_index() {
    let s = ready();
    let plan: String = s
        .connection()
        .query_row(
            "EXPLAIN QUERY PLAN SELECT max(hlc) FROM change_log",
            [],
            |r| r.get(3),
        )
        .unwrap();
    assert!(plan.contains("ix_change_log_hlc"), "{plan}");
}

fn ops(s: &Store, id: &str) -> Vec<String> {
    let mut st = s
        .connection()
        .prepare("SELECT op FROM change_log WHERE entity_id = ?1 ORDER BY seq")
        .unwrap();
    st.query_map([id], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

fn tag(s: &Store, sql: &str) {
    let tx = s.connection().unchecked_transaction().unwrap();
    s.track(&tx, "tag", "t", Visibility::Household, |tx| {
        tx.execute(sql, [])?;
        Ok(())
    })
    .unwrap();
    tx.commit().unwrap();
}

/// F11: only the live → deleted transition is a delete; restore and tombstone edits upsert.
#[test]
fn restore_and_tombstone_edits_log_upsert() {
    let s = ready();
    tag(
        &s,
        "INSERT INTO tag (id,name,hlc,hlc_device) VALUES ('t','x',0,'')",
    );
    tag(&s, "UPDATE tag SET deleted = 1 WHERE id = 't'");
    tag(&s, "UPDATE tag SET name = 'y' WHERE id = 't'");
    tag(&s, "UPDATE tag SET deleted = 0 WHERE id = 't'");
    assert_eq!(ops(&s, "t"), ["upsert", "delete", "upsert", "upsert"]);
}

/// F11: values the log cannot carry are refused, and the write rolls back with them.
#[test]
fn real_and_blob_changes_are_refused() {
    let s = ready();
    s.connection()
        .execute_batch(
            "CREATE TABLE odd (id TEXT PRIMARY KEY, r REAL, b BLOB, hlc INTEGER NOT NULL DEFAULT 0, \
             hlc_device TEXT NOT NULL DEFAULT '', deleted INTEGER NOT NULL DEFAULT 0)",
        )
        .unwrap();
    for (id, sql) in [
        ("r", "INSERT INTO odd (id, r) VALUES ('r', 1.5)"),
        ("b", "INSERT INTO odd (id, b) VALUES ('b', x'00')"),
    ] {
        let tx = s.connection().unchecked_transaction().unwrap();
        let err = s
            .track(&tx, "odd", id, Visibility::Household, |tx| {
                tx.execute(sql, [])?;
                Ok(())
            })
            .unwrap_err();
        assert!(
            matches!(err, mrk_store::StoreError::UnloggableValue { .. }),
            "{err}"
        );
    }
    assert_eq!(count(&s, "SELECT count(*) FROM change_log"), 0);
}
