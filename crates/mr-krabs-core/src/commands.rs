//! Name-based command catalogue. The Tauri shell exposes each command as a typed
//! `#[tauri::command]`; the devserver exposes the same names over HTTP via [`dispatch`].
//! Both end up calling the same `App` methods, so behavior is identical.
//!
//! JSON argument keys are the Tauri parameter names: `{input}`, `{id}`, `{id, input}`,
//! `{query}`, or `{}`.

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::api::{
    AccountInput, AccountListQuery, CategoryTreeQuery, HouseholdInput, NewTxInput, Page,
    TxDetailDto, TxQuery, TxRowDto,
};
use crate::{ApiError, App, ErrorCode};

/// Every command available to the UI, in catalogue order.
pub const COMMANDS: &[&str] = &[
    "app_health",
    "app_bootstrap",
    "household_create",
    "settings_get",
    "money_parse",
    "account_list",
    "account_get",
    "account_create",
    "account_update",
    "account_archive",
    "category_tree",
    "tx_create",
    "tx_get",
    "tx_list",
    "tx_update",
    "tx_delete",
    "tx_restore",
];

#[derive(serde::Deserialize)]
struct Input<T> {
    input: T,
}

#[derive(serde::Deserialize)]
struct Id {
    id: String,
}

#[derive(serde::Deserialize)]
struct IdInput<T> {
    id: String,
    input: T,
}

#[derive(serde::Deserialize)]
struct Query<T> {
    query: T,
}

/// Executes a command by name with JSON arguments (`{}` when the command takes none).
/// Arguments are deserialized before the command runs, so malformed input is `VALIDATION`.
pub fn dispatch(app: &mut App, name: &str, args: Value) -> Result<Value, ApiError> {
    match name {
        "app_health" => to_value(app.health()?),
        "app_bootstrap" => to_value(app.bootstrap()?),
        "household_create" => {
            let a: Input<HouseholdInput> = from_args(args)?;
            to_value(app.household_create(a.input)?)
        }
        "settings_get" => to_value(app.settings_get()?),
        "money_parse" => {
            let a: Input<crate::MoneyInput> = from_args(args)?;
            to_value(app.money_parse(a.input)?)
        }
        "account_list" => {
            let a: Query<AccountListQuery> = from_args(args)?;
            to_value(app.account_list(a.query)?)
        }
        "account_get" => {
            let a: Id = from_args(args)?;
            to_value(app.account_get(a.id)?)
        }
        "account_create" => {
            let a: Input<AccountInput> = from_args(args)?;
            to_value(app.account_create(a.input)?)
        }
        "account_update" => {
            let a: IdInput<AccountInput> = from_args(args)?;
            to_value(app.account_update(a.id, a.input)?)
        }
        "account_archive" => {
            let a: Id = from_args(args)?;
            to_value(app.account_archive(a.id)?)
        }
        "category_tree" => {
            let a: Query<CategoryTreeQuery> = from_args(args)?;
            to_value(app.category_tree(a.query)?)
        }
        "tx_create" => {
            let a: Input<NewTxInput> = from_args(args)?;
            to_value(app.tx_create(a.input)?)
        }
        "tx_get" => {
            let a: Id = from_args(args)?;
            to_value(app.tx_get(a.id)?)
        }
        "tx_list" => {
            let a: Query<TxQuery> = from_args(args)?;
            to_value(app.tx_list(a.query)?)
        }
        "tx_update" => {
            let a: IdInput<NewTxInput> = from_args(args)?;
            to_value(app.tx_update(a.id, a.input)?)
        }
        "tx_delete" => {
            let a: Id = from_args(args)?;
            to_value(app.tx_delete(a.id)?)
        }
        "tx_restore" => {
            let a: Id = from_args(args)?;
            to_value(app.tx_restore(a.id)?)
        }
        other => Err(ApiError::new(
            ErrorCode::UnknownCommand,
            format!("unknown command {other:?}"),
        )),
    }
}

/// M1 commands whose slice has not landed yet (design Q4, Q12). Each returns
/// `NOT_IMPLEMENTED`; the slice that builds it replaces the body (and may move the method
/// next to the rest of `App`), keeping the signature the shells and bindings rely on.
/// Household and account commands live in `household.rs` / `accounts.rs` (S4).
impl App {
    pub fn tx_update(&mut self, _id: String, _input: NewTxInput) -> Result<TxDetailDto, ApiError> {
        Err(ApiError::not_implemented("tx_update"))
    }
    pub fn tx_delete(&mut self, _id: String) -> Result<(), ApiError> {
        Err(ApiError::not_implemented("tx_delete"))
    }
    pub fn tx_restore(&mut self, _id: String) -> Result<(), ApiError> {
        Err(ApiError::not_implemented("tx_restore"))
    }
}

fn from_args<T: DeserializeOwned>(args: Value) -> Result<T, ApiError> {
    serde_json::from_value(args).map_err(ApiError::validation)
}

