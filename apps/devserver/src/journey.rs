//! JRN-1, M1's done-when, end to end through the HTTP shell (ADR-0007) on a real database
//! file: onboarding in CLP, a checking account, two expenses and a transfer to savings, then
//! the balances, the transfer's shape, the change log, and the same balances after a reopen.
//!
//! Every step is an HTTP request to [`router`]; the file is read back directly only to check
//! what was persisted (the change log has no command).

use std::path::{Path as FsPath, PathBuf};
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use mr_krabs_core::App;
use mr_krabs_core::seed::seed_id;
use serde_json::{Value, json};
use tower::ServiceExt;

use super::router;

/// U+2212, the expense sign of MASTER §2.
const MINUS: char = '\u{2212}';

/// A fresh directory for the database file, removed when the test ends.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("mrk-devserver-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The devserver as `main` builds it, on the database file at `path`.
fn serve(path: &FsPath) -> Router {
    router(Arc::new(Mutex::new(App::open(path).unwrap())))
}

/// `POST /api/invoke/<command>`; panics unless the shell answers 200.
async fn post(app: &Router, command: &str, args: Value) -> Value {
    let req = Request::post(format!("/api/invoke/{command}"))
        .header("content-type", "application/json")
        .body(Body::from(args.to_string()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(status, StatusCode::OK, "{command} {args} → {body}");
    body
}

fn clp(amount: &str) -> Value {
    json!({ "amount": amount, "currency": "CLP" })
}

fn str_of<'a>(v: &'a Value, key: &str) -> &'a str {
    v[key]
        .as_str()
        .unwrap_or_else(|| panic!("{key} missing in {v}"))
}

/// Depth-first search of `category_tree`'s nodes by name.
fn find_category<'a>(nodes: &'a Value, name: &str) -> Option<&'a Value> {
    nodes.as_array()?.iter().find_map(|n| {
        if n["name"] == name {
            Some(n)
        } else {
            find_category(&n["children"], name)
        }
    })
}

/// `account id → (balance minor, balance display)` of every listed account.
async fn balances(app: &Router) -> Vec<(String, String, String)> {
    let list = post(
        app,
        "account_list",
        json!({ "query": { "view": "combined" } }),
    )
    .await;
    let mut out: Vec<_> = list
        .as_array()
        .unwrap()
        .iter()
        .map(|s| {
            (
                str_of(&s["account"], "id").to_owned(),
                str_of(&s["balance"], "minor").to_owned(),
                str_of(&s["balance"], "display").to_owned(),
            )
        })
        .collect();
    out.sort();
    out
}

fn balance_of(all: &[(String, String, String)], id: &str) -> (String, String) {
    let (_, minor, display) = all
        .iter()
        .find(|(a, _, _)| a == id)
        .unwrap_or_else(|| panic!("account {id} not listed in {all:?}"));
    (minor.clone(), display.clone())
}

