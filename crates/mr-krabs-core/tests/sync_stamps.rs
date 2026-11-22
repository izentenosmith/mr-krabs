//! Acceptance tests for slice S1 (sync stamps) through the `App` API: the local device
//! (REQ-8), the persisted clock, and `category_delete` writing the change log (REQ-7).
//! The database is read back through a second connection to the same file.

use std::path::PathBuf;

use mr_krabs_core::App;
use mr_krabs_core::seed::seed_id;
use mrk_store::Store;
use serde_json::{Value as Json, json};

struct Db {
    dir: PathBuf,
    path: PathBuf,
}

impl Db {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("mrk-s1-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        Self {
            path: dir.join("mrkrabs.sqlite"),
            dir,
        }
    }
    fn raw(&self) -> Store {
        Store::open(&self.path).unwrap()
    }
    fn int(&self, sql: &str) -> i64 {
        self.raw().connection().query_row(sql, [], |r| r.get(0)).unwrap()
    }
    fn text(&self, sql: &str) -> String {
        self.raw().connection().query_row(sql, [], |r| r.get(0)).unwrap()
    }
    fn local_device(&self) -> String {
        self.text("SELECT id FROM device WHERE is_local = 1")
    }
    fn log(&self) -> Vec<Log> {
        let raw = self.raw();
        let mut st = raw
            .connection()
            .prepare(
                "SELECT hlc, device_id, entity, entity_id, op, fields_json, visibility \
                 FROM change_log ORDER BY seq",
            )
            .unwrap();
        st.query_map([], |r| {
            Ok(Log {
                hlc: r.get(0)?,
                device: r.get(1)?,
                entity: r.get(2)?,
                id: r.get(3)?,
                op: r.get(4)?,
                fields: serde_json::from_str(&r.get::<_, String>(5)?).unwrap(),
                visibility: r.get(6)?,
            })
        })
        .unwrap()
        .map(Result::unwrap)
        .collect()
    }
}

impl Drop for Db {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[derive(Debug)]
struct Log {
    hlc: i64,
    device: String,
    entity: String,
    id: String,
    op: String,
    fields: Json,
    visibility: String,
}

fn household_app(db: &Db) -> App {
    let mut app = App::open(&db.path).unwrap();
    db.raw()
        .connection()
        .execute(
            "INSERT INTO household (id, name, base_currency_code, hlc, hlc_device) \
             VALUES ('h', 'Casa', 'CLP', 1, 'dev-a')",
            [],
        )
        .unwrap();
    app.seed_household_defaults().unwrap();
    app
}

/// The merge fixture: a posting, a sub-category, budget lines (one month colliding), a payee
/// default and a payee rule all pointing at Supermercado.
fn merge_fixture(db: &Db) -> (App, String, String) {
    let app = household_app(db);
    let (sup, del) = (
        seed_id("expense.food.supermarket"),
        seed_id("expense.food.delivery"),
    );
    let savings = seed_id("account.savings");
    db.raw()
        .connection()
        .execute_batch(&format!(
            "INSERT INTO journal_entry (id, entry_date, hlc, hlc_device) VALUES ('e', '2026-11-02', 1, 'dev-a');
             INSERT INTO posting (id, entry_id, leg, account_id, amount_minor, currency_code, status, hlc, hlc_device)
                 VALUES ('p1', 'e', 'account', '{savings}', -45000, 'CLP', 'cleared', 1, 'dev-a');
             INSERT INTO posting (id, entry_id, leg, category_id, amount_minor, currency_code, hlc, hlc_device)
                 VALUES ('p2', 'e', 'category', '{sup}', 45000, 'CLP', 1, 'dev-a');
             INSERT INTO category (id, parent_id, kind, name, name_key, hlc, hlc_device)
                 VALUES ('organic', '{sup}', 'expense', 'Orgánico', 'organico', 1, 'dev-a');
             INSERT INTO budget_line (id, period, category_id, scope, amount_minor, currency_code, hlc, hlc_device) VALUES
                 ('b1', '2026-11', '{sup}', 'household', 300000, 'CLP', 1, 'dev-a'),
                 ('b2', '2026-12', '{sup}', 'household', 280000, 'CLP', 1, 'dev-a'),
                 ('b3', '2026-11', '{del}', 'household',  40000, 'CLP', 1, 'dev-a');
             INSERT INTO payee (id, name, default_category_id, hlc, hlc_device) VALUES ('py', 'Super', '{sup}', 1, 'dev-a');
             INSERT INTO payee_rule (id, match_kind, pattern, set_category_id, hlc, hlc_device)
                 VALUES ('pr', 'contains', 'SUPER', '{sup}', 1, 'dev-a');"
        ))
        .unwrap();
    (app, sup, del)
}

fn find<'a>(log: &'a [Log], entity: &str, id: &str) -> &'a Log {
    log.iter()
        .find(|l| l.entity == entity && l.id == id)
        .unwrap_or_else(|| panic!("no log row for {entity}/{id} in {log:?}"))
}

