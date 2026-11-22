//! Acceptance tests for slice S4 (household, accounts, category_tree), REQ-1, REQ-2, REQ-13.
//! Every test acts through `commands::dispatch` on an app opened on a temp file; what was
//! persisted (change log, device link, legs) is read back through a second connection.
//! One test per brief criterion, named after it (`.workflow/brief.md`, Acceptance tests).

use std::collections::BTreeMap;
use std::path::PathBuf;

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
        let dir = std::env::temp_dir().join(format!("mrk-s4-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("mrkrabs.sqlite");
        let app = App::open(&path).unwrap();
        Self { dir, path, app }
    }

    fn call(&mut self, name: &str, args: Value) -> Result<Value, ApiError> {
        dispatch(&mut self.app, name, args)
    }
    fn ok(&mut self, name: &str, args: Value) -> Value {
        self.call(name, args.clone())
            .unwrap_or_else(|e| panic!("{name} {args} failed: {e}"))
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
    fn opt_text(&self, sql: &str) -> Option<String> {
        self.raw()
            .connection()
            .query_row(sql, [], |r| r.get(0))
            .unwrap()
    }

    /// `(entity, entity_id, op, visibility, fields_json)` in log order.
    fn log(&self) -> Vec<(String, String, String, String, Value)> {
        let raw = self.raw();
        let mut st = raw
            .connection()
            .prepare(
                "SELECT entity, entity_id, op, visibility, fields_json FROM change_log ORDER BY seq",
            )
            .unwrap();
        st.query_map([], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                serde_json::from_str(&r.get::<_, String>(4)?).unwrap(),
            ))
        })
        .unwrap()
        .map(Result::unwrap)
        .collect()
    }
    fn log_len(&self) -> usize {
        self.log().len()
    }

    /// Everything a command could have written, to prove a refused one wrote nothing.
    fn state(&self) -> String {
        let mut out = Vec::new();
        for t in [
            "household",
            "member",
            "account",
            "journal_entry",
            "posting",
            "category",
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
        out.push(format!(
            "device:{:?}",
            self.opt_text("SELECT member_id FROM device WHERE is_local = 1")
        ));
        out.join("|")
    }

    fn onboard(&mut self) -> Value {
        self.ok(
            "household_create",
            json!({ "input": { "name": "Casa", "myName": "Yo", "baseCurrency": "CLP" } }),
        )
    }
    fn create_account(&mut self, input: Value) -> Value {
        self.ok("account_create", json!({ "input": input }))
    }
    fn list(&mut self, view: &str, include_archived: bool) -> Vec<Value> {
        let v = self.ok(
            "account_list",
            json!({ "query": { "view": view, "includeArchived": include_archived } }),
        );
        v.as_array().unwrap().clone()
    }
    fn summary(&mut self, id: &str) -> Option<Value> {
        self.list("combined", true)
            .into_iter()
            .find(|s| s["account"]["id"] == id)
    }
    fn opening_entry(&self, account: &str) -> String {
        self.texts(&format!(
            "SELECT DISTINCT e.id FROM journal_entry e JOIN posting p ON p.entry_id = e.id \
             WHERE e.kind = 'opening_balance' AND p.account_id = '{account}'"
        ))
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("no opening-balance entry for {account}"))
    }
    fn posting_ids(&self, entry: &str) -> Vec<String> {
        self.texts(&format!(
            "SELECT id FROM posting WHERE entry_id = '{entry}' ORDER BY id"
        ))
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

fn checking(opening: Option<(&str, &str)>) -> Value {
    let mut v = json!({ "name": "Cuenta corriente", "kind": "checking", "currency": "CLP" });
    if let Some((amount, date)) = opening {
        v["openingBalance"] = json!({ "amount": clp(amount), "date": date });
    }
    v
}

fn fields(e: &ApiError) -> BTreeMap<String, String> {
    e.fields.clone().unwrap_or_default()
}

fn field_error(e: &ApiError, code: ErrorCode, field: &str, key: &str) {
    assert_eq!(e.code, code, "{e}");
    assert_eq!(
        fields(e).get(field).map(String::as_str),
        Some(key),
        "fields of {e}: {:?}",
        e.fields
    );
}

/// All nodes of a `CategoryNodeDto[]` tree, depth first.
fn flatten(nodes: &Value) -> Vec<Value> {
    let mut out = Vec::new();
    for n in nodes.as_array().unwrap() {
        out.push(n.clone());
        out.extend(flatten(&n["children"]));
    }
    out
}

fn node<'a>(nodes: &'a [Value], id: &str) -> Option<&'a Value> {
    nodes.iter().find(|n| n["id"] == id)
}

