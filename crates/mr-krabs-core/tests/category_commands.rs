//! Acceptance tests for slice K1 (category commands): REQ-15 – REQ-19 (DEC-30 Q10).
//! Every write goes through the command boundary (`dispatch`, the devserver's and Tauri's
//! shared entry) on an app opened on a temp file; the tree is read through `App`. What was
//! persisted (rows, change log) is read back through a second connection.

use std::path::PathBuf;

use mr_krabs_core::api::{CategoryDto, CategoryKind, CategoryNodeDto, CategoryTreeQuery};
use mr_krabs_core::commands::dispatch;
use mr_krabs_core::seed::seed_id;
use mr_krabs_core::{ApiError, App, ErrorCode};
use mrk_store::Store;
use serde_json::{Value, json};

/// The K1 commands, called by name through `dispatch` with the wire arguments of
/// bindings.ts, answering the decoded `CategoryDto`.
trait CategoryCommands {
    fn run(&mut self, name: &str, input: Value) -> Result<CategoryDto, ApiError>;
    fn cat_create(
        &mut self,
        kind: CategoryKind,
        parent_id: Option<&str>,
        name: &str,
        icon: Option<&str>,
    ) -> Result<CategoryDto, ApiError> {
        let input = json!({ "kind": kind, "parentId": parent_id, "name": name, "icon": icon });
        self.run("category_create", input)
    }
    fn cat_update(
        &mut self,
        id: &str,
        name: Option<&str>,
        icon: Option<&str>,
    ) -> Result<CategoryDto, ApiError> {
        self.run(
            "category_update",
            json!({ "id": id, "name": name, "icon": icon }),
        )
    }
    fn cat_move(
        &mut self,
        id: &str,
        parent_id: Option<&str>,
        index: usize,
    ) -> Result<CategoryDto, ApiError> {
        let input = json!({ "id": id, "parentId": parent_id, "index": index });
        self.run("category_move", input)
    }
    fn cat_delete(&mut self, id: &str, merge_into_id: Option<&str>) -> Result<(), ApiError>;
    fn cat_archive(&mut self, id: &str, archived: bool) -> Result<CategoryDto, ApiError> {
        self.run(
            "category_archive",
            json!({ "id": id, "archived": archived }),
        )
    }
}

impl CategoryCommands for App {
    fn run(&mut self, name: &str, input: Value) -> Result<CategoryDto, ApiError> {
        let out = dispatch(self, name, json!({ "input": input }))?;
        Ok(serde_json::from_value(out).unwrap())
    }
    fn cat_delete(&mut self, id: &str, merge_into_id: Option<&str>) -> Result<(), ApiError> {
        let input = json!({ "id": id, "mergeIntoId": merge_into_id });
        let out = dispatch(self, "category_delete", json!({ "input": input }))?;
        assert_eq!(out, Value::Null);
        Ok(())
    }
}

struct Env {
    dir: PathBuf,
    path: PathBuf,
    app: App,
}