#[test]
fn category_merge_logs_every_touched_row_with_the_local_device_and_increasing_hlc() {
    let db = Db::new("merge");
    let (mut app, sup, del) = merge_fixture(&db);
    app.category_delete(&sup, Some(&del)).unwrap();

    let log = db.log();
    let device = db.local_device();
    let mut touched: Vec<(String, String)> =
        log.iter().map(|l| (l.entity.clone(), l.id.clone())).collect();
    touched.sort();
    let mut want: Vec<(String, String)> = [
        ("budget_line", "b1"),
        ("budget_line", "b2"),
        ("budget_line", "b3"),
        ("category", "organic"),
        ("category", sup.as_str()),
        ("payee", "py"),
        ("payee_rule", "pr"),
        ("posting", "p2"),
    ]
    .iter()
    .map(|(e, i)| (e.to_string(), i.to_string()))
    .collect();
    want.sort();
    assert_eq!(touched, want);
    let mut last = 0;
    for l in &log {
        assert_eq!(l.device, device);
        assert!(l.hlc > last, "hlc not increasing: {log:?}");
        last = l.hlc;
        assert_eq!(l.visibility, "household");
        let (hlc, dev): (i64, String) = db
            .raw()
            .connection()
            .query_row(
                &format!("SELECT hlc, hlc_device FROM {} WHERE id = ?1", l.entity),
                [&l.id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!((hlc, dev), (l.hlc, device.clone()), "{}/{}", l.entity, l.id);
    }
    assert_eq!(find(&log, "budget_line", "b1").op, "delete");
    assert_eq!(find(&log, "category", &sup).op, "delete");
    assert_eq!(find(&log, "posting", "p2").op, "upsert");
}

#[test]
fn deleting_an_unused_category_logs_one_delete_with_its_field_clock() {
    let db = Db::new("delete");
    let mut app = household_app(&db);
    let cinema = seed_id("expense.leisure.cinema");
    app.category_delete(&cinema, None).unwrap();

    let log = db.log();
    assert_eq!(log.len(), 1, "{log:?}");
    let l = &log[0];
    assert_eq!((l.entity.as_str(), l.id.as_str(), l.op.as_str()), ("category", cinema.as_str(), "delete"));
    assert_eq!(l.fields, json!({"deleted": {"old": 0, "new": 1}}));
    let (hlc, dev): (i64, String) = db
        .raw()
        .connection()
        .query_row(
            "SELECT hlc, device_id FROM field_clock WHERE entity = 'category' AND entity_id = ?1 AND field = 'deleted'",
            [&cinema],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!((hlc, dev), (l.hlc, db.local_device()));
    assert_eq!(db.int("SELECT count(*) FROM field_clock"), 1);
}

#[test]
fn merge_log_rows_carry_only_the_changed_fields() {
    let db = Db::new("fields");
    let (mut app, sup, del) = merge_fixture(&db);
    app.category_delete(&sup, Some(&del)).unwrap();
    let log = db.log();
    assert_eq!(
        find(&log, "budget_line", "b3").fields,
        json!({"amount_minor": {"old": 40000, "new": 340000}})
    );
    assert_eq!(
        find(&log, "posting", "p2").fields,
        json!({"category_id": {"old": sup, "new": del}})
    );
    assert_eq!(
        find(&log, "category", &sup).fields,
        json!({"deleted": {"old": 0, "new": 1}, "merged_into_id": {"old": null, "new": del}})
    );
    assert_eq!(
        find(&log, "budget_line", "b1").fields,
        json!({"deleted": {"old": 0, "new": 1}})
    );
}

#[test]
fn opening_twice_keeps_one_local_device() {
    let db = Db::new("device");
    drop(App::open(&db.path).unwrap());
    let first = db.local_device();
    drop(App::open(&db.path).unwrap());
    assert_eq!(db.local_device(), first);
    assert_eq!(db.int("SELECT count(*) FROM device WHERE is_local = 1"), 1);
    assert_eq!(db.int("SELECT count(*) FROM device"), 1);
    assert_eq!(
        db.int("SELECT count(*) FROM device WHERE public_key IS NULL AND member_id IS NULL"),
        1
    );
    assert_eq!(uuid::Uuid::parse_str(&first).unwrap().get_version_num(), 7);
}

#[test]
fn stamps_after_a_restart_are_greater_than_every_earlier_stamp() {
    let db = Db::new("restart");
    let mut app = household_app(&db);
    app.category_delete(&seed_id("expense.leisure.cinema"), None).unwrap();
    let first = db.log()[0].hlc;
    drop(app);
    // A stamp from a clock running ten days ahead (e.g. the wall clock later went back).
    let ahead = first + ((10 * 24 * 3_600_000_i64) << 16);
    db.raw()
        .connection()
        .execute(
            "INSERT INTO change_log (change_id, hlc, device_id, entity, entity_id, op, fields_json, visibility, applied_at) \
             VALUES ('x', ?1, 'other', 'tag', 't', 'upsert', '{}', 'household', '2026-11-22')",
            [ahead],
        )
        .unwrap();
    let mut app = App::open(&db.path).unwrap();
    app.category_delete(&seed_id("expense.food.delivery"), None).unwrap();
    let last = db.log().last().unwrap().hlc;
    assert!(last > ahead && ahead > first, "{first} {ahead} {last}");
}

#[test]
fn seeding_writes_no_change_log_but_edits_do() {
    let db = Db::new("seed");
    let mut app = household_app(&db);
    assert_eq!(db.int("SELECT count(*) FROM change_log"), 0);
    assert_eq!(db.int("SELECT max(hlc) FROM category"), 0);
    assert_eq!(db.int("SELECT count(*) FROM category WHERE hlc_device <> 'seed'"), 0);
    // Contrast: a user edit on a seeded row is logged.
    app.category_delete(&seed_id("expense.leisure.cinema"), None).unwrap();
    assert_eq!(db.int("SELECT count(*) FROM change_log"), 1);
}
