//! Acceptance tests for slice S5 (transaction commands): REQ-3, REQ-4, REQ-5, REQ-6.
//! Every test acts through `commands::dispatch` on an app opened on a temp file; what was
//! persisted (legs, statuses, change log) is read back through a second connection.
//! One test per brief criterion, named after it (`.workflow/brief.md`, Acceptance tests).

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use mr_krabs_core::commands::{COMMANDS, dispatch};
use mr_krabs_core::seed::seed_id;
use mr_krabs_core::{ApiError, App, ErrorCode};
use mrk_store::Store;
use serde_json::{Value, json};

struct Env {
    dir: PathBuf,
    path: PathBuf,
    app: App,
}

impl Env {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("mrk-s5-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("mrkrabs.sqlite");
        let app = App::open(&path).unwrap();
        let mut env = Self { dir, path, app };
        env.ok(
            "household_create",
            json!({ "input": { "name": "Casa", "myName": "Yo", "baseCurrency": "CLP" } }),
        );
        env
    }

    fn call(&mut self, name: &str, args: Value) -> Result<Value, ApiError> {
        dispatch(&mut self.app, name, args)
    }
    fn ok(&mut self, name: &str, args: Value) -> Value {
        self.call(name, args.clone())
            .unwrap_or_else(|e| panic!("{name} {args} failed: {e} {:?}", e.fields))
    }
    fn err(&mut self, name: &str, args: Value) -> ApiError {
        match self.call(name, args.clone()) {
            Ok(v) => panic!("{name} {args} succeeded: {v}"),
            Err(e) => e,
        }
    }

    fn raw(&self) -> Store {
        Store::open(&self.path).unwrap()
    }
    fn exec(&self, sql: &str) {
        self.raw().connection().execute_batch(sql).unwrap();
    }
    fn int(&self, sql: &str) -> i64 {
        self.raw()
            .connection()
            .query_row(sql, [], |r| r.get(0))
            .unwrap()
    }
    fn texts(&self, sql: &str) -> Vec<String> {
        let raw = self.raw();
        let mut st = raw.connection().prepare(sql).unwrap();
        st.query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    }