fn uuid() -> &'static str {
    "0190f0f0-0000-7000-8000-000000000000"
}

// ───────────────────────────── criteria ─────────────────────────────

#[test]
fn c01_tracer_household_create_then_bootstrap_returns_household_me_and_settings() {
    let mut env = Env::new("c01");
    let hh = env.onboard();
    assert_eq!(hh["name"], "Casa");
    assert_eq!(hh["baseCurrency"], "CLP");
    assert_eq!(hh["locale"], "es-CL");

    let b = env.ok("app_bootstrap", json!({}));
    assert_eq!(b["household"], hh);
    let members = b["members"].as_array().unwrap();
    assert_eq!(members.len(), 1);
    assert_eq!(members[0]["displayName"], "Yo");
    assert_eq!(b["me"], members[0]);
    assert_eq!(
        b["settings"],
        json!({ "baseCurrency": "CLP", "locale": "es-CL", "budgetMode": "per_installment" })
    );
    let health = env.ok("app_health", json!({}));
    assert_eq!(b["schemaVersion"], health["latestSchemaVersion"]);
    assert_eq!(
        env.opt_text("SELECT member_id FROM device WHERE is_local = 1"),
        Some(members[0]["id"].as_str().unwrap().to_owned())
    );
}

#[test]
fn c02_household_create_logs_household_and_member_only_and_seeds_unlogged_defaults() {
    let mut env = Env::new("c02");
    let hh = env.onboard();
    let me = env.ok("app_bootstrap", json!({}))["me"]["id"].clone();
    let log = env.log();
    let rows: Vec<_> = log
        .iter()
        .map(|(e, id, op, vis, _)| (e.as_str(), id.as_str(), op.as_str(), vis.as_str()))
        .collect();
    assert_eq!(
        rows,
        vec![
            (
                "household",
                hh["id"].as_str().unwrap(),
                "upsert",
                "household"
            ),
            ("member", me.as_str().unwrap(), "upsert", "household"),
        ]
    );
    assert_eq!(
        env.int("SELECT count(*) FROM category WHERE deleted = 0 AND hlc_device = 'seed'"),
        33
    );
    assert_eq!(
        env.int("SELECT count(*) FROM account WHERE deleted = 0 AND hlc_device = 'seed'"),
        3
    );
}

#[test]
fn c03_second_household_create_is_conflict_and_writes_nothing() {
    let mut env = Env::new("c03");
    env.onboard();
    let before = env.state();
    let e = env.err(
        "household_create",
        json!({ "input": { "name": "Otra", "myName": "Otro", "baseCurrency": "USD" } }),
    );
    assert_eq!(e.code, ErrorCode::Conflict, "{e}");
    assert_eq!(env.state(), before);
}

#[test]
fn c04_before_onboarding_bootstrap_is_empty_and_settings_and_accounts_are_not_found() {
    let mut env = Env::new("c04");
    let b = env.ok("app_bootstrap", json!({}));
    assert_eq!(b["household"], Value::Null);
    assert_eq!(b["members"], json!([]));
    assert_eq!(b["me"], Value::Null);
    assert_eq!(b["settings"], Value::Null);
    assert!(b["schemaVersion"].as_u64().unwrap() >= 3);
    assert_eq!(env.err("settings_get", json!({})).code, ErrorCode::NotFound);
    assert_eq!(
        env.err("account_create", json!({ "input": checking(None) }))
            .code,
        ErrorCode::NotFound
    );
    // Contrast: after onboarding both work.
    env.onboard();
    assert_eq!(
        env.ok("settings_get", json!({})),
        json!({ "baseCurrency": "CLP", "locale": "es-CL", "budgetMode": "per_installment" })
    );
    env.create_account(checking(None));
}

#[test]
fn c05_account_create_with_opening_balance_shows_it_in_list_and_get() {
    let mut env = Env::new("c05");
    env.onboard();
    let a = env.create_account(checking(Some(("500.000", "2026-11-01"))));
    assert_eq!(a["scope"], "household");
    assert_eq!(a["visibility"], "shared_full");
    assert_eq!(a["ownerMemberId"], Value::Null);
    assert_eq!(a["isArchived"], false);
    let id = a["id"].as_str().unwrap().to_owned();

    let s = env.summary(&id).expect("listed");
    assert_eq!(s["account"], a);
    assert_eq!(
        s["balance"],
        json!({ "minor": "500000", "currency": "CLP", "display": "$500.000" })
    );
    let d = env.ok("account_get", json!({ "id": id }));
    assert_eq!(d["account"], a);
    assert_eq!(d["balance"], s["balance"]);
    assert_eq!(
        d["openingBalance"],
        json!({ "amount": { "minor": "500000", "currency": "CLP", "display": "$500.000" }, "date": "2026-11-01" })
    );
}

