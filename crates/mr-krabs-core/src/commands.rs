//! Name-based command catalogue. The Tauri shell exposes each command as a typed
//! `#[tauri::command]`; the devserver exposes the same names over HTTP via [`dispatch`].
//! Both end up calling the same `App` methods, so behavior is identical.

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::{ApiError, App, ErrorCode};

/// Every command available to the UI, in catalogue order.
pub const COMMANDS: &[&str] = &["app_health", "money_parse"];

/// Executes a command by name with JSON arguments (`{}` when the command takes none).
pub fn dispatch(app: &App, name: &str, args: Value) -> Result<Value, ApiError> {
    match name {
        "app_health" => to_value(app.health()?),
        "money_parse" => {
            #[derive(serde::Deserialize)]
            struct Args {
                input: crate::MoneyInput,
            }
            let a: Args = from_args(args)?;
            to_value(app.money_parse(a.input)?)
        }
        other => Err(ApiError::new(
            ErrorCode::UnknownCommand,
            format!("unknown command {other:?}"),
        )),
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

    #[test]
    fn dispatches_health() {
        let app = App::open_in_memory().unwrap();
        let v = dispatch(&app, "app_health", json!({})).unwrap();
        assert_eq!(v["status"], "ok");
        assert!(v["schemaVersion"].is_number());
    }

    #[test]
    fn dispatches_money_parse_with_camel_case_args() {
        let app = App::open_in_memory().unwrap();
        let v = dispatch(
            &app,
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
        let app = App::open_in_memory().unwrap();
        let e = dispatch(&app, "money_parse", json!({ "nope": 1 })).unwrap_err();
        assert_eq!(e.code, ErrorCode::Validation);
    }

    #[test]
    fn unknown_command() {
        let app = App::open_in_memory().unwrap();
        let e = dispatch(&app, "rm_rf", json!({})).unwrap_err();
        assert_eq!(e.code, ErrorCode::UnknownCommand);
    }

    #[test]
    fn catalogue_is_dispatchable() {
        let app = App::open_in_memory().unwrap();
        for name in COMMANDS {
            let r = dispatch(&app, name, json!({}));
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
}