impl Env {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("mrk-k1-{name}-{}", std::process::id()));
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
    fn log_count(&self) -> i64 {
        self.int("SELECT count(*) FROM change_log")
    }
    /// `(op, fields_json)` of every change-log row of one category, in log order.
    fn category_log(&self, id: &str) -> Vec<(String, Value)> {
        let raw = self.raw();
        let mut st = raw
            .connection()
            .prepare(
                "SELECT op, fields_json FROM change_log
                 WHERE entity = 'category' AND entity_id = ?1 ORDER BY seq",
            )
            .unwrap();
        st.query_map([id], |r| {
            let f: String = r.get(1)?;
            Ok((r.get(0)?, serde_json::from_str(&f).unwrap()))
        })
        .unwrap()
        .map(Result::unwrap)
        .collect()
    }

    fn tree(&self) -> Vec<CategoryNodeDto> {
        self.app
            .category_tree(CategoryTreeQuery { kind: None })
            .unwrap()
    }
    fn node(&self, id: &str) -> CategoryNodeDto {
        find(&self.tree(), id).unwrap_or_else(|| panic!("{id} not in the tree"))
    }
    fn child_ids(&self, parent: &str) -> Vec<String> {
        self.node(parent)
            .children
            .iter()
            .map(|c| c.id.clone())
            .collect()
    }

    fn create(&mut self, kind: CategoryKind, parent: Option<&str>, name: &str) -> String {
        self.app
            .cat_create(kind, parent, name, None)
            .unwrap_or_else(|e| panic!("create {name}: {e} {:?}", e.fields))
            .id
    }

    fn account(&mut self) -> String {
        let input = json!({ "name": "Cuenta", "kind": "checking", "currency": "CLP" });
        self.ok("account_create", json!({ "input": input }))["id"]
            .as_str()
            .unwrap()
            .to_owned()
    }
    fn expense(&mut self, account: &str, category: &str) -> Result<Value, ApiError> {
        self.call("tx_create", json!({ "input": expense(account, category) }))
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn expense(account: &str, category: &str) -> Value {
    json!({
        "type": "expense", "accountId": account, "categoryId": category,
        "amount": { "amount": "1.000", "currency": "CLP" }, "date": "2026-11-21",
        "description": "Compra"
    })
}

fn find(nodes: &[CategoryNodeDto], id: &str) -> Option<CategoryNodeDto> {
    nodes.iter().find_map(|n| {
        if n.id == id {
            Some(n.clone())
        } else {
            find(&n.children, id)
        }
    })
}

fn all_ids(nodes: &[CategoryNodeDto], out: &mut Vec<String>) {
    for n in nodes {
        out.push(n.id.clone());
        all_ids(&n.children, out);
    }
}

fn field(e: &ApiError, code: ErrorCode, field: &str, key: &str) {
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

const UNKNOWN: &str = "0190f0f0-0000-7000-8000-000000000000";

// ───────────────────────────── REQ-15 create ─────────────────────────────

#[test]
fn req15_create_places_a_category_of_either_kind_at_any_depth() {
    let mut env = Env::new("create");
    let food = seed_id("expense.food");
    let sup = seed_id("expense.food.supermarket");

    let organic = env.create(CategoryKind::Expense, Some(&sup), "  Orgánico ");
    let n = env.node(&organic);
    assert_eq!(n.name, "Orgánico");
    assert_eq!(n.parent_id.as_deref(), Some(sup.as_str()));
    assert!(!n.is_archived);
    // Appended after the existing children.
    let kids = env.child_ids(&food);
    let last = env.create(CategoryKind::Expense, Some(&food), "Panadería");
    assert_eq!(env.child_ids(&food), [kids, vec![last]].concat());

    let bonus = env.create(CategoryKind::Income, None, "Bonos");
    assert!(
        env.tree()
            .iter()
            .any(|r| r.id == bonus && r.parent_id.is_none())
    );
    let deep = env.create(CategoryKind::Expense, Some(&organic), "Verduras");
    assert_eq!(env.node(&deep).parent_id.as_deref(), Some(organic.as_str()));

    // One logged insert per create.
    let log = env.category_log(&organic);
    assert_eq!(log.len(), 1, "{log:?}");
    assert_eq!(log[0].0, "upsert");
    assert_eq!(log[0].1["name"]["new"], "Orgánico");
}

#[test]
fn req15_create_refuses_duplicates_wrong_kind_and_bad_names_writing_nothing() {
    let mut env = Env::new("create-refused");
    let food = seed_id("expense.food");
    let before = env.log_count();
    let app = &mut env.app;

    let e = app
        .cat_create(CategoryKind::Expense, Some(&food), "supermercado", None)
        .unwrap_err();
    field(&e, ErrorCode::Validation, "name", "errors.field.duplicate");
    let e = app
        .cat_create(CategoryKind::Expense, Some(&food), " SÚPERMERCADO ", None)
        .unwrap_err();
    field(&e, ErrorCode::Validation, "name", "errors.field.duplicate");
    let e = app
        .cat_create(CategoryKind::Expense, Some(&seed_id("income")), "X", None)
        .unwrap_err();
    field(
        &e,
        ErrorCode::Validation,
        "parentId",
        "errors.field.wrongKind",
    );
    let e = app
        .cat_create(CategoryKind::Expense, Some(UNKNOWN), "X", None)
        .unwrap_err();
    field(&e, ErrorCode::NotFound, "parentId", "errors.field.notFound");
    // A malformed id is just an unknown one.
    let e = app
        .cat_create(CategoryKind::Expense, Some("nope"), "X", None)
        .unwrap_err();
    field(&e, ErrorCode::NotFound, "parentId", "errors.field.notFound");
    let e = app
        .cat_create(CategoryKind::Expense, None, "   ", None)
        .unwrap_err();
    field(&e, ErrorCode::Validation, "name", "errors.field.required");
    let e = app
        .cat_create(CategoryKind::Expense, None, &"x".repeat(81), None)
        .unwrap_err();
    field(&e, ErrorCode::Validation, "name", "errors.field.tooLong");
    app.cat_create(CategoryKind::Expense, None, &"x".repeat(80), None)
        .unwrap();
    assert_eq!(env.log_count(), before + 1);

    // Same name under another parent, or in the other kind's tree, is fine.
    env.create(
        CategoryKind::Expense,
        Some(&seed_id("expense.leisure")),
        "Supermercado",
    );
    env.create(CategoryKind::Income, None, "Comida");
}

// ───────────────────────────── REQ-16 update ─────────────────────────────

#[test]
fn req16_rename_shows_on_past_entries_and_logs_one_row() {
    let mut env = Env::new("rename");
    let sup = seed_id("expense.food.supermarket");
    let account = env.account();
    let tx = env.expense(&account, &sup).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let log_before = env.category_log(&sup).len();

    let n = env
        .app
        .cat_update(&sup, Some(" Súper "), Some("cart"))
        .unwrap();
    assert_eq!(
        (n.name.as_str(), n.icon.as_deref()),
        ("Súper", Some("cart"))
    );
    let list = env.ok("tx_list", json!({ "query": { "view": "combined" } }));
    let item = list["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["id"] == tx.as_str())
        .unwrap();
    assert_eq!(item["categoryName"], "Súper");

    let log = env.category_log(&sup);
    assert_eq!(log.len(), log_before + 1, "{log:?}");
    let fields = log.last().unwrap().1.as_object().unwrap();
    let mut keys: Vec<&str> = fields.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, ["icon", "name", "name_key"]);

    // No-op: nothing logged. A blank icon clears it.
    env.app.cat_update(&sup, Some("Súper"), None).unwrap();
    assert_eq!(env.category_log(&sup).len(), log_before + 1);
    let n = env.app.cat_update(&sup, None, Some(" ")).unwrap();
    assert_eq!(n.icon, None);
}

#[test]
fn req16_update_refuses_duplicates_and_unknown_ids() {
    let mut env = Env::new("update-refused");
    let sup = seed_id("expense.food.supermarket");
    let e = env
        .app
        .cat_update(&sup, Some("Delivery"), None)
        .unwrap_err();
    field(&e, ErrorCode::Validation, "name", "errors.field.duplicate");
    // Renaming to its own name in another case is not a duplicate.
    env.app
        .cat_update(&sup, Some("SUPERMERCADO"), None)
        .unwrap();
    let e = env.app.cat_update(UNKNOWN, Some("X"), None).unwrap_err();
    assert_eq!(e.code, ErrorCode::NotFound);
    env.app
        .cat_delete(&seed_id("expense.leisure.cinema"), None)
        .unwrap();
    let e = env
        .app
        .cat_update(&seed_id("expense.leisure.cinema"), Some("X"), None)
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::NotFound);
}

// ───────────────────────────── REQ-17 move ─────────────────────────────

#[test]
fn req17_move_reparents_at_any_depth_and_reorders() {
    let mut env = Env::new("move");
    let food = seed_id("expense.food");
    let leisure = seed_id("expense.leisure");
    let delivery = seed_id("expense.food.delivery");

    let n = env.app.cat_move(&delivery, Some(&leisure), 0).unwrap();
    assert_eq!(n.parent_id.as_deref(), Some(leisure.as_str()));
    assert_eq!(env.child_ids(&leisure)[0], delivery);
    assert!(!env.child_ids(&food).contains(&delivery));

    // Reorder within the parent: last, then back to the middle.
    let kids = env.child_ids(&leisure);
    env.app
        .cat_move(&delivery, Some(&leisure), kids.len() - 1)
        .unwrap();
    assert_eq!(env.child_ids(&leisure).last(), Some(&delivery));
    env.app.cat_move(&delivery, Some(&leisure), 1).unwrap();
    let mut expected: Vec<String> = kids[1..].to_vec();
    expected.insert(1, delivery.clone());
    assert_eq!(env.child_ids(&leisure), expected);

    // To a root, and under a sub-category (depth 3).
    env.app.cat_move(&delivery, None, 0).unwrap();
    let roots: Vec<String> = env
        .app
        .category_tree(CategoryTreeQuery {
            kind: Some(CategoryKind::Expense),
        })
        .unwrap()
        .iter()
        .map(|r| r.id.clone())
        .collect();
    assert_eq!(roots[0], delivery);
    let sup = seed_id("expense.food.supermarket");
    env.app.cat_move(&delivery, Some(&sup), 0).unwrap();
    assert_eq!(env.child_ids(&sup), [delivery]);
}

#[test]
fn req17_move_refuses_cycles_kind_changes_collisions_and_bad_index() {
    let mut env = Env::new("move-refused");
    let food = seed_id("expense.food");
    let sup = seed_id("expense.food.supermarket");
    let organic = env.create(CategoryKind::Expense, Some(&sup), "Orgánico");
    let before = env.log_count();
    let app = &mut env.app;

    let e = app.cat_move(&food, Some(&organic), 0).unwrap_err();
    field(&e, ErrorCode::Validation, "parentId", "errors.field.cycle");
    let e = app.cat_move(&food, Some(&food), 0).unwrap_err();
    field(&e, ErrorCode::Validation, "parentId", "errors.field.cycle");
    let e = app.cat_move(&sup, Some(&seed_id("income")), 0).unwrap_err();
    field(
        &e,
        ErrorCode::Validation,
        "parentId",
        "errors.field.wrongKind",
    );
    let e = app.cat_move(&sup, Some(UNKNOWN), 0).unwrap_err();
    field(&e, ErrorCode::NotFound, "parentId", "errors.field.notFound");
    // Transporte has no "Supermercado"; Ocio's children are 3: index 4 is past the end.
    let leisure = seed_id("expense.leisure");
    let n = env.child_ids(&leisure).len();
    let e = env.app.cat_move(&sup, Some(&leisure), n + 1).unwrap_err();
    field(
        &e,
        ErrorCode::Validation,
        "index",
        "errors.field.outOfRange",
    );
    let e = env.app.cat_move(UNKNOWN, None, 0).unwrap_err();
    assert_eq!(e.code, ErrorCode::NotFound);
    assert_eq!(env.log_count(), before);

    // A name already under the destination.
    let dup = env.create(CategoryKind::Expense, Some(&leisure), "Supermercado");
    let e = env.app.cat_move(&sup, Some(&leisure), 0).unwrap_err();
    field(&e, ErrorCode::Validation, "name", "errors.field.duplicate");
    assert!(env.child_ids(&leisure).contains(&dup));
}

/// X-16 (M1 #29 F3): a live category under an unknown or deleted parent, or on a parent cycle,
/// is a root (a cycle is broken at its smallest id, the rule R0's roll-up shares); nothing
/// live is hidden and nothing loops.
#[test]
fn req17_x16_category_tree_roots_orphans_and_cycle_rows_hiding_nothing() {
    let env = Env::new("cycle");
    // Only sync can make these (M7); written raw here.
    let (a, b, c, orphan) = (
        "0190f0f0-0000-7000-8000-00000000000a",
        "0190f0f0-0000-7000-8000-00000000000b",
        "0190f0f0-0000-7000-8000-00000000000c",
        "0190f0f0-0000-7000-8000-00000000000d",
    );
    let cinema = seed_id("expense.leisure.cinema");
    env.exec(&format!(
        "PRAGMA foreign_keys = OFF;
         INSERT INTO category (id, parent_id, kind, name, name_key, hlc, hlc_device) VALUES
           ('{b}', '{a}', 'expense', 'B', 'b', 1, 'x'),
           ('{a}', '{c}', 'expense', 'A', 'a', 1, 'x'),
           ('{c}', '{b}', 'expense', 'C', 'c', 1, 'x'),
           ('{orphan}', '{UNKNOWN}', 'expense', 'D', 'd', 1, 'x');
         UPDATE category SET parent_id = '{a}' WHERE id = '{cinema}';
         UPDATE category SET deleted = 1 WHERE id = '{}';",
        seed_id("expense.transport")
    ));
    let tree = env.tree();
    let mut ids = Vec::new();
    all_ids(&tree, &mut ids);
    let live = env.int("SELECT count(*) FROM category WHERE deleted = 0");
    assert_eq!(ids.len() as i64, live, "every live category exactly once");
    let mut unique = ids.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), ids.len());

    let root = |id: &str| tree.iter().find(|r| r.id == id).cloned();
    let cycle_root = root(a).expect("the cycle's smallest id is a root");
    assert_eq!(cycle_root.parent_id, None);
    assert_eq!(env.node(b).parent_id.as_deref(), Some(a));
    assert_eq!(env.node(c).parent_id.as_deref(), Some(b));
    assert_eq!(env.node(&cinema).parent_id.as_deref(), Some(a));
    assert!(root(orphan).is_some(), "unknown parent: a root");
    for child in seed_children(&env, "expense.transport") {
        assert!(root(&child).is_some(), "deleted parent: a root");
    }
}

