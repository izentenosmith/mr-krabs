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
    AccountInput, AccountListQuery, BreakdownDto, BudgetCellDto, BudgetCopyInput, BudgetMatrixDto,
    BudgetMatrixQuery, BudgetSetInput, CalendarDto, CashFlowDto, CashFlowQuery,
    CategoryArchiveInput, CategoryDeleteInput, CategoryDto, CategoryInput, CategoryMoveInput,
    CategoryTreeQuery, CategoryUpdateInput, HouseholdInput, MonthCalendarQuery, NewTxInput,
    SpendingQuery, TxQuery,
};
use crate::support::command;
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
    "category_create",
    "category_update",
    "category_move",
    "category_archive",
    "category_delete",
    "budget_set",
    "budget_copy_month",
    "budget_matrix",
    "report_month_calendar",
    "report_cash_flow",
    "report_spending",
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
        "category_create" => {
            let a: Input<CategoryInput> = from_args(args)?;
            to_value(app.category_create(a.input)?)
        }
        "category_update" => {
            let a: Input<CategoryUpdateInput> = from_args(args)?;
            to_value(app.category_update(a.input)?)
        }
        "category_move" => {
            let a: Input<CategoryMoveInput> = from_args(args)?;
            to_value(app.category_move(a.input)?)
        }
        "category_archive" => {
            let a: Input<CategoryArchiveInput> = from_args(args)?;
            to_value(app.category_archive(a.input)?)
        }
        "category_delete" => {
            let a: Input<CategoryDeleteInput> = from_args(args)?;
            to_value(app.category_delete(&a.input.id, a.input.merge_into_id.as_deref())?)
        }
        "budget_set" => {
            let a: Input<BudgetSetInput> = from_args(args)?;
            to_value(app.budget_set(a.input)?)
        }
        "budget_copy_month" => {
            let a: Input<BudgetCopyInput> = from_args(args)?;
            to_value(app.budget_copy_month(a.input)?)
        }
        "budget_matrix" => {
            let a: Query<BudgetMatrixQuery> = from_args(args)?;
            to_value(app.budget_matrix(a.query)?)
        }
        "report_month_calendar" => {
            let a: Query<MonthCalendarQuery> = from_args(args)?;
            to_value(app.report_month_calendar(a.query)?)
        }
        "report_cash_flow" => {
            let a: Query<CashFlowQuery> = from_args(args)?;
            to_value(app.report_cash_flow(a.query)?)
        }
        "report_spending" => {
            let a: Query<SpendingQuery> = from_args(args)?;
            to_value(app.report_spending(a.query)?)
        }
        other => Err(ApiError::new(
            ErrorCode::UnknownCommand,
            format!("unknown command {other:?}"),
        )),
    }
}