fn to_value<T: Serialize>(v: T) -> Result<Value, ApiError> {
    serde_json::to_value(v).map_err(ApiError::internal)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    fn app() -> App {
        App::open_in_memory().unwrap()
    }

    #[test]
    fn dispatches_health() {
        let v = dispatch(&mut app(), "app_health", json!({})).unwrap();
        assert_eq!(v["status"], "ok");
        assert!(v["schemaVersion"].is_number());
    }

    #[test]
    fn dispatches_money_parse_with_camel_case_args() {
        let v = dispatch(
            &mut app(),
            "money_parse",
            json!({ "input": { "amount": "27.990", "currency": "CLP" } }),
        )
        .unwrap();
        // Wire contract with @mr-krabs/api-client: camelCase, minor units as a string.
        insta::assert_json_snapshot!(v, @r#"
        {
          "currency": "CLP",
          "display": "$27.990",
          "minor": "27990"
        }
        "#);
    }

    #[test]
    fn bad_args_are_validation_errors() {
        let e = dispatch(&mut app(), "money_parse", json!({ "nope": 1 })).unwrap_err();
        assert_eq!(e.code, ErrorCode::Validation);
    }

    #[test]
    fn unknown_command() {
        let e = dispatch(&mut app(), "rm_rf", json!({})).unwrap_err();
        assert_eq!(e.code, ErrorCode::UnknownCommand);
    }

    #[test]
    fn catalogue_is_dispatchable() {
        let mut app = app();
        for name in COMMANDS {
            let r = dispatch(&mut app, name, json!({}));
            assert!(
                !matches!(
                    r,
                    Err(ApiError {
                        code: ErrorCode::UnknownCommand,
                        ..
                    })
                ),
                "{name} is listed but not dispatchable"
            );
        }
    }

    /// Valid documented arguments for every M1 command (the shapes in bindings.ts).
    fn m1_stub_calls() -> Vec<(&'static str, Value)> {
        let money = json!({ "amount": "27.990", "currency": "CLP" });
        let expense = json!({
            "type": "expense", "accountId": "a", "categoryId": "c",
            "amount": money, "date": "2026-11-21"
        });
        let account = json!({
            "name": "Cuenta", "kind": "checking", "currency": "CLP",
            "openingBalance": { "amount": money, "date": "2026-11-01" }
        });
        vec![
            ("app_bootstrap", json!({})),
            (
                "household_create",
                json!({ "input": { "name": "Casa", "myName": "Yo", "baseCurrency": "CLP" } }),
            ),
            ("settings_get", json!({})),
            ("account_list", json!({ "query": { "view": "combined" } })),
            ("account_get", json!({ "id": "a" })),
            ("account_create", json!({ "input": account })),
            ("account_update", json!({ "id": "a", "input": account })),
            ("account_archive", json!({ "id": "a" })),
            ("category_tree", json!({ "query": { "kind": "expense" } })),
            ("tx_create", json!({ "input": expense })),
            ("tx_get", json!({ "id": "t" })),
            (
                "tx_list",
                json!({ "query": { "view": "household", "accountId": "a", "limit": 500 } }),
            ),
            ("tx_update", json!({ "id": "t", "input": expense })),
            ("tx_delete", json!({ "id": "t" })),
            ("tx_restore", json!({ "id": "t" })),
        ]
    }

    #[test]
    fn catalogue_lists_every_m1_command() {
        let mut expected: Vec<&str> = vec!["app_health", "money_parse"];
        expected.extend(m1_stub_calls().iter().map(|(n, _)| *n));
        let mut listed = COMMANDS.to_vec();
        listed.sort_unstable();
        expected.sort_unstable();
        assert_eq!(listed, expected);
    }

    #[test]
    fn transaction_commands_with_valid_args_are_not_implemented_yet() {
        let mut app = app();
        for (name, args) in m1_stub_calls()
            .into_iter()
            .filter(|(n, _)| n.starts_with("tx_"))
        {
            let e = dispatch(&mut app, name, args).unwrap_err();
            assert_eq!(e.code, ErrorCode::NotImplemented, "{name}: {e}");
            assert!(e.message.contains(name), "{name}: {}", e.message);
        }
    }

    #[test]
    fn m1_commands_with_malformed_args_are_validation_errors() {
        let mut app = app();
        for (name, _) in m1_stub_calls() {
            if ["app_bootstrap", "settings_get"].contains(&name) {
                continue; // take no arguments
            }
            let e = dispatch(
                &mut app,
                name,
                json!({ "id": 1, "input": { "type": "split" } }),
            )
            .unwrap_err();
            assert_eq!(e.code, ErrorCode::Validation, "{name}: {e}");
        }
    }

    /// Generates `packages/api-client/src/bindings.ts` from the DTOs (design Q4).
    /// Fails when the committed file is stale; `MRK_UPDATE_BINDINGS=1` rewrites it.
    #[test]
    fn bindings_are_fresh() {
        use crate::api::*;
        let types = specta::Types::default()
            .register::<ErrorCode>()
            .register::<ApiError>()
            .register::<HealthDto>()
            .register::<MoneyDto>()
            .register::<MoneyInput>()
            .register::<BootstrapDto>()
            .register::<HouseholdInput>()
            .register::<HouseholdDto>()
            .register::<SettingsDto>()
            .register::<AccountListQuery>()
            .register::<AccountSummaryDto>()
            .register::<AccountDetailDto>()
            .register::<AccountInput>()
            .register::<AccountDto>()
            .register::<CategoryTreeQuery>()
            .register::<CategoryNodeDto>()
            .register::<NewTxInput>()
            .register::<TxQuery>()
            .register::<Page<TxRowDto>>()
            .register::<TxDetailDto>();
        let generated = specta_typescript::Typescript::default()
            .header(
                "// Generated from crates/mr-krabs-core/src/api.rs by the `bindings_are_fresh` test.\n\
                 // Do not edit: change the Rust DTOs, then run it with MRK_UPDATE_BINDINGS=1.",
            )
            .export(&types, specta_serde::Format)
            .unwrap();
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../packages/api-client/src/bindings.ts");
        if std::env::var_os("MRK_UPDATE_BINDINGS").is_some() {
            std::fs::write(&path, &generated).unwrap();
        }
        let committed = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(
            committed == generated,
            "{} is stale: run `MRK_UPDATE_BINDINGS=1 cargo test -p mr-krabs-core bindings_are_fresh` \
             and commit the result",
            path.display()
        );
    }
}