#[tokio::test]
async fn jrn1_m1_done_when_over_http_on_a_file_db() {
    let dir = TempDir::new("jrn1");
    let db = dir.0.join("mrkrabs.sqlite");
    let app = serve(&db);

    // Onboarding in CLP.
    let household = post(
        &app,
        "household_create",
        json!({ "input": {
            "name": "Casa", "myName": "Yo", "baseCurrency": "CLP", "locale": "es-CL"
        } }),
    )
    .await;
    assert_eq!(household["baseCurrency"], "CLP");

    // A checking account with its opening balance.
    let checking = post(
        &app,
        "account_create",
        json!({ "input": {
            "name": "Cuenta corriente", "kind": "checking", "currency": "CLP",
            "openingBalance": { "amount": clp("1.000.000"), "date": "2026-11-01" }
        } }),
    )
    .await;
    let checking = str_of(&checking, "id").to_owned();

    // The category picker's source.
    let tree = post(
        &app,
        "category_tree",
        json!({ "query": { "kind": "expense" } }),
    )
    .await;
    let supermarket = find_category(&tree, "Supermercado").expect("Supermercado in the tree");
    let supermarket = str_of(supermarket, "id").to_owned();

    // The seeded savings account, as the sidebar lists it.
    let savings = seed_id("account.savings");
    let list = post(
        &app,
        "account_list",
        json!({ "query": { "view": "combined" } }),
    )
    .await;
    assert!(
        list.as_array()
            .unwrap()
            .iter()
            .any(|s| s["account"]["id"] == savings.as_str() && s["account"]["name"] == "Ahorro"),
        "seeded savings account not listed: {list}"
    );

    // Two expenses and a transfer to savings.
    let mut expenses = Vec::new();
    for (amount, date) in [("45.990", "2026-11-20"), ("12.500", "2026-11-21")] {
        let e = post(
            &app,
            "tx_create",
            json!({ "input": {
                "type": "expense", "accountId": checking, "categoryId": supermarket,
                "amount": clp(amount), "date": date, "description": "Supermercado"
            } }),
        )
        .await;
        assert_eq!(e["kind"], "expense");
        expenses.push(str_of(&e, "id").to_owned());
    }
    let transfer = post(
        &app,
        "tx_create",
        json!({ "input": {
            "type": "transfer", "fromAccountId": checking, "toAccountId": savings,
            "amount": clp("100.000"), "date": "2026-11-22", "description": "Ahorro"
        } }),
    )
    .await;
    let transfer = str_of(&transfer, "id").to_owned();

    // Sidebar balances, formatted in Rust (es-CL).
    let before = balances(&app).await;
    assert_eq!(
        balance_of(&before, &checking),
        ("841510".into(), "$841.510".into())
    );
    assert_eq!(
        balance_of(&before, &savings),
        ("100000".into(), "$100.000".into())
    );

    // The transfer is a transfer: no category, unsigned (MASTER §2); expenses carry −.
    let rows = post(&app, "tx_list", json!({ "query": { "view": "combined" } })).await;
    let rows = rows["items"].as_array().unwrap();
    let row = |id: &str| {
        rows.iter()
            .find(|r| r["id"] == id)
            .unwrap_or_else(|| panic!("{id} not in tx_list"))
    };
    let t = row(&transfer);
    assert_eq!(t["kind"], "transfer");
    assert_eq!(t["categoryId"], Value::Null);
    assert_eq!(t["categoryName"], Value::Null);
    let shown = str_of(&t["amount"], "display");
    assert_eq!(shown, "$100.000", "a transfer is shown unsigned");
    for id in &expenses {
        let e = row(id);
        assert_eq!(e["kind"], "expense");
        assert_eq!(e["categoryId"], supermarket.as_str());
        assert!(str_of(&e["amount"], "display").starts_with(MINUS), "{e}");
    }
    let expense_rows = rows.iter().filter(|r| r["kind"] == "expense").count();
    assert_eq!(
        expense_rows, 2,
        "the transfer is never counted as an expense"
    );

    // The change log, read straight from the file.
    {
        let conn = rusqlite::Connection::open(&db).unwrap();
        let category_legs: i64 = conn
            .query_row(
                "SELECT count(*) FROM posting WHERE entry_id = ?1 AND category_id IS NOT NULL",
                [&transfer],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(category_legs, 0, "a transfer has no category leg");

        let local: String = conn
            .query_row("SELECT id FROM device WHERE is_local = 1", [], |r| r.get(0))
            .unwrap();
        let mut st = conn
            .prepare("SELECT entity, entity_id, device_id, hlc FROM change_log ORDER BY seq")
            .unwrap();
        let log: Vec<(String, String, String, i64)> = st
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert!(!log.is_empty());
        assert!(
            log.iter().all(|(_, _, device, _)| *device == local),
            "every change is stamped by the local device"
        );
        assert!(
            log.windows(2).all(|w| w[0].3 < w[1].3),
            "HLC strictly increases in log order: {:?}",
            log.iter().map(|l| l.3).collect::<Vec<_>>()
        );
        let logged = |entity: &str, id: &str| {
            log.iter()
                .any(|(e, i, _, _)| e.as_str() == entity && i.as_str() == id)
        };

        // Every mutation: household, me, the account, and each entry with all its legs.
        let household_id = str_of(&household, "id");
        assert!(
            logged("household", household_id),
            "household_create not logged"
        );
        let me: String = conn
            .query_row(
                "SELECT id FROM member WHERE household_id = ?1",
                [household_id],
                |r| r.get(0),
            )
            .unwrap();
        assert!(logged("member", &me), "member not logged");
        assert!(logged("account", &checking), "account_create not logged");
        let mut entries = conn
            .prepare("SELECT id FROM journal_entry ORDER BY id")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .map(Result::unwrap)
            .collect::<Vec<_>>();
        assert_eq!(
            entries.len(),
            4,
            "opening balance, two expenses, one transfer"
        );
        let mut created = expenses.clone();
        created.push(transfer.clone());
        created.sort();
        entries.retain(|e| !created.contains(e));
        assert_eq!(entries.len(), 1, "the fourth entry is the opening balance");
        created.extend(entries);
        for entry in &created {
            assert!(logged("journal_entry", entry), "entry {entry} not logged");
            let legs = conn
                .prepare("SELECT id FROM posting WHERE entry_id = ?1")
                .unwrap()
                .query_map([entry], |r| r.get::<_, String>(0))
                .unwrap()
                .map(Result::unwrap)
                .collect::<Vec<_>>();
            assert_eq!(legs.len(), 2, "entry {entry} has two legs");
            for leg in legs {
                assert!(
                    logged("posting", &leg),
                    "posting {leg} of {entry} not logged"
                );
            }
        }

        // Seeded rows are deterministic on every device and are never logged (ADR-0008).
        let mut seeded = conn
            .prepare("SELECT id FROM category WHERE seed_key IS NOT NULL")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .map(Result::unwrap)
            .collect::<Vec<_>>();
        assert!(seeded.contains(&supermarket));
        seeded.extend(
            [
                "account.emergency_fund",
                "account.savings",
                "account.investments",
            ]
            .map(seed_id),
        );
        let leaked: Vec<_> = log
            .iter()
            .filter(|(_, id, _, _)| seeded.contains(id))
            .collect();
        assert!(leaked.is_empty(), "seeded rows were logged: {leaked:?}");
    }

    // Reopen the same file: the balances are the same.
    drop(app);
    let reopened = serve(&db);
    assert_eq!(balances(&reopened).await, before);
}