#[test]
fn c06_opening_balance_is_one_entry_with_account_and_fx_legs_and_four_log_rows() {
    let mut env = Env::new("c06");
    env.onboard();
    let logged = env.log_len();
    let a = env.create_account(checking(Some(("500.000", "2026-11-01"))));
    let id = a["id"].as_str().unwrap().to_owned();
    let entry = env.opening_entry(&id);
    assert_eq!(
        env.texts(&format!(
            "SELECT entry_date || '|' || kind || '|' || deleted FROM journal_entry WHERE id = '{entry}'"
        )),
        ["2026-11-01|opening_balance|0"]
    );
    assert_eq!(
        env.texts(&format!(
            "SELECT leg || '|' || ifnull(account_id, '-') || '|' || amount_minor || '|' || currency_code \
             || '|' || ifnull(status, '-') FROM posting WHERE entry_id = '{entry}' ORDER BY leg"
        )),
        [
            format!("account|{id}|500000|CLP|cleared"),
            "fx|-|-500000|CLP|-".to_owned()
        ]
    );
    let mut new: Vec<String> = env.log()[logged..]
        .iter()
        .map(|(e, _, op, _, _)| format!("{e}:{op}"))
        .collect();
    new.sort();
    assert_eq!(
        new,
        [
            "account:upsert",
            "journal_entry:upsert",
            "posting:upsert",
            "posting:upsert"
        ]
    );
}

#[test]
fn c07_personal_account_defaults_to_me_shared_summary_and_private_rows_log_private() {
    let mut env = Env::new("c07");
    env.onboard();
    let me = env.ok("app_bootstrap", json!({}))["me"]["id"].clone();
    let mut input = checking(None);
    input["scope"] = json!("personal");
    let a = env.create_account(input);
    assert_eq!(a["scope"], "personal");
    assert_eq!(a["ownerMemberId"], me);
    assert_eq!(a["visibility"], "shared_summary");

    let logged = env.log_len();
    let mut private = checking(Some(("10.000", "2026-11-01")));
    private["scope"] = json!("personal");
    private["visibility"] = json!("private");
    let p = env.create_account(private);
    assert_eq!(p["visibility"], "private");
    let new = &env.log()[logged..];
    assert_eq!(new.len(), 4);
    assert!(
        new.iter().all(|(_, _, _, vis, _)| vis == "private"),
        "{new:?}"
    );
}