/// M2 commands whose slice has not landed yet (DEC-31, as M1/DEC-2). Each returns
/// `NOT_IMPLEMENTED`; the slice that builds it (category-commands, budget-commands,
/// report-commands) replaces the body and may move the method next to the rest of `App`,
/// keeping the signature the shells and bindings rely on.
impl App {
    pub fn category_create(&mut self, _input: CategoryInput) -> Result<CategoryDto, ApiError> {
        command("category_create", || {
            Err(ApiError::not_implemented("category_create"))
        })
    }
    pub fn category_update(
        &mut self,
        _input: CategoryUpdateInput,
    ) -> Result<CategoryDto, ApiError> {
        command("category_update", || {
            Err(ApiError::not_implemented("category_update"))
        })
    }
    pub fn category_move(&mut self, _input: CategoryMoveInput) -> Result<CategoryDto, ApiError> {
        command("category_move", || {
            Err(ApiError::not_implemented("category_move"))
        })
    }
    pub fn category_archive(
        &mut self,
        _input: CategoryArchiveInput,
    ) -> Result<CategoryDto, ApiError> {
        command("category_archive", || {
            Err(ApiError::not_implemented("category_archive"))
        })
    }
    /// `null` when the amount was empty or zero and the line was deleted (or absent).
    pub fn budget_set(
        &mut self,
        _input: BudgetSetInput,
    ) -> Result<Option<BudgetCellDto>, ApiError> {
        command("budget_set", || {
            Err(ApiError::not_implemented("budget_set"))
        })
    }
    /// The number of lines written into `to`.
    pub fn budget_copy_month(&mut self, _input: BudgetCopyInput) -> Result<u32, ApiError> {
        command("budget_copy_month", || {
            Err(ApiError::not_implemented("budget_copy_month"))
        })
    }
    pub fn budget_matrix(&self, _query: BudgetMatrixQuery) -> Result<BudgetMatrixDto, ApiError> {
        command("budget_matrix", || {
            Err(ApiError::not_implemented("budget_matrix"))
        })
    }
    pub fn report_month_calendar(
        &self,
        _query: MonthCalendarQuery,
    ) -> Result<CalendarDto, ApiError> {
        command("report_month_calendar", || {
            Err(ApiError::not_implemented("report_month_calendar"))
        })
    }
    pub fn report_cash_flow(&self, _query: CashFlowQuery) -> Result<CashFlowDto, ApiError> {
        command("report_cash_flow", || {
            Err(ApiError::not_implemented("report_cash_flow"))
        })
    }
    pub fn report_spending(&self, _query: SpendingQuery) -> Result<BreakdownDto, ApiError> {
        command("report_spending", || {
            Err(ApiError::not_implemented("report_spending"))
        })
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

    /// Valid documented arguments for every M2 command (DEC-30, the shapes in bindings.ts).
    fn m2_calls() -> Vec<(&'static str, Value)> {
        vec![
            (
                "category_create",
                json!({ "input": { "kind": "expense", "parentId": "p", "name": "Cine", "icon": "film" } }),
            ),
            (
                "category_update",
                json!({ "input": { "id": "c", "name": "Cine" } }),
            ),
            (
                "category_move",
                json!({ "input": { "id": "c", "parentId": null, "index": 0 } }),
            ),
            (
                "category_archive",
                json!({ "input": { "id": "c", "archived": true } }),
            ),
            (
                "category_delete",
                json!({ "input": { "id": "c", "mergeIntoId": "d" } }),
            ),
            (
                "budget_set",
                json!({ "input": { "period": "2026-11", "categoryId": "c", "scope": "household", "amount": "150.000" } }),
            ),
            (
                "budget_copy_month",
                json!({ "input": { "from": "2026-10", "to": "2026-11" } }),
            ),
            (
                "budget_matrix",
                json!({ "query": { "year": 2026, "view": "combined" } }),
            ),
            (
                "report_month_calendar",
                json!({ "query": { "month": "2026-11", "view": "household" } }),
            ),
            (
                "report_cash_flow",
                json!({ "query": { "from": "2026-01", "to": "2026-11", "view": "combined" } }),
            ),
            (
                "report_spending",
                json!({ "query": { "from": "2026-11", "to": "2026-11", "view": "personal" } }),
            ),
        ]
    }

    #[test]
    fn catalogue_lists_every_m1_and_m2_command() {
        let mut expected: Vec<&str> = vec!["app_health", "money_parse"];
        expected.extend(m1_stub_calls().iter().map(|(n, _)| *n));
        expected.extend(m2_calls().iter().map(|(n, _)| *n));
        let mut listed = COMMANDS.to_vec();
        listed.sort_unstable();
        expected.sort_unstable();
        assert_eq!(listed, expected);
    }

    #[test]
    fn commands_with_malformed_args_are_validation_errors() {
        let mut app = app();
        for (name, _) in m1_stub_calls().into_iter().chain(m2_calls()) {
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

    /// REQ-32: until their slices land, M2 commands accept their documented arguments and
    /// answer `NOT_IMPLEMENTED` (`category_delete` is wired for real, below).
    #[test]
    fn m2_stubs_accept_their_arguments_and_answer_not_implemented() {
        let mut app = household_app();
        for (name, args) in m2_calls() {
            if name == "category_delete" {
                continue;
            }
            let e = dispatch(&mut app, name, args).unwrap_err();
            assert_eq!(e.code, ErrorCode::NotImplemented, "{name}: {e}");
            assert_eq!(e.message, format!("{name} is not implemented yet"));
        }
    }

    fn household_app() -> App {
        let mut app = app();
        dispatch(
            &mut app,
            "household_create",
            json!({ "input": { "name": "Casa", "myName": "Yo", "baseCurrency": "CLP" } }),
        )
        .unwrap();
        app
    }

    fn expense_tree(app: &mut App) -> Vec<Value> {
        let v = dispatch(
            app,
            "category_tree",
            json!({ "query": { "kind": "expense" } }),
        )
        .unwrap();
        v.as_array().unwrap().clone()
    }

    fn ids(nodes: &[Value]) -> Vec<String> {
        nodes
            .iter()
            .flat_map(|n| {
                let mut out = vec![n["id"].as_str().unwrap().to_owned()];
                out.extend(ids(n["children"].as_array().unwrap()));
                out
            })
            .collect()
    }

    /// REQ-32: `category_delete` dispatches to the real `App::category_delete`.
    #[test]
    fn category_delete_dispatches_for_real() {
        let mut app = household_app();
        let tree = expense_tree(&mut app);
        let group = tree
            .iter()
            .find(|n| !n["children"].as_array().unwrap().is_empty())
            .unwrap();
        let leaf = group["children"][0]["id"].as_str().unwrap().to_owned();
        let group_id = group["id"].as_str().unwrap().to_owned();
        let other_root = tree
            .iter()
            .map(|n| n["id"].as_str().unwrap().to_owned())
            .find(|id| *id != group_id)
            .unwrap();

        // An unused leaf is deleted outright.
        let v = dispatch(
            &mut app,
            "category_delete",
            json!({ "input": { "id": leaf } }),
        )
        .unwrap();
        assert_eq!(v, Value::Null);
        assert!(!ids(&expense_tree(&mut app)).contains(&leaf));
        let e = dispatch(
            &mut app,
            "category_delete",
            json!({ "input": { "id": leaf } }),
        )
        .unwrap_err();
        assert_eq!(e.code, ErrorCode::NotFound);

        // A group with sub-categories needs a merge target; with one, its children move.
        let e = dispatch(
            &mut app,
            "category_delete",
            json!({ "input": { "id": group_id } }),
        )
        .unwrap_err();
        assert_eq!(e.code, ErrorCode::Validation);
        let children = ids(group["children"].as_array().unwrap());
        dispatch(
            &mut app,
            "category_delete",
            json!({ "input": { "id": group_id, "mergeIntoId": other_root } }),
        )
        .unwrap();
        let tree = expense_tree(&mut app);
        assert!(!ids(&tree).contains(&group_id));
        let target = tree
            .iter()
            .find(|n| n["id"] == other_root.as_str())
            .unwrap();
        let moved = ids(target["children"].as_array().unwrap());
        for child in children.iter().filter(|c| **c != leaf) {
            assert!(
                moved.contains(child),
                "{child} did not move under the merge target"
            );
        }
    }

    /// `TxQuery.categoryId` is refused until report-commands builds the filter, so nobody
    /// takes an unfiltered page for a filtered one; without it `tx_list` is unchanged.
    #[test]
    fn tx_list_refuses_category_filter_until_built() {
        let mut app = household_app();
        let e = dispatch(
            &mut app,
            "tx_list",
            json!({ "query": { "view": "combined", "categoryId": "c" } }),
        )
        .unwrap_err();
        assert_eq!(e.code, ErrorCode::NotImplemented);
        let v = dispatch(
            &mut app,
            "tx_list",
            json!({ "query": { "view": "combined" } }),
        )
        .unwrap();
        assert_eq!(v["items"], json!([]));
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
            .register::<TxDetailDto>()
            .register::<CategoryDto>()
            .register::<CategoryInput>()
            .register::<CategoryUpdateInput>()
            .register::<CategoryMoveInput>()
            .register::<CategoryArchiveInput>()
            .register::<CategoryDeleteInput>()
            .register::<BudgetSetInput>()
            .register::<BudgetCellDto>()
            .register::<BudgetCopyInput>()
            .register::<BudgetMatrixQuery>()
            .register::<BudgetMatrixDto>()
            .register::<MonthCalendarQuery>()
            .register::<CalendarDto>()
            .register::<CashFlowQuery>()
            .register::<CashFlowDto>()
            .register::<SpendingQuery>()
            .register::<BreakdownDto>();
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