fn seed_children(env: &Env, parent: &str) -> Vec<String> {
    let raw = env.raw();
    let mut st = raw
        .connection()
        .prepare("SELECT id FROM category WHERE parent_id = ?1 AND deleted = 0")
        .unwrap();
    st.query_map([seed_id(parent)], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

// ───────────────────────────── REQ-18 archive ─────────────────────────────

#[test]
fn req18_archive_hides_the_subtree_and_unarchive_restores_it() {
    let mut env = Env::new("archive");
    let food = seed_id("expense.food");
    let sup = seed_id("expense.food.supermarket");
    let delivery = seed_id("expense.food.delivery");
    let organic = env.create(CategoryKind::Expense, Some(&sup), "Orgánico");
    // Archived on its own before the group.
    env.app.cat_archive(&delivery, true).unwrap();
    let before = env.log_count();

    let n = env.app.cat_archive(&food, true).unwrap();
    assert!(n.is_archived);
    assert_eq!(env.log_count(), before + 1, "one row written");
    for id in [&food, &sup, &organic, &delivery] {
        assert!(env.node(id).is_archived, "{id}");
    }
    // Derived at read time (X-19): only the group's own flag is stored.
    assert_eq!(
        env.int(&format!(
            "SELECT count(*) FROM category WHERE is_archived = 1 AND id IN ('{sup}', '{organic}')"
        )),
        0
    );
    // Idempotent: nothing logged.
    env.app.cat_archive(&food, true).unwrap();
    assert_eq!(env.log_count(), before + 1);

    env.app.cat_archive(&food, false).unwrap();
    // Unarchiving restores the subtree with one write: the group's own row.
    assert_eq!(env.log_count(), before + 2, "no write per child");
    assert_eq!(
        env.category_log(&food).last().unwrap().1["is_archived"]["new"],
        0
    );
    assert!(!env.node(&food).is_archived);
    assert!(!env.node(&organic).is_archived);
    assert!(env.node(&delivery).is_archived, "archived on its own");
}

#[test]
fn req18_archived_category_keeps_existing_entries_valid_and_refuses_new_ones() {
    let mut env = Env::new("archive-tx");
    let food = seed_id("expense.food");
    let sup = seed_id("expense.food.supermarket");
    let account = env.account();
    let tx = env.expense(&account, &sup).unwrap();
    let id = tx["id"].as_str().unwrap().to_owned();

    env.app.cat_archive(&food, true).unwrap();
    // New entry into a category archived through its group.
    let e = env.expense(&account, &sup).unwrap_err();
    field(
        &e,
        ErrorCode::Validation,
        "categoryId",
        "errors.field.archived",
    );
    // A no-op edit of the existing entry passes; so does changing only its header.
    env.ok(
        "tx_update",
        json!({ "id": id, "input": expense(&account, &sup) }),
    );
    let mut input = expense(&account, &sup);
    input["description"] = json!("Otra");
    env.ok("tx_update", json!({ "id": id, "input": input }));
    // Fixing the amount keeps the archived category; moving it into one is refused.
    let mut input = expense(&account, &sup);
    input["amount"] = json!({ "amount": "2.000", "currency": "CLP" });
    env.ok("tx_update", json!({ "id": id, "input": input }));
    let e = env
        .call(
            "tx_update",
            json!({ "id": id, "input": expense(&account, &seed_id("expense.food.delivery")) }),
        )
        .unwrap_err();
    field(
        &e,
        ErrorCode::Validation,
        "categoryId",
        "errors.field.archived",
    );
    // Creating or moving under an archived group is refused.
    let e = env
        .app
        .cat_create(CategoryKind::Expense, Some(&sup), "X", None)
        .unwrap_err();
    field(
        &e,
        ErrorCode::Validation,
        "parentId",
        "errors.field.archived",
    );
    let e = env
        .app
        .cat_move(&seed_id("expense.leisure.cinema"), Some(&food), 0)
        .unwrap_err();
    field(
        &e,
        ErrorCode::Validation,
        "parentId",
        "errors.field.archived",
    );

    env.app.cat_archive(&food, false).unwrap();
    env.expense(&account, &sup).unwrap();
}

// ───────────────────────────── REQ-19 delete ─────────────────────────────

#[test]
fn req19_delete_unused_and_used_needs_a_merge_target() {
    let mut env = Env::new("delete");
    let sup = seed_id("expense.food.supermarket");
    let cinema = seed_id("expense.leisure.cinema");
    let account = env.account();
    env.expense(&account, &sup).unwrap();

    env.app.cat_delete(&cinema, None).unwrap();
    assert!(find(&env.tree(), &cinema).is_none());
    let e = env.app.cat_delete(&cinema, None).unwrap_err();
    assert_eq!(e.code, ErrorCode::NotFound);

    let before = env.log_count();
    let e = env.app.cat_delete(&sup, None).unwrap_err();
    field(
        &e,
        ErrorCode::Validation,
        "mergeIntoId",
        "errors.field.mergeRequired",
    );
    let e = env
        .app
        .cat_delete(&sup, Some(&seed_id("income.salary")))
        .unwrap_err();
    field(
        &e,
        ErrorCode::Validation,
        "mergeIntoId",
        "errors.field.wrongKind",
    );
    let e = env.app.cat_delete(&sup, Some(UNKNOWN)).unwrap_err();
    field(
        &e,
        ErrorCode::NotFound,
        "mergeIntoId",
        "errors.field.notFound",
    );
    let e = env.app.cat_delete(&sup, Some("nope")).unwrap_err();
    field(
        &e,
        ErrorCode::NotFound,
        "mergeIntoId",
        "errors.field.notFound",
    );
    let e = env
        .app
        .cat_delete(&seed_id("expense.food"), Some(&sup))
        .unwrap_err();
    field(
        &e,
        ErrorCode::Validation,
        "mergeIntoId",
        "errors.field.cycle",
    );
    assert_eq!(env.log_count(), before);
}

#[test]
fn req19_merge_moves_postings_and_sums_colliding_budget_lines() {
    let mut env = Env::new("merge");
    let sup = seed_id("expense.food.supermarket");
    let del = seed_id("expense.food.delivery");
    let account = env.account();
    let tx = env.expense(&account, &sup).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let organic = env.create(CategoryKind::Expense, Some(&sup), "Orgánico");
    // Budget lines are written raw until B1's budget_set lands.
    env.exec(&format!(
        "INSERT INTO budget_line (id, period, category_id, scope, amount_minor, currency_code, hlc, hlc_device) VALUES
           ('b1', '2026-11', '{sup}', 'household', 300000, 'CLP', 1, 'x'),
           ('b2', '2026-12', '{sup}', 'household', 280000, 'CLP', 1, 'x'),
           ('b3', '2026-11', '{del}', 'household',  40000, 'CLP', 1, 'x');"
    ));

    env.app.cat_delete(&sup, Some(&del)).unwrap();

    let detail = env.ok("tx_get", json!({ "id": tx }));
    assert_eq!(detail["categoryId"], del.as_str(), "{detail}");
    assert_eq!(env.node(&organic).parent_id.as_deref(), Some(del.as_str()));
    assert_eq!(
        env.int("SELECT amount_minor FROM budget_line WHERE id = 'b3' AND deleted = 0"),
        340_000
    );
    assert_eq!(
        env.int("SELECT deleted FROM budget_line WHERE id = 'b1'"),
        1
    );
    assert_eq!(
        env.int(&format!(
            "SELECT count(*) FROM budget_line WHERE id = 'b2' AND category_id = '{del}'"
        )),
        1
    );
    assert!(find(&env.tree(), &sup).is_none());
}

/// Behaviour change (X-16, `sibling_key`): with no kind filter, income roots come before
/// expense roots, and siblings with the same sort order are ordered by accent-insensitive name.
#[test]
fn category_tree_orders_siblings_by_kind_sort_order_then_normalized_name() {
    let mut env = Env::new("order");
    let tree = env.tree();
    let first_expense = tree
        .iter()
        .position(|r| r.kind == CategoryKind::Expense)
        .unwrap();
    assert!(
        tree[..first_expense]
            .iter()
            .all(|r| r.kind == CategoryKind::Income)
    );
    assert!(
        tree[first_expense..]
            .iter()
            .all(|r| r.kind == CategoryKind::Expense)
    );

    let parent = env.create(CategoryKind::Expense, None, "Orden");
    let zeta = env.create(CategoryKind::Expense, Some(&parent), "Zeta");
    let alamo = env.create(CategoryKind::Expense, Some(&parent), "Álamo");
    env.exec(&format!(
        "UPDATE category SET sort_order = 0 WHERE id IN ('{zeta}', '{alamo}')"
    ));
    assert_eq!(env.child_ids(&parent), [alamo, zeta]);
}
