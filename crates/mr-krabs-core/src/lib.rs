//! Mr Krabs application core.
//!
//! The only crate the shells (Tauri desktop app, docker devserver) depend on.
//! Shells deserialize input, call [`App`] (directly or through [`commands::dispatch`]),
//! and serialize the result — nothing else (ADR-0002, `docs/04-command-boundary.md`).

pub mod api;
pub mod commands;

use std::path::{Path, PathBuf};

use mrk_money::{Currency, parse_es_cl};
use mrk_store::Store;

pub use api::{ApiError, ErrorCode, HealthDto, MoneyDto, MoneyInput};

/// Version of the core, reported to the UI.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub struct App {
    store: Store,
}

impl App {
    /// Opens (or creates) the database at `db_path` and runs pending migrations.
    pub fn open(db_path: &Path) -> Result<Self, ApiError> {
        let mut store = Store::open(db_path).map_err(ApiError::internal)?;
        store.migrate().map_err(ApiError::internal)?;
        Ok(Self { store })
    }

    /// In-memory app for tests.
    pub fn open_in_memory() -> Result<Self, ApiError> {
        let mut store = Store::open_in_memory().map_err(ApiError::internal)?;
        store.migrate().map_err(ApiError::internal)?;
        Ok(Self { store })
    }

    pub fn health(&self) -> Result<HealthDto, ApiError> {
        Ok(HealthDto {
            status: "ok".into(),
            version: VERSION.into(),
            schema_version: self.store.schema_version().map_err(ApiError::internal)?,
            latest_schema_version: Store::latest_version(),
            db_path: self.store.path().map(|p| p.display().to_string()),
        })
    }

    /// Parses user-typed money (es-CL) — the UI never parses or computes money itself.
    pub fn money_parse(&self, input: MoneyInput) -> Result<MoneyDto, ApiError> {
        let currency = Currency::built_in(&input.currency).ok_or_else(|| {
            ApiError::validation(format!("unsupported currency {:?}", input.currency))
        })?;
        let minor =
            parse_es_cl(&input.amount, currency.minor_units).map_err(ApiError::validation)?;
        Ok(MoneyDto::from_money(mrk_money::Money::new(
            minor,
            currency.code,
        )))
    }
}

/// Default database location used by shells when none is configured.
pub fn default_db_file(data_dir: &Path) -> PathBuf {
    data_dir.join("mrkrabs.sqlite")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_reports_schema() {
        let app = App::open_in_memory().unwrap();
        let h = app.health().unwrap();
        assert_eq!(h.status, "ok");
        assert_eq!(h.schema_version, h.latest_schema_version);
        assert_eq!(h.version, VERSION);
        assert!(h.db_path.is_none());
    }

    #[test]
    fn money_parse_returns_display_and_string_minor() {
        let app = App::open_in_memory().unwrap();
        let m = app
            .money_parse(MoneyInput {
                amount: "$1.234.567".into(),
                currency: "CLP".into(),
            })
            .unwrap();
        assert_eq!(m.minor, "1234567");
        assert_eq!(m.currency, "CLP");
        assert_eq!(m.display, "$1.234.567");
    }

    #[test]
    fn money_parse_rejects_bad_input() {
        let app = App::open_in_memory().unwrap();
        let err = app
            .money_parse(MoneyInput {
                amount: "1.234,5".into(),
                currency: "CLP".into(),
            })
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::Validation);
        let err = app
            .money_parse(MoneyInput {
                amount: "1".into(),
                currency: "XYZ".into(),
            })
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::Validation);
    }

    #[test]
    fn opens_database_file() {
        let dir = std::env::temp_dir().join(format!("mrk-core-test-{}", std::process::id()));
        let app = App::open(&default_db_file(&dir)).unwrap();
        assert!(
            app.health()
                .unwrap()
                .db_path
                .unwrap()
                .ends_with("mrkrabs.sqlite")
        );
        drop(app);
        let _ = std::fs::remove_dir_all(dir);
    }
}
