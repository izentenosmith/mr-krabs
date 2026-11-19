//! Development-only HTTP shell (docker compose `back` service).
//!
//! Mirrors the Tauri shell: it only deserializes requests, calls
//! `mr_krabs_core::commands::dispatch`, and serializes the result. The React app talks to
//! it through `@mr-krabs/api-client`'s HTTP transport when it is not running inside Tauri.
//! It is never shipped to users.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use mr_krabs_core::{ApiError, App, ErrorCode, commands};
use serde_json::Value;
use tower_http::trace::TraceLayer;

type Shared = Arc<Mutex<App>>;

pub fn router(app: Shared) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/commands", get(list_commands))
        .route("/api/invoke/{command}", post(invoke))
        .layer(TraceLayer::new_for_http())
        .with_state(app)
}

async fn health(State(app): State<Shared>) -> Response {
    run(&app, "app_health", Value::Object(Default::default()))
}

async fn list_commands() -> Json<&'static [&'static str]> {
    Json(commands::COMMANDS)
}

async fn invoke(
    State(app): State<Shared>,
    Path(command): Path<String>,
    body: Option<Json<Value>>,
) -> Response {
    let args = body
        .map(|Json(v)| v)
        .unwrap_or_else(|| Value::Object(Default::default()));
    run(&app, &command, args)
}

fn run(app: &Shared, command: &str, args: Value) -> Response {
    let result = match app.lock() {
        Ok(guard) => commands::dispatch(&guard, command, args),
        Err(_) => Err(ApiError::internal("core lock poisoned")),
    };
    match result {
        Ok(v) => (StatusCode::OK, Json(v)).into_response(),
        Err(e) => (status_for(e.code), Json(e)).into_response(),
    }
}

fn status_for(code: ErrorCode) -> StatusCode {
    match code {
        ErrorCode::Validation => StatusCode::UNPROCESSABLE_ENTITY,
        ErrorCode::NotFound | ErrorCode::UnknownCommand => StatusCode::NOT_FOUND,
        ErrorCode::Internal => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,tower_http=info".into()),
        )
        .init();

    let db_path = std::env::var("MRK_DB_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| mr_krabs_core::default_db_file(&PathBuf::from(".data")));
    let bind: SocketAddr = std::env::var("MRK_BIND")
        .unwrap_or_else(|_| "127.0.0.1:7878".into())
        .parse()?;

    let app = App::open(&db_path)?;
    tracing::info!(db = %db_path.display(), %bind, "mr krabs devserver ready");

    let listener = tokio::net::TcpListener::bind(bind).await?;
    axum::serve(listener, router(Arc::new(Mutex::new(app))))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn test_router() -> Router {
        router(Arc::new(Mutex::new(App::open_in_memory().unwrap())))
    }

    async fn body_json(resp: Response) -> Value {
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn health_endpoint() {
        let resp = test_router()
            .oneshot(Request::get("/api/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(body_json(resp).await["status"], "ok");
    }

    #[tokio::test]
    async fn invoke_money_parse() {
        let req = Request::post("/api/invoke/money_parse")
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"input":{"amount":"$1.234.567","currency":"CLP"}}"#,
            ))
            .unwrap();
        let resp = test_router().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(body_json(resp).await["display"], "$1.234.567");
    }

    #[tokio::test]
    async fn validation_errors_are_422() {
        let req = Request::post("/api/invoke/money_parse")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"input":{"amount":"x","currency":"CLP"}}"#))
            .unwrap();
        let resp = test_router().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body_json(resp).await["code"], "VALIDATION");
    }

    #[tokio::test]
    async fn unknown_command_is_404() {
        let resp = test_router()
            .oneshot(
                Request::post("/api/invoke/nope")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
}