#[test]
fn c08_archive_hides_the_account_unless_include_archived_and_rearchiving_logs_nothing() {
    let mut env = Env::new("c08");
    env.onboard();
    let id = env.create_account(checking(None))["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let archived = env.ok("account_archive", json!({ "id": id }));
    assert_eq!(archived["isArchived"], true);
    assert!(
        !env.list("combined", false)
            .iter()
            .any(|s| s["account"]["id"] == id)
    );
    let shown = env.summary(&id).expect("included when includeArchived");
    assert_eq!(shown["account"]["isArchived"], true);
    let logged = env.log_len();
    let again = env.ok("account_archive", json!({ "id": id }));
    assert_eq!(again["isArchived"], true);
    assert_eq!(env.log_len(), logged);
}

#[test]
fn c09_account_list_filters_by_view_shows_zero_seed_balances_and_is_ordered() {
    let mut env = Env::new("c09");
    env.onboard();
    let household = env.create_account(checking(None))["id"].clone();
    let mut personal = checking(None);
    personal["name"] = json!("Mi cuenta");
    personal["scope"] = json!("personal");
    let personal = env.create_account(personal)["id"].clone();

    let ids = |rows: &[Value]| {
        rows.iter()
            .map(|s| s["account"]["id"].clone())
            .collect::<Vec<_>>()
    };
    let combined = env.list("combined", false);
    let p = env.list("personal", false);
    let h = env.list("household", false);
    assert_eq!(ids(&p), vec![personal.clone()]);
    assert!(ids(&h).contains(&household) && !ids(&h).contains(&personal));
    assert_eq!(combined.len(), 2 + 3, "two created + three seeded");
    assert_eq!(h.len(), 1 + 3);

    let savings = seed_id("account.savings");
    let seeded = combined
        .iter()
        .find(|s| s["account"]["id"] == savings.as_str())
        .expect("seeded savings account listed");
    assert_eq!(
        seeded["balance"],
        json!({ "minor": "0", "currency": "CLP", "display": "$0" })
    );
    let keys: Vec<(i64, String, String)> = combined
        .iter()
        .map(|s| {
            let a = &s["account"];
            (
                a["sortOrder"].as_i64().unwrap(),
                a["name"].as_str().unwrap().to_owned(),
                a["id"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    let mut sorted = keys.clone();
    sorted.sort();
    assert_eq!(keys, sorted);
}

#[test]
fn c10_update_logs_only_changed_fields_and_an_unchanged_resend_logs_nothing() {
    let mut env = Env::new("c10");
    env.onboard();
    let id = env.create_account(checking(Some(("500.000", "2026-11-01"))))["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let logged = env.log_len();
    let mut renamed = checking(Some(("500.000", "2026-11-01")));
    renamed["name"] = json!("Corriente");
    let a = env.ok("account_update", json!({ "id": id, "input": renamed }));
    assert_eq!(a["name"], "Corriente");
    let new = &env.log()[logged..];
    assert_eq!(new.len(), 1, "{new:?}");
    let (entity, eid, _, _, f) = &new[0];
    assert_eq!((entity.as_str(), eid.as_str()), ("account", id.as_str()));
    assert_eq!(
        f.as_object().unwrap().keys().collect::<Vec<_>>(),
        vec!["name"]
    );

    // The edit dialog re-sends the opening balance as its display text (X-8).
    let logged = env.log_len();
    let mut resend = checking(Some(("$500.000", "2026-11-01")));
    resend["name"] = json!("Corriente");
    env.ok("account_update", json!({ "id": id, "input": resend }));
    assert_eq!(env.log_len(), logged);
}

#[test]
fn c11_opening_balance_edit_rewrites_in_place_removal_soft_deletes_and_readding_restores() {
    let mut env = Env::new("c11");
    env.onboard();
    let id = env.create_account(checking(Some(("500.000", "2026-11-01"))))["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let entry = env.opening_entry(&id);
    let ids = env.posting_ids(&entry);

    env.ok(
        "account_update",
        json!({ "id": id, "input": checking(Some(("750.000", "2026-11-02"))) }),
    );
    assert_eq!(env.posting_ids(&entry), ids);
    assert_eq!(env.summary(&id).unwrap()["balance"]["minor"], "750000");
    assert_eq!(
        env.ok("account_get", json!({ "id": id }))["openingBalance"]["date"],
        "2026-11-02"
    );

    env.ok(
        "account_update",
        json!({ "id": id, "input": checking(None) }),
    );
    assert_eq!(
        env.int(&format!(
            "SELECT deleted FROM journal_entry WHERE id = '{entry}'"
        )),
        1
    );
    assert_eq!(
        env.int(&format!(
            "SELECT min(deleted) FROM posting WHERE entry_id = '{entry}'"
        )),
        1
    );
    assert_eq!(env.summary(&id).unwrap()["balance"]["minor"], "0");
    assert_eq!(
        env.ok("account_get", json!({ "id": id }))["openingBalance"],
        Value::Null
    );

    env.ok(
        "account_update",
        json!({ "id": id, "input": checking(Some(("100.000", "2026-11-03"))) }),
    );
    assert_eq!(env.opening_entry(&id), entry);
    assert_eq!(env.posting_ids(&entry), ids);
    assert_eq!(
        env.int(
            "SELECT count(*) FROM journal_entry WHERE kind = 'opening_balance' AND deleted = 0"
        ),
        1
    );
    assert_eq!(env.summary(&id).unwrap()["balance"]["minor"], "100000");
}

#[test]
fn c12_zero_opening_balance_means_none_on_create_and_removes_it_on_update() {
    let mut env = Env::new("c12");
    env.onboard();
    let entries = env.int("SELECT count(*) FROM journal_entry");
    let zero = env.create_account(checking(Some(("0", "2026-11-01"))));
    let zid = zero["id"].as_str().unwrap().to_owned();
    assert_eq!(env.int("SELECT count(*) FROM journal_entry"), entries);
    assert_eq!(env.summary(&zid).unwrap()["balance"]["minor"], "0");
    assert_eq!(
        env.ok("account_get", json!({ "id": zid }))["openingBalance"],
        Value::Null
    );

    // Contrast: a non-zero one is an entry, and updating it to zero removes it.
    let id = env.create_account(checking(Some(("500.000", "2026-11-01"))))["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let entry = env.opening_entry(&id);
    env.ok(
        "account_update",
        json!({ "id": id, "input": checking(Some(("0", "2026-11-01"))) }),
    );
    assert_eq!(
        env.int(&format!(
            "SELECT deleted FROM journal_entry WHERE id = '{entry}'"
        )),
        1
    );
    assert_eq!(
        env.ok("account_get", json!({ "id": id }))["openingBalance"],
        Value::Null
    );
}

#[test]
fn c13_currency_change_without_other_postings_moves_opening_balance_legs() {
    let mut env = Env::new("c13");
    env.onboard();
    let id = env.create_account(checking(Some(("500.000", "2026-11-01"))))["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let entry = env.opening_entry(&id);
    let input = json!({
        "name": "Cuenta corriente", "kind": "checking", "currency": "USD",
        "openingBalance": { "amount": { "amount": "500,00", "currency": "USD" }, "date": "2026-11-01" }
    });
    let a = env.ok("account_update", json!({ "id": id, "input": input }));
    assert_eq!(a["currency"], "USD");
    assert_eq!(
        env.texts(&format!(
            "SELECT DISTINCT currency_code || ':' || abs(amount_minor) FROM posting WHERE entry_id = '{entry}'"
        )),
        ["USD:50000"]
    );
    let b = &env.summary(&id).unwrap()["balance"];
    assert_eq!(
        (&b["minor"], &b["currency"]),
        (&json!("50000"), &json!("USD"))
    );
    assert_eq!(env.int("SELECT count(*) FROM pragma_foreign_key_check"), 0);
}

#[test]
fn c14_category_tree_by_kind_returns_the_nested_seeded_tree() {
    let mut env = Env::new("c14");
    env.onboard();
    let tree = env.ok("category_tree", json!({ "query": { "kind": "expense" } }));
    let nodes = flatten(&tree);
    assert_eq!(
        nodes.len() as i64,
        env.int("SELECT count(*) FROM category WHERE deleted = 0 AND kind = 'expense'")
    );
    assert!(nodes.iter().all(|n| n["kind"] == "expense"));
    let food = seed_id("expense.food");
    let roots = tree.as_array().unwrap();
    let food_node = roots
        .iter()
        .find(|n| n["id"] == food.as_str())
        .expect("root");
    assert_eq!(food_node["parentId"], Value::Null);
    assert!(!food_node["name"].as_str().unwrap().is_empty());
    let supermarket = seed_id("expense.food.supermarket");
    let child = node(&flatten(&food_node["children"]), &supermarket)
        .expect("child nested under its parent")
        .clone();
    assert_eq!(child["parentId"], food.as_str());
    assert_eq!(child["isArchived"], false);

    let all = flatten(&env.ok("category_tree", json!({ "query": {} })));
    assert!(all.iter().any(|n| n["kind"] == "income"));
    assert!(all.iter().any(|n| n["kind"] == "expense"));
    assert_eq!(
        all.len() as i64,
        env.int("SELECT count(*) FROM category WHERE deleted = 0")
    );
}

#[test]
fn c15_category_tree_has_user_rows_any_depth_flags_archived_and_drops_deleted_and_merged() {
    let mut env = Env::new("c15");
    env.onboard();
    let sup = seed_id("expense.food.supermarket");
    env.exec(&format!(
        "INSERT INTO category (id, parent_id, kind, name, name_key, hlc, hlc_device) \
         VALUES ('organic', '{sup}', 'expense', 'Orgánico', 'organico', 1, 'dev-a');
         UPDATE category SET is_archived = 1 WHERE id = '{}';",
        seed_id("expense.food.restaurants")
    ));
    let cinema = seed_id("expense.leisure.cinema");
    let delivery = seed_id("expense.food.delivery");
    env.app.category_delete(&cinema, None).unwrap();
    env.app
        .category_delete(&delivery, Some(&seed_id("expense.food.restaurants")))
        .unwrap();

    let nodes = flatten(&env.ok("category_tree", json!({ "query": { "kind": "expense" } })));
    let organic = node(&nodes, "organic").expect("user sub-category at depth 3");
    assert_eq!(organic["parentId"], sup.as_str());
    assert_eq!(
        node(&nodes, &seed_id("expense.food.restaurants")).unwrap()["isArchived"],
        true
    );
    assert!(node(&nodes, &cinema).is_none(), "deleted category listed");
    assert!(node(&nodes, &delivery).is_none(), "merged category listed");
}

#[test]
fn c16_category_tree_is_catalogued_dispatchable_and_in_the_bindings() {
    assert!(COMMANDS.contains(&"category_tree"));
    let mut env = Env::new("c16");
    let r = env.call("category_tree", json!({ "query": {} }));
    assert!(
        !matches!(&r, Err(e) if e.code == ErrorCode::UnknownCommand || e.code == ErrorCode::NotImplemented),
        "{r:?}"
    );
    let bindings = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../packages/api-client/src/bindings.ts"),
    )
    .unwrap();
    for ty in ["CategoryNodeDto", "CategoryTreeQuery", "CategoryKind"] {
        assert!(
            bindings.contains(&format!("export type {ty} ")),
            "{ty} missing from bindings.ts"
        );
    }
}

#[test]
fn c17_edge_other_postings_freeze_currency_and_opening_balance_but_not_an_unchanged_resend() {
    for deleted in [0, 1] {
        let mut env = Env::new(&format!("c17-{deleted}"));
        env.onboard();
        let id = env.create_account(checking(Some(("500.000", "2026-11-01"))))["id"]
            .as_str()
            .unwrap()
            .to_owned();
        let sup = seed_id("expense.food.supermarket");
        env.exec(&format!(
            "INSERT INTO journal_entry (id, entry_date, deleted, hlc, hlc_device) VALUES ('e', '2026-11-05', {deleted}, 1, 'dev-a');
             INSERT INTO posting (id, entry_id, leg, account_id, amount_minor, currency_code, status, deleted, hlc, hlc_device)
                 VALUES ('p1', 'e', 'account', '{id}', -45990, 'CLP', 'cleared', {deleted}, 1, 'dev-a');
             INSERT INTO posting (id, entry_id, leg, category_id, amount_minor, currency_code, deleted, hlc, hlc_device)
                 VALUES ('p2', 'e', 'category', '{sup}', 45990, 'CLP', {deleted}, 1, 'dev-a');"
        ));
        let before = env.state();
        let usd = json!({ "name": "Cuenta corriente", "kind": "checking", "currency": "USD" });
        let e = env.err("account_update", json!({ "id": id, "input": usd }));
        assert_eq!(
            e.code,
            ErrorCode::CurrencyMismatch,
            "deleted={deleted}: {e}"
        );
        assert!(fields(&e).contains_key("currency"), "{e:?}");
        assert_eq!(env.state(), before);

        let e = env.err(
            "account_update",
            json!({ "id": id, "input": checking(Some(("600.000", "2026-11-01"))) }),
        );
        field_error(
            &e,
            ErrorCode::Validation,
            "openingBalance.amount",
            "errors.field.lockedByPostings",
        );
        assert_eq!(env.state(), before);

        // Contrast: re-sending the same currency and opening balance is no change (X-8).
        let mut same = checking(Some(("$500.000", "2026-11-01")));
        same["name"] = json!("Corriente");
        let a = env.ok("account_update", json!({ "id": id, "input": same }));
        assert_eq!(a["name"], "Corriente");
    }
}

#[test]
fn c18_edge_household_create_validates_names_and_currency_and_writes_nothing() {
    let mut env = Env::new("c18");
    let before = env.state();
    let long = "x".repeat(101);
    let cases = [
        (
            json!({ "name": " ", "myName": "Yo", "baseCurrency": "CLP" }),
            "name",
            "errors.field.required",
        ),
        (
            json!({ "name": "Casa", "myName": "", "baseCurrency": "CLP" }),
            "myName",
            "errors.field.required",
        ),
        (
            json!({ "name": long, "myName": "Yo", "baseCurrency": "CLP" }),
            "name",
            "errors.field.tooLong",
        ),
        (
            json!({ "name": "Casa", "myName": "Yo", "baseCurrency": "XYZ" }),
            "baseCurrency",
            "errors.field.unsupportedCurrency",
        ),
    ];
    for (input, field, key) in cases {
        let e = env.err("household_create", json!({ "input": input }));
        field_error(&e, ErrorCode::Validation, field, key);
        assert_eq!(env.state(), before, "{input}");
    }
    // Contrast: a name of exactly 100 chars is accepted.
    env.ok(
        "household_create",
        json!({ "input": { "name": "x".repeat(100), "myName": "Yo", "baseCurrency": "CLP" } }),
    );
}

#[test]
fn c19_edge_account_create_refuses_bad_opening_balance_and_visibility_and_writes_nothing() {
    let mut env = Env::new("c19");
    env.onboard();
    let before = env.state();
    let mut usd_amount = checking(None);
    usd_amount["openingBalance"] =
        json!({ "amount": { "amount": "10,00", "currency": "USD" }, "date": "2026-11-01" });
    let e = env.err("account_create", json!({ "input": usd_amount }));
    assert_eq!(e.code, ErrorCode::CurrencyMismatch, "{e}");
    assert!(fields(&e).contains_key("openingBalance.amount"), "{e:?}");
    assert_eq!(env.state(), before);

    let e = env.err(
        "account_create",
        json!({ "input": checking(Some(("1.2.3", "2026-11-01"))) }),
    );
    field_error(
        &e,
        ErrorCode::Validation,
        "openingBalance.amount",
        "errors.field.invalidAmount",
    );
    let e = env.err(
        "account_create",
        json!({ "input": checking(Some(("500.000", "2026-02-30"))) }),
    );
    field_error(
        &e,
        ErrorCode::Validation,
        "openingBalance.date",
        "errors.field.invalidDate",
    );
    let mut private_household = checking(None);
    private_household["visibility"] = json!("private");
    let e = env.err("account_create", json!({ "input": private_household }));
    field_error(
        &e,
        ErrorCode::Validation,
        "visibility",
        "errors.field.notAllowed",
    );
    assert_eq!(env.state(), before);
}

#[test]
fn c20_edge_account_commands_refuse_non_uuid_ids_and_unknown_ids() {
    let mut env = Env::new("c20");
    env.onboard();
    let calls = |id: &str| {
        vec![
            ("account_get", json!({ "id": id })),
            (
                "account_update",
                json!({ "id": id, "input": checking(None) }),
            ),
            ("account_archive", json!({ "id": id })),
        ]
    };
    for (name, args) in calls("not-a-uuid") {
        assert_eq!(env.err(name, args).code, ErrorCode::Validation, "{name}");
    }
    for (name, args) in calls(uuid()) {
        assert_eq!(env.err(name, args).code, ErrorCode::NotFound, "{name}");
    }
}

#[test]
fn c21_abuse_full_card_numbers_and_oversized_text_are_refused_and_write_nothing() {
    let mut env = Env::new("c21");
    env.onboard();
    let before = env.state();
    let with = |key: &str, value: Value| {
        let mut v = checking(None);
        v[key] = value;
        v
    };
    let cases = [
        (
            with("numberMask", json!("4111 1111 1111 1111")),
            "numberMask",
            "errors.field.fullNumber",
        ),
        (
            with("numberMask", json!("•".repeat(21))),
            "numberMask",
            "errors.field.tooLong",
        ),
        (
            with("name", json!("n".repeat(101))),
            "name",
            "errors.field.tooLong",
        ),
        (with("name", json!("  ")), "name", "errors.field.required"),
        (
            with("institution", json!("i".repeat(101))),
            "institution",
            "errors.field.tooLong",
        ),
    ];
    for (input, field, key) in cases {
        let e = env.err("account_create", json!({ "input": input }));
        field_error(&e, ErrorCode::Validation, field, key);
        assert_eq!(env.state(), before, "{field}");
    }
    // Contrast: a masked number with its last four digits is accepted.
    let a = env.create_account(with("numberMask", json!("•••• 4417")));
    assert_eq!(a["numberMask"], "•••• 4417");
}

#[test]
fn c22_abuse_overflowing_amount_is_validation_not_a_panic() {
    let mut env = Env::new("c22");
    env.onboard();
    let before = env.state();
    let e = env.err(
        "account_create",
        json!({ "input": checking(Some(("99.999.999.999.999.999.999", "2026-11-01"))) }),
    );
    field_error(
        &e,
        ErrorCode::Validation,
        "openingBalance.amount",
        "errors.field.invalidAmount",
    );
    assert_eq!(env.state(), before);
}

#[test]
fn c23_catalogue_only_tx_commands_are_still_not_implemented_and_malformed_args_are_validation() {
    let mut env = Env::new("c23");
    env.onboard();
    let account = env.create_account(checking(None))["id"].clone();
    let money = clp("1.000");
    let expense = json!({
        "type": "expense", "accountId": account, "categoryId": seed_id("expense.food"),
        "amount": money, "date": "2026-11-21"
    });
    let mine = [
        ("app_bootstrap", json!({})),
        ("settings_get", json!({})),
        ("account_list", json!({ "query": { "view": "combined" } })),
        ("account_get", json!({ "id": account })),
        (
            "account_update",
            json!({ "id": account, "input": checking(None) }),
        ),
        ("account_archive", json!({ "id": account })),
        ("account_create", json!({ "input": checking(None) })),
        ("category_tree", json!({ "query": {} })),
    ];
    for (name, args) in mine {
        let r = env.call(name, args);
        assert!(r.is_ok(), "{name}: {r:?}");
    }
    let tx = [
        ("tx_create", json!({ "input": expense })),
        ("tx_get", json!({ "id": uuid() })),
        ("tx_list", json!({ "query": { "view": "combined" } })),
        ("tx_update", json!({ "id": uuid(), "input": expense })),
        ("tx_delete", json!({ "id": uuid() })),
        ("tx_restore", json!({ "id": uuid() })),
    ];
    for (name, args) in tx {
        assert_eq!(
            env.err(name, args).code,
            ErrorCode::NotImplemented,
            "{name}"
        );
    }
    for name in COMMANDS {
        if ["app_health", "app_bootstrap", "settings_get"].contains(name) {
            continue; // take no arguments
        }
        let e = env.err(
            name,
            json!({ "id": 1, "input": { "type": "split" }, "query": 7 }),
        );
        assert_eq!(e.code, ErrorCode::Validation, "{name}: {e}");
    }
}

// ───────────────────────────── journey ─────────────────────────────

#[test]
fn journey_onboard_create_accounts_see_balances_archive_and_pick_a_category() {
    let mut env = Env::new("journey");
    assert_eq!(env.ok("app_bootstrap", json!({}))["household"], Value::Null);
    env.onboard();
    let checking_id = env.create_account(checking(Some(("500.000", "2026-11-01"))))["id"].clone();
    let savings_id = env.create_account(json!({
        "name": "Ahorro vacaciones", "kind": "savings", "currency": "CLP",
        "openingBalance": { "amount": clp("1.250.000"), "date": "2026-11-01" }
    }))["id"]
        .clone();
    let rows = env.list("combined", false);
    let balance = |id: &Value| {
        rows.iter()
            .find(|s| &s["account"]["id"] == id)
            .map(|s| s["balance"]["display"].clone())
    };
    assert_eq!(balance(&checking_id), Some(json!("$500.000")));
    assert_eq!(balance(&savings_id), Some(json!("$1.250.000")));

    env.ok("account_archive", json!({ "id": checking_id }));
    assert!(
        !env.list("combined", false)
            .iter()
            .any(|s| s["account"]["id"] == checking_id)
    );
    let tree = flatten(&env.ok("category_tree", json!({ "query": { "kind": "expense" } })));
    assert!(node(&tree, &seed_id("expense.food.supermarket")).is_some());
}

#[test]
fn c24_eur_opening_balance_resent_as_its_display_text_is_no_change() {
    let mut env = Env::new("c24");
    env.onboard();
    let eur = |amount: &str| {
        json!({
            "name": "Cuenta euros", "kind": "checking", "currency": "EUR",
            "openingBalance": { "amount": { "amount": amount, "currency": "EUR" }, "date": "2026-11-01" }
        })
    };
    let id = env.create_account(eur("1.234,56"))["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let display = env.ok("account_get", json!({ "id": id }))["openingBalance"]["amount"]["display"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(display, "€1.234,56");
    let logged = env.log_len();
    env.ok(
        "account_update",
        json!({ "id": id, "input": eur(&display) }),
    );
    assert_eq!(env.log_len(), logged);
}

// ───────────────────────────── remediation (ledger iteration 2) ─────────────────────────────

#[test]
fn f2_number_mask_with_more_than_four_digits_of_any_script_is_refused() {
    let mut env = Env::new("f2");
    env.onboard();
    let before = env.state();
    let mut input = checking(None);
    input["numberMask"] = json!("４１１１ １１１１ １１１１ １１１１");
    let e = env.err("account_create", json!({ "input": input }));
    field_error(&e, ErrorCode::Validation, "numberMask", "errors.field.fullNumber");
    assert_eq!(env.state(), before);
}

#[test]
fn f4_account_update_keeps_the_owner_of_an_existing_personal_account() {
    let mut env = Env::new("f4");
    let household = env.onboard()["id"].as_str().unwrap().to_owned();
    let mut input = checking(None);
    input["scope"] = json!("personal");
    let id = env.create_account(input.clone())["id"]
        .as_str()
        .unwrap()
        .to_owned();
    env.exec(&format!(
        "INSERT INTO member (id, household_id, display_name, hlc, hlc_device) \
         VALUES ('{}', '{household}', 'Otra', 1, 'dev-b');
         UPDATE account SET owner_member_id = '{}' WHERE id = '{id}';",
        uuid(),
        uuid()
    ));
    input["name"] = json!("Renombrada");
    let a = env.ok("account_update", json!({ "id": id, "input": input }));
    assert_eq!(a["ownerMemberId"], uuid());
    assert_eq!(a["name"], "Renombrada");
}