    /// `(entity, entity_id, op, fields_json)` in log order.
    fn log(&self) -> Vec<(String, String, String, Value)> {
        let raw = self.raw();
        let mut st = raw
            .connection()
            .prepare("SELECT entity, entity_id, op, fields_json FROM change_log ORDER BY seq")
            .unwrap();
        st.query_map([], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                serde_json::from_str(&r.get::<_, String>(3)?).unwrap(),
            ))
        })
        .unwrap()
        .map(Result::unwrap)
        .collect()
    }

    /// Everything a command could have written, to prove a refused one wrote nothing.
    fn state(&self) -> String {
        let mut out = Vec::new();
        for t in [
            "account",
            "journal_entry",
            "posting",
            "reconciliation",
            "change_log",
            "field_clock",
        ] {
            let raw = self.raw();
            let mut st = raw
                .connection()
                .prepare(&format!("SELECT * FROM {t}"))
                .unwrap();
            let cols = st.column_count();
            let mut rows: Vec<String> = st
                .query_map([], |r| {
                    Ok((0..cols)
                        .map(|i| format!("{:?}", r.get_ref(i).unwrap()))
                        .collect::<Vec<_>>()
                        .join(","))
                })
                .unwrap()
                .map(Result::unwrap)
                .collect();
            rows.sort();
            out.push(format!("{t}:{}", rows.join(";")));
        }
        out.join("|")
    }

    fn account(&mut self, name: &str, opening: Option<&str>, personal: bool) -> String {
        let mut input = json!({ "name": name, "kind": "checking", "currency": "CLP" });
        if let Some(a) = opening {
            input["openingBalance"] = json!({ "amount": clp(a), "date": "2026-10-01" });
        }
        if personal {
            input["scope"] = json!("personal");
        }
        self.ok("account_create", json!({ "input": input }))["id"]
            .as_str()
            .unwrap()
            .to_owned()
    }
    fn balance(&mut self, account: &str) -> String {
        self.ok("account_get", json!({ "id": account }))["balance"]["minor"]
            .as_str()
            .unwrap()
            .to_owned()
    }
    fn create(&mut self, input: Value) -> Value {
        self.ok("tx_create", json!({ "input": input }))
    }
    fn create_id(&mut self, input: Value) -> String {
        self.create(input)["id"].as_str().unwrap().to_owned()
    }
    fn get(&mut self, id: &str) -> Value {
        self.ok("tx_get", json!({ "id": id }))
    }
    fn list(&mut self, query: Value) -> Value {
        self.ok("tx_list", json!({ "query": query }))
    }
    fn list_ids(&mut self, query: Value) -> BTreeSet<String> {
        self.list(query)["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["id"].as_str().unwrap().to_owned())
            .collect()
    }
    fn row(&mut self, query: Value, id: &str) -> Value {
        self.list(query)["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == id)
            .cloned()
            .unwrap_or_else(|| panic!("{id} not listed"))
    }
    fn posting_ids(&self, entry: &str) -> Vec<String> {
        self.texts(&format!(
            "SELECT id FROM posting WHERE entry_id = '{entry}' ORDER BY id"
        ))
    }
    /// `(leg, account_id, category_id, amount_minor, status)` of every posting, by id.
    fn legs(
        &self,
        entry: &str,
    ) -> Vec<(String, Option<String>, Option<String>, i64, Option<String>)> {
        let raw = self.raw();
        let mut st = raw
            .connection()
            .prepare(
                "SELECT leg, account_id, category_id, amount_minor, status FROM posting \
                 WHERE entry_id = ?1 ORDER BY id",
            )
            .unwrap();
        st.query_map([entry], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })
        .unwrap()
        .map(Result::unwrap)
        .collect()
    }
    /// Marks the entry's account legs reconciled against a statement of `account`.
    fn reconcile(&self, entry: &str, account: &str) {
        self.exec(&format!(
            "INSERT INTO reconciliation (id, account_id, statement_date, statement_balance_minor, \
                 status, hlc, hlc_device) \
             VALUES ('rec-{entry}', '{account}', '2026-11-30', 0, 'open', 0, ''); \
             UPDATE posting SET status = 'reconciled', reconciliation_id = 'rec-{entry}' \
             WHERE entry_id = '{entry}' AND leg = 'account' AND account_id = '{account}';"
        ));
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn clp(amount: &str) -> Value {
    json!({ "amount": amount, "currency": "CLP" })
}

fn food() -> String {
    seed_id("expense.food.supermarket")
}
fn salary() -> String {
    seed_id("income.salary")
}

fn expense(account: &str, amount: &str, date: &str) -> Value {
    json!({
        "type": "expense", "accountId": account, "categoryId": food(),
        "amount": clp(amount), "date": date, "description": "Supermercado"
    })
}
fn income(account: &str, amount: &str, date: &str) -> Value {
    json!({
        "type": "income", "accountId": account, "categoryId": salary(),
        "amount": clp(amount), "date": date, "description": "Sueldo"
    })
}
fn transfer(from: &str, to: &str, amount: &str, date: &str) -> Value {
    json!({
        "type": "transfer", "fromAccountId": from, "toAccountId": to,
        "amount": clp(amount), "date": date, "description": "Ahorro"
    })
}

fn field_error(e: &ApiError, code: ErrorCode, field: &str, key: &str) {
    assert_eq!(e.code, code, "{e}");
    assert_eq!(
        e.fields
            .as_ref()
            .and_then(|f| f.get(field))
            .map(String::as_str),
        Some(key),
        "fields of {e}: {:?}",
        e.fields
    );
}

fn uuid() -> &'static str {
    "0190f0f0-0000-7000-8000-000000000000"
}

fn keys(v: &Value) -> BTreeSet<String> {
    v.as_object().unwrap().keys().cloned().collect()
}

const MINUS: char = '\u{2212}';

// ───────────────────────────── criteria ─────────────────────────────

#[test]
fn a01_tracer_expense_is_created_read_back_and_lowers_the_balance() {
    let mut env = Env::new("a01");
    let checking = env.account("Cuenta corriente", None, false);
    let created = env.create(expense(&checking, "45.990", "2026-11-21"));
    assert_eq!(created["kind"], "expense");
    assert_eq!(created["accountId"], checking.as_str());
    assert_eq!(created["categoryId"], food().as_str());
    assert_eq!(created["amount"]["minor"], "45990");
    assert_eq!(created["amount"]["display"], "$45.990");
    assert_eq!(created["status"], "cleared");
    assert_eq!(created["date"], "2026-11-21");
    assert_eq!(created["description"], "Supermercado");
    assert_eq!(created["deleted"], false);
    let id = created["id"].as_str().unwrap().to_owned();
    assert_eq!(env.get(&id), created);
    assert_eq!(env.balance(&checking), "-45990");
}

#[test]
fn a02_create_writes_one_regular_entry_two_legs_and_three_create_log_rows() {
    let mut env = Env::new("a02");
    let checking = env.account("Cuenta corriente", None, false);
    let before = env.log().len();
    let id = env.create_id(expense(&checking, "45.990", "2026-11-21"));
    assert_eq!(
        env.texts(&format!("SELECT kind FROM journal_entry WHERE id = '{id}'")),
        vec!["regular"]
    );
    let mut legs = env.legs(&id);
    legs.sort_by_key(|l| l.0.clone());
    assert_eq!(
        legs,
        vec![
            (
                "account".into(),
                Some(checking.clone()),
                None,
                -45990,
                Some("cleared".into())
            ),
            ("category".into(), None, Some(food()), 45990, None),
        ]
    );
    let new_rows: Vec<_> = env.log().into_iter().skip(before).collect();
    assert_eq!(new_rows.len(), 3, "{new_rows:?}");
    let mut logged: Vec<(String, String)> = new_rows
        .iter()
        .map(|(entity, eid, op, _)| (entity.clone(), format!("{op}:{}", eid == &id)))
        .collect();
    logged.sort();
    assert_eq!(
        logged,
        vec![
            ("journal_entry".into(), "create:true".into()),
            ("posting".into(), "create:false".into()),
            ("posting".into(), "create:false".into()),
        ]
    );
    for (_, _, _, fields) in &new_rows {
        for (k, v) in fields.as_object().unwrap() {
            assert_eq!(v["old"], Value::Null, "{k} in {fields}");
        }
    }
    let posting_ids: BTreeSet<_> = new_rows
        .iter()
        .filter(|r| r.0 == "posting")
        .map(|r| r.1.clone())
        .collect();
    assert_eq!(posting_ids, env.posting_ids(&id).into_iter().collect());
}

#[test]
fn a03_transfer_moves_both_balances_has_no_category_leg_and_is_never_an_expense() {
    let mut env = Env::new("a03");
    let checking = env.account("Cuenta corriente", Some("500.000"), false);
    let savings = env.account("Ahorro", None, false);
    let t = env.create(transfer(&checking, &savings, "100.000", "2026-11-21"));
    assert_eq!(t["kind"], "transfer");
    assert_eq!(t["accountId"], checking.as_str());
    assert_eq!(t["toAccountId"], savings.as_str());
    assert_eq!(t["categoryId"], Value::Null);
    assert_eq!(t["amount"]["minor"], "100000");
    assert_eq!(env.balance(&checking), "400000");
    assert_eq!(env.balance(&savings), "100000");
    let id = t["id"].as_str().unwrap().to_owned();
    let legs = env.legs(&id);
    assert!(legs.iter().all(|l| l.0 == "account"), "{legs:?}");
    assert_eq!(
        env.int(&format!(
            "SELECT count(*) FROM posting WHERE entry_id = '{id}' AND category_id IS NOT NULL"
        )),
        0
    );
    let row = env.row(json!({ "view": "combined" }), &id);
    assert_eq!(row["kind"], "transfer");
    assert!(
        !row["amount"]["display"]
            .as_str()
            .unwrap()
            .starts_with(MINUS)
    );
    assert!(!row["amount"]["display"].as_str().unwrap().starts_with('-'));
    assert_eq!(row["categoryId"], Value::Null);
}

#[test]
fn a04_list_rows_carry_master_sign_rules_and_are_signed_from_the_seen_account() {
    let mut env = Env::new("a04");
    let checking = env.account("Cuenta corriente", Some("500.000"), false);
    let savings = env.account("Ahorro", None, false);
    let e = env.create_id(expense(&checking, "45.990", "2026-11-21"));
    let i = env.create_id(income(&checking, "1.000.000", "2026-11-20"));
    let t = env.create_id(transfer(&checking, &savings, "100.000", "2026-11-19"));
    let all = json!({ "view": "combined" });

    let er = env.row(all.clone(), &e);
    assert_eq!(er["amount"]["minor"], "-45990");
    assert_eq!(er["amount"]["display"], format!("{MINUS}$45.990"));
    assert_eq!(er["categoryId"], food().as_str());
    assert_eq!(er["categoryName"], "Supermercado");
    assert_eq!(er["accountName"], "Cuenta corriente");
    assert_eq!(er["status"], "cleared");

    let ir = env.row(all.clone(), &i);
    assert_eq!(ir["amount"]["minor"], "1000000");
    assert_eq!(ir["amount"]["display"], "+$1.000.000");

    let tr = env.row(all.clone(), &t);
    assert_eq!(tr["amount"]["minor"], "-100000");
    assert_eq!(tr["amount"]["display"], "$100.000");
    assert_eq!(tr["accountId"], checking.as_str());
    assert_eq!(tr["counterAccountId"], savings.as_str());
    assert_eq!(tr["counterAccountName"], "Ahorro");

    let from_savings = env.row(json!({ "view": "combined", "accountId": savings }), &t);
    assert_eq!(from_savings["accountId"], savings.as_str());
    assert_eq!(from_savings["counterAccountId"], checking.as_str());
    assert_eq!(from_savings["amount"]["minor"], "100000");
    assert_eq!(from_savings["amount"]["display"], "$100.000");

    let opening = env.texts(&format!(
        "SELECT DISTINCT e.id FROM journal_entry e JOIN posting p ON p.entry_id = e.id \
         WHERE e.kind = 'opening_balance' AND p.account_id = '{checking}'"
    ));
    let or = env.row(all, &opening[0]);
    assert_eq!(or["kind"], "opening_balance");
    assert_eq!(or["amount"]["minor"], "500000");
    assert_eq!(or["amount"]["display"], "+$500.000");
}

#[test]
fn a05_list_pages_250_entries_by_date_desc_id_desc_without_gaps_or_duplicates() {
    let mut env = Env::new("a05");
    let checking = env.account("Cuenta corriente", None, false);
    let mut expected: Vec<(String, String)> = (0..250)
        .map(|n| {
            let date = format!("2026-10-{:02}", 1 + n % 10);
            let id = env.create_id(expense(&checking, "1.000", &date));
            (date, id)
        })
        .collect();
    expected.sort();
    expected.reverse();

    let mut seen: Vec<(String, String)> = Vec::new();
    let mut sizes = Vec::new();
    let mut cursor = Value::Null;
    loop {
        let mut q = json!({ "view": "combined", "limit": 100 });
        if !cursor.is_null() {
            q["cursor"] = cursor.clone();
        }
        let page = env.list(q);
        let items = page["items"].as_array().unwrap();
        sizes.push(items.len());
        seen.extend(items.iter().map(|r| {
            (
                r["date"].as_str().unwrap().to_owned(),
                r["id"].as_str().unwrap().to_owned(),
            )
        }));
        cursor = page["nextCursor"].clone();
        if cursor.is_null() {
            break;
        }
        assert!(sizes.len() < 5, "paging does not end");
    }
    assert_eq!(sizes, vec![100, 100, 50]);
    assert_eq!(seen, expected);

    let default = env.list(json!({ "view": "combined" }));
    assert_eq!(default["items"].as_array().unwrap().len(), 100);
    assert!(default["nextCursor"].is_string());
}

#[test]
fn a06_list_filters_by_account_date_range_and_view_and_hides_deleted() {
    let mut env = Env::new("a06");
    let a = env.account("A", None, false);
    let b = env.account("B", None, false);
    let p = env.account("P", None, true);
    let e1 = env.create_id(expense(&a, "1.000", "2026-11-01"));
    let e2 = env.create_id(expense(&b, "2.000", "2026-11-02"));
    let t = env.create_id(transfer(&a, &b, "3.000", "2026-11-03"));
    let e4 = env.create_id(expense(&p, "4.000", "2026-11-04"));
    let gone = env.create_id(expense(&a, "5.000", "2026-11-05"));
    env.ok("tx_delete", json!({ "id": gone }));
    let set = |ids: &[&String]| ids.iter().map(|s| (*s).clone()).collect::<BTreeSet<_>>();

    assert_eq!(
        env.list_ids(json!({ "view": "combined", "accountId": a })),
        set(&[&e1, &t])
    );
    assert_eq!(
        env.list_ids(json!({ "view": "combined", "from": "2026-11-02", "to": "2026-11-03" })),
        set(&[&e2, &t])
    );
    assert_eq!(env.list_ids(json!({ "view": "personal" })), set(&[&e4]));
    assert_eq!(
        env.list_ids(json!({ "view": "household" })),
        set(&[&e1, &e2, &t])
    );
    assert_eq!(
        env.list_ids(json!({ "view": "combined" })),
        set(&[&e1, &e2, &t, &e4])
    );
}

#[test]
fn a07_update_amount_keeps_posting_ids_logs_only_amounts_and_a_resend_logs_nothing() {
    let mut env = Env::new("a07");
    let checking = env.account("Cuenta corriente", None, false);
    let id = env.create_id(expense(&checking, "45.990", "2026-11-21"));
    let ids = env.posting_ids(&id);
    let before = env.log().len();
    let edited = expense(&checking, "50.000", "2026-11-21");
    let out = env.ok("tx_update", json!({ "id": id, "input": edited }));
    assert_eq!(out["amount"]["minor"], "50000");
    assert_eq!(env.posting_ids(&id), ids);
    assert_eq!(env.balance(&checking), "-50000");
    let new_rows: Vec<_> = env.log().into_iter().skip(before).collect();
    assert_eq!(new_rows.len(), 2, "{new_rows:?}");
    for (entity, _, op, fields) in &new_rows {
        assert_eq!((entity.as_str(), op.as_str()), ("posting", "update"));
        assert_eq!(keys(fields), BTreeSet::from(["amount_minor".to_owned()]));
    }
    let after = env.log().len();
    env.ok("tx_update", json!({ "id": id, "input": edited }));
    assert_eq!(env.log().len(), after);
}

#[test]
fn a08_type_change_converts_the_counter_leg_in_place_and_sets_or_clears_its_status() {
    let mut env = Env::new("a08");
    let a = env.account("A", None, false);
    let b = env.account("B", None, false);
    let id = env.create_id(expense(&a, "10.000", "2026-11-21"));
    let ids = env.posting_ids(&id);

    let t = env.ok(
        "tx_update",
        json!({ "id": id, "input": transfer(&a, &b, "10.000", "2026-11-21") }),
    );
    assert_eq!(t["kind"], "transfer");
    assert_eq!(env.posting_ids(&id), ids);
    let legs = env.legs(&id);
    assert!(
        legs.iter().all(|l| l.0 == "account" && l.2.is_none()),
        "{legs:?}"
    );
    assert!(
        legs.iter().any(|l| l.1.as_deref() == Some(b.as_str())
            && l.3 == 10000
            && l.4.as_deref() == Some("cleared")),
        "{legs:?}"
    );
    assert_eq!(env.balance(&b), "10000");

    let e = env.ok(
        "tx_update",
        json!({ "id": id, "input": expense(&a, "10.000", "2026-11-21") }),
    );
    assert_eq!(e["kind"], "expense");
    assert_eq!(env.posting_ids(&id), ids);
    let legs = env.legs(&id);
    assert!(
        legs.iter().any(|l| l.0 == "category"
            && l.1.is_none()
            && l.2.as_deref() == Some(food().as_str())
            && l.4.is_none()),
        "{legs:?}"
    );
    assert_eq!(env.balance(&b), "0");
}

#[test]
fn a09_header_edit_updates_date_description_and_memo_in_place() {
    let mut env = Env::new("a09");
    let checking = env.account("Cuenta corriente", None, false);
    let id = env.create_id(expense(&checking, "45.990", "2026-11-21"));
    let before = env.log().len();
    let mut input = expense(&checking, "45.990", "2026-11-22");
    input["description"] = json!("Feria");
    input["memo"] = json!("con la familia");
    let out = env.ok("tx_update", json!({ "id": id, "input": input }));
    assert_eq!(out["date"], "2026-11-22");
    assert_eq!(out["description"], "Feria");
    assert_eq!(out["memo"], "con la familia");
    let new_rows: Vec<_> = env.log().into_iter().skip(before).collect();
    assert_eq!(new_rows.len(), 1, "{new_rows:?}");
    let (entity, eid, op, fields) = &new_rows[0];
    assert_eq!(
        (entity.as_str(), eid.as_str(), op.as_str()),
        ("journal_entry", id.as_str(), "update")
    );
    assert_eq!(
        keys(fields),
        ["description", "entry_date", "memo"]
            .map(String::from)
            .into()
    );
    assert_eq!(env.get(&id), out);
}

#[test]
fn a10_delete_soft_deletes_entry_and_postings_restore_brings_them_back_and_repeats_are_no_ops() {
    let mut env = Env::new("a10");
    let checking = env.account("Cuenta corriente", None, false);
    let id = env.create_id(expense(&checking, "45.990", "2026-11-21"));
    let ids = env.posting_ids(&id);

    let before = env.log().len();
    assert_eq!(env.ok("tx_delete", json!({ "id": id })), Value::Null);
    assert_eq!(env.balance(&checking), "0");
    assert!(!env.list_ids(json!({ "view": "combined" })).contains(&id));
    assert_eq!(env.get(&id)["deleted"], true);
    assert_eq!(
        env.int(&format!(
            "SELECT count(*) FROM posting WHERE entry_id = '{id}' AND deleted = 1"
        )),
        2
    );
    let rows: Vec<_> = env.log().into_iter().skip(before).collect();
    assert_eq!(rows.len(), 3, "{rows:?}");
    assert!(
        rows.iter()
            .all(|r| keys(&r.3) == BTreeSet::from(["deleted".to_owned()]))
    );
    let after_delete = env.log().len();
    env.ok("tx_delete", json!({ "id": id }));
    assert_eq!(env.log().len(), after_delete);

    assert_eq!(env.ok("tx_restore", json!({ "id": id })), Value::Null);
    assert_eq!(env.balance(&checking), "-45990");
    assert!(env.list_ids(json!({ "view": "combined" })).contains(&id));
    assert_eq!(env.get(&id)["deleted"], false);
    assert_eq!(env.posting_ids(&id), ids);
    assert_eq!(env.log().len(), after_delete + 3);
    env.ok("tx_restore", json!({ "id": id }));
    assert_eq!(env.log().len(), after_delete + 3);
}

#[test]
fn a11_edge_archived_account_wrong_currency_and_unknown_ids_are_refused_and_write_nothing() {
    let mut env = Env::new("a11");
    let checking = env.account("Cuenta corriente", None, false);
    let old = env.account("Vieja", None, false);
    env.ok("account_archive", json!({ "id": old }));
    let before = env.state();

    let e = env.err(
        "tx_create",
        json!({ "input": expense(&old, "1.000", "2026-11-21") }),
    );
    assert_eq!(e.code, ErrorCode::Validation, "{e}");
    let e = env.err(
        "tx_create",
        json!({ "input": transfer(&checking, &old, "1.000", "2026-11-21") }),
    );
    assert_eq!(e.code, ErrorCode::Validation, "{e}");

    let mut usd = expense(&checking, "10,00", "2026-11-21");
    usd["amount"] = json!({ "amount": "10,00", "currency": "USD" });
    assert_eq!(
        env.err("tx_create", json!({ "input": usd })).code,
        ErrorCode::CurrencyMismatch
    );

    let e = env.err(
        "tx_create",
        json!({ "input": expense(uuid(), "1.000", "2026-11-21") }),
    );
    assert_eq!(e.code, ErrorCode::NotFound, "{e}");
    let mut unknown_cat = expense(&checking, "1.000", "2026-11-21");
    unknown_cat["categoryId"] = json!(uuid());
    let e = env.err("tx_create", json!({ "input": unknown_cat }));
    assert_eq!(e.code, ErrorCode::NotFound, "{e}");
    let e = env.err(
        "tx_create",
        json!({ "input": transfer(&checking, uuid(), "1.000", "2026-11-21") }),
    );
    assert_eq!(e.code, ErrorCode::NotFound, "{e}");

    assert_eq!(env.state(), before);
}

#[test]
fn a12_edge_a_reconciled_leg_locks_update_delete_and_restore_with_conflict() {
    let mut env = Env::new("a12");
    let checking = env.account("Cuenta corriente", None, false);
    let id = env.create_id(expense(&checking, "45.990", "2026-11-21"));
    env.reconcile(&id, &checking);
    let before = env.state();

    let mut header_only = expense(&checking, "45.990", "2026-11-21");
    header_only["description"] = json!("otra cosa");
    for input in [header_only, expense(&checking, "1.000", "2026-11-21")] {
        let e = env.err("tx_update", json!({ "id": id, "input": input }));
        assert_eq!(e.code, ErrorCode::Conflict, "{e}");
    }
    assert_eq!(
        env.err("tx_delete", json!({ "id": id })).code,
        ErrorCode::Conflict
    );
    assert_eq!(env.state(), before);

    let gone = env.create_id(expense(&checking, "1.000", "2026-11-21"));
    env.ok("tx_delete", json!({ "id": gone }));
    env.reconcile(&gone, &checking);
    let before = env.state();
    assert_eq!(
        env.err("tx_restore", json!({ "id": gone })).code,
        ErrorCode::Conflict
    );
    assert_eq!(env.state(), before);
}

#[test]
fn a14_edge_bad_input_carries_field_reasons_and_writes_nothing() {
    let mut env = Env::new("a14");
    let checking = env.account("Cuenta corriente", None, false);
    let before = env.state();
    let base = expense(&checking, "1.000", "2026-11-21");
    let with = |k: &str, v: Value| {
        let mut x = base.clone();
        x[k] = v;
        x
    };
    let cases = [
        (
            with("amount", clp("doce lucas")),
            ErrorCode::Validation,
            "amount",
            "errors.field.invalidAmount",
        ),
        (
            with("amount", clp("0")),
            ErrorCode::Validation,
            "amount",
            "errors.field.invalidAmount",
        ),
        (
            with("amount", clp("-1.000")),
            ErrorCode::Validation,
            "amount",
            "errors.field.invalidAmount",
        ),
        (
            with("amount", json!({ "amount": "1", "currency": "XXX" })),
            ErrorCode::Validation,
            "amount",
            "errors.field.unsupportedCurrency",
        ),
        (
            with("date", json!("2026-02-30")),
            ErrorCode::Validation,
            "date",
            "errors.field.invalidDate",
        ),
        (
            with("date", json!("21-11-2026")),
            ErrorCode::Validation,
            "date",
            "errors.field.invalidDate",
        ),
        (
            with("categoryId", json!(salary())),
            ErrorCode::Validation,
            "categoryId",
            "errors.field.wrongKind",
        ),
        (
            transfer(&checking, &checking, "1.000", "2026-11-21"),
            ErrorCode::Validation,
            "toAccountId",
            "errors.field.sameAccount",
        ),
    ];
    for (input, code, field, key) in cases {
        let e = env.err("tx_create", json!({ "input": input }));
        field_error(&e, code, field, key);
    }
    assert_eq!(env.state(), before);
    env.exec(&format!(
        "UPDATE category SET is_archived = 1 WHERE id = '{}'",
        seed_id("expense.food.delivery")
    ));
    let before = env.state();
    let e = env.err(
        "tx_create",
        json!({ "input": with("categoryId", json!(seed_id("expense.food.delivery"))) }),
    );
    field_error(
        &e,
        ErrorCode::Validation,
        "categoryId",
        "errors.field.archived",
    );
    assert_eq!(env.state(), before);
}

#[test]
fn a15_edge_update_validates_header_on_a_no_op_refuses_opening_balances_and_unknown_ids() {
    let mut env = Env::new("a15");
    let checking = env.account("Cuenta corriente", Some("500.000"), false);
    let id = env.create_id(expense(&checking, "1.000", "2026-11-21"));
    let before = env.state();

    let mut bad_date = expense(&checking, "1.000", "2026-11-21");
    bad_date["date"] = json!("2026-02-30");
    let e = env.err("tx_update", json!({ "id": id, "input": bad_date }));
    field_error(
        &e,
        ErrorCode::Validation,
        "date",
        "errors.field.invalidDate",
    );
    let mut long = expense(&checking, "1.000", "2026-11-21");
    long["description"] = json!("x".repeat(501));
    let e = env.err("tx_update", json!({ "id": id, "input": long }));
    field_error(
        &e,
        ErrorCode::Validation,
        "description",
        "errors.field.tooLong",
    );

    let opening = env.texts(&format!(
        "SELECT DISTINCT e.id FROM journal_entry e JOIN posting p ON p.entry_id = e.id \
         WHERE e.kind = 'opening_balance' AND p.account_id = '{checking}'"
    ))[0]
        .clone();
    for (name, args) in [
        (
            "tx_update",
            json!({ "id": opening, "input": expense(&checking, "1.000", "2026-11-21") }),
        ),
        ("tx_delete", json!({ "id": opening })),
        ("tx_restore", json!({ "id": opening })),
    ] {
        assert_eq!(env.err(name, args).code, ErrorCode::Validation, "{name}");
    }
    for (name, args) in [
        (
            "tx_update",
            json!({ "id": uuid(), "input": expense(&checking, "1.000", "2026-11-21") }),
        ),
        ("tx_delete", json!({ "id": uuid() })),
        ("tx_restore", json!({ "id": uuid() })),
        ("tx_get", json!({ "id": uuid() })),
    ] {
        assert_eq!(env.err(name, args).code, ErrorCode::NotFound, "{name}");
    }
    assert_eq!(env.state(), before);

    env.ok("tx_delete", json!({ "id": id }));
    let before = env.state();
    let e = env.err(
        "tx_update",
        json!({ "id": id, "input": expense(&checking, "2.000", "2026-11-21") }),
    );
    assert_eq!(e.code, ErrorCode::NotFound, "{e}");
    assert_eq!(env.state(), before);
}

#[test]
fn a16_edge_list_refuses_bad_limit_cursor_dates_and_unknown_account() {
    let mut env = Env::new("a16");
    env.account("Cuenta corriente", None, false);
    let cases = [
        (
            json!({ "view": "combined", "limit": 0 }),
            "limit",
            "errors.field.outOfRange",
        ),
        (
            json!({ "view": "combined", "limit": 501 }),
            "limit",
            "errors.field.outOfRange",
        ),
        (
            json!({ "view": "combined", "cursor": "not-a-cursor" }),
            "cursor",
            "errors.field.invalid",
        ),
        (
            json!({ "view": "combined", "from": "2026-13-01" }),
            "from",
            "errors.field.invalidDate",
        ),
        (
            json!({ "view": "combined", "to": "ayer" }),
            "to",
            "errors.field.invalidDate",
        ),
    ];
    for (query, field, key) in cases {
        let e = env.err("tx_list", json!({ "query": query }));
        field_error(&e, ErrorCode::Validation, field, key);
    }
    // The edges themselves are accepted.
    for limit in [1, 500] {
        env.list(json!({ "view": "combined", "limit": limit }));
    }
    let e = env.err(
        "tx_list",
        json!({ "query": { "view": "combined", "accountId": uuid() } }),
    );
    assert_eq!(e.code, ErrorCode::NotFound, "{e}");
}

#[test]
fn a17_edge_restore_onto_an_account_archived_since_is_refused_and_writes_nothing() {
    let mut env = Env::new("a17");
    let checking = env.account("Cuenta corriente", None, false);
    let id = env.create_id(expense(&checking, "1.000", "2026-11-21"));
    env.ok("tx_delete", json!({ "id": id }));
    env.ok("account_archive", json!({ "id": checking }));
    let before = env.state();
    let e = env.err("tx_restore", json!({ "id": id }));
    assert_eq!(e.code, ErrorCode::Validation, "{e}");
    assert_eq!(env.state(), before);
}

#[test]
fn a18_abuse_non_uuid_ids_oversized_text_and_overflow_are_refused_and_sql_is_stored_verbatim() {
    let mut env = Env::new("a18");
    let checking = env.account("Cuenta corriente", None, false);
    let before = env.state();
    for name in ["tx_get", "tx_delete", "tx_restore"] {
        let e = env.err(name, json!({ "id": "1' OR '1'='1" }));
        assert_eq!(e.code, ErrorCode::Validation, "{name}");
    }
    let e = env.err(
        "tx_create",
        json!({ "input": expense("x", "1.000", "2026-11-21") }),
    );
    assert_eq!(e.code, ErrorCode::Validation, "{e}");
    let mut long = expense(&checking, "1.000", "2026-11-21");
    long["description"] = json!("x".repeat(501));
    field_error(
        &env.err("tx_create", json!({ "input": long })),
        ErrorCode::Validation,
        "description",
        "errors.field.tooLong",
    );
    let mut memo = expense(&checking, "1.000", "2026-11-21");
    memo["memo"] = json!("m".repeat(2001));
    field_error(
        &env.err("tx_create", json!({ "input": memo })),
        ErrorCode::Validation,
        "memo",
        "errors.field.tooLong",
    );
    field_error(
        &env.err(
            "tx_create",
            json!({ "input": expense(&checking, "99.999.999.999.999.999.999", "2026-11-21") }),
        ),
        ErrorCode::Validation,
        "amount",
        "errors.field.invalidAmount",
    );
    assert_eq!(env.state(), before);

    // At the limits, text is accepted and stored exactly as sent.
    let sql = "'); DROP TABLE posting; --";
    let mut input = expense(&checking, "1.000", "2026-11-21");
    input["description"] = json!(sql);
    input["memo"] = json!("m".repeat(2000));
    let id = env.create_id(input);
    assert_eq!(env.get(&id)["description"], sql);
    let mut edge = expense(&checking, "1.000", "2026-11-21");
    edge["description"] = json!("x".repeat(500));
    env.create(edge);
}

/// Captures every event at `info` and above while `f` runs on this thread.
fn info_log(f: impl FnOnce()) -> String {
    #[derive(Clone, Default)]
    struct Buf(Arc<Mutex<Vec<u8>>>);
    impl std::io::Write for Buf {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let buf = Buf::default();
    let writer = buf.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_ansi(false)
        .with_writer(move || writer.clone())
        .finish();
    tracing::subscriber::with_default(subscriber, f);
    let bytes = buf.0.lock().unwrap().clone();
    String::from_utf8(bytes).unwrap()
}

#[test]
fn a19_info_logs_carry_one_span_per_command_and_no_amounts_descriptions_or_memos() {
    let mut env = Env::new("a19");
    let a = env.account("Cuenta corriente", None, false);
    let b = env.account("Ahorro", None, false);
    let out = info_log(|| {
        let mut input = expense(&a, "45.990", "2026-11-21");
        input["description"] = json!("DESCRIPCION-SECRETA");
        input["memo"] = json!("MEMO-SECRETO");
        let id = env.create_id(input);
        env.get(&id);
        env.list(json!({ "view": "combined" }));
        env.ok(
            "tx_update",
            json!({ "id": id, "input": transfer(&a, &b, "45.990", "2026-11-21") }),
        );
        env.ok("tx_delete", json!({ "id": id }));
        env.ok("tx_restore", json!({ "id": id }));
        // A refusal whose DomainError names the account must not reach info either.
        env.err(
            "tx_create",
            json!({ "input": expense(uuid(), "77.777", "2026-11-21") }),
        );
    });
    for name in [
        "tx_create",
        "tx_get",
        "tx_list",
        "tx_update",
        "tx_delete",
        "tx_restore",
    ] {
        assert!(
            out.lines()
                .any(|l| l.contains(&format!("command=\"{name}\""))
                    || l.contains(&format!("command={name}"))),
            "no span for {name} in:\n{out}"
        );
    }
    for secret in [
        "45990",
        "45.990",
        "77777",
        "77.777",
        "DESCRIPCION-SECRETA",
        "MEMO-SECRETO",
        uuid(),
        "unknown account",
    ] {
        assert!(!out.contains(secret), "{secret:?} logged at info:\n{out}");
    }
    assert!(out.contains("NOT_FOUND"), "result code not logged:\n{out}");
}

#[test]
fn a20_catalogue_every_tx_command_is_implemented_and_still_dispatchable() {
    let mut env = Env::new("a20");
    let checking = env.account("Cuenta corriente", None, false);
    let input = expense(&checking, "1.000", "2026-11-21");
    let id = env.create_id(input.clone());
    let calls = [
        ("tx_get", json!({ "id": id })),
        ("tx_list", json!({ "query": { "view": "combined" } })),
        ("tx_update", json!({ "id": id, "input": input })),
        ("tx_delete", json!({ "id": id })),
        ("tx_restore", json!({ "id": id })),
    ];
    for (name, args) in calls {
        let r = env.call(name, args);
        assert!(r.is_ok(), "{name}: {r:?}");
    }
    let tx: Vec<&str> = COMMANDS
        .iter()
        .copied()
        .filter(|c| c.starts_with("tx_"))
        .collect();
    assert_eq!(
        tx,
        [
            "tx_create",
            "tx_get",
            "tx_list",
            "tx_update",
            "tx_delete",
            "tx_restore"
        ]
    );
}

// ───────────────────────────── journey ─────────────────────────────

#[test]
fn journey_record_expenses_and_a_transfer_edit_delete_and_undo_and_see_the_balances() {
    let mut env = Env::new("journey");
    let checking = env.account("Cuenta corriente", Some("1.000.000"), false);
    let savings = env.account("Ahorro", None, false);
    let groceries = env.create_id(expense(&checking, "45.990", "2026-11-20"));
    env.create_id(expense(&checking, "12.000", "2026-11-21"));
    let saved = env.create_id(transfer(&checking, &savings, "100.000", "2026-11-21"));
    assert_eq!(env.balance(&checking), "842010");
    assert_eq!(env.balance(&savings), "100000");

    let rows = env.list(json!({ "view": "combined", "accountId": checking }));
    let kinds: Vec<&str> = rows["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["kind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, ["transfer", "expense", "expense", "opening_balance"]);
    let expense_total: i64 = rows["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["kind"] == "expense")
        .map(|r| {
            r["amount"]["minor"]
                .as_str()
                .unwrap()
                .parse::<i64>()
                .unwrap()
        })
        .sum();
    assert_eq!(
        expense_total, -57990,
        "the transfer is not counted as an expense"
    );

    env.ok(
        "tx_update",
        json!({ "id": groceries, "input": expense(&checking, "40.000", "2026-11-20") }),
    );
    assert_eq!(env.balance(&checking), "848000");
    env.ok("tx_delete", json!({ "id": saved }));
    assert_eq!(env.balance(&checking), "948000");
    assert_eq!(env.balance(&savings), "0");
    env.ok("tx_restore", json!({ "id": saved }));
    assert_eq!(env.balance(&checking), "848000");
    assert_eq!(env.balance(&savings), "100000");
}
