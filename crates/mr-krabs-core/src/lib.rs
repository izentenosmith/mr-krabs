//! Mr Krabs application core.
//!
//! The only crate the shells (Tauri desktop app, docker devserver) depend on.
//! Shells deserialize input, call [`App`] (directly or through [`commands::dispatch`]),
//! and serialize the result — nothing else (ADR-0002, `docs/04-command-boundary.md`).

mod accounts;
pub mod api;
mod categories;
pub mod commands;
mod household;
pub mod seed;
mod support;
mod transactions;

use std::path::{Path, PathBuf};

use mrk_money::{Currency, parse_es_cl};
use mrk_store::{SeedReport, Store};

pub use api::{ApiError, ErrorCode, HealthDto, MoneyDto, MoneyInput};

/// Version of the core, reported to the UI.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub struct App {
    store: Store,
}

impl App {
    /// Opens (or creates) the database at `db_path`, runs pending migrations and creates the
    /// local device on first open.
    pub fn open(db_path: &Path) -> Result<Self, ApiError> {
        Self::ready(Store::open(db_path).map_err(ApiError::internal)?)
    }

    /// In-memory app for tests.
    pub fn open_in_memory() -> Result<Self, ApiError> {
        Self::ready(Store::open_in_memory().map_err(ApiError::internal)?)
    }

    /// Migrates, then makes sure the local device exists and its clock is restored.
    fn ready(mut store: Store) -> Result<Self, ApiError> {
        store.migrate().map_err(ApiError::internal)?;
        store.ensure_local_device().map_err(ApiError::internal)?;
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
        let minor = parse_es_cl(&input.amount, currency).map_err(ApiError::validation)?;
        Ok(MoneyDto::from_money(mrk_money::Money::new(
            minor,
            currency.code,
        )))
    }

    /// Seeds the default categories and savings/investment accounts in the household's
    /// language and base currency (docs/01, ADR-0008). Idempotent, and never resurrects a
    /// seeded row the household deleted. `household_create` calls it; adoption will too.
    pub fn seed_household_defaults(&mut self) -> Result<SeedReport, ApiError> {
        let (categories, accounts) = seed_catalogue(&self.household_row()?)?;
        Ok(self.store.seed_defaults(&categories, &accounts)?)
    }
}

/// The default categories and accounts in the household's language and base currency.
pub(crate) fn seed_catalogue(
    household: &mrk_store::HouseholdRow,
) -> Result<(Vec<mrk_store::SeedCategory>, Vec<mrk_store::SeedAccount>), ApiError> {
    let locale = seed::Locale::parse(&household.locale)
        .ok_or_else(|| ApiError::internal(format!("unsupported locale {:?}", household.locale)))?;
    Ok((
        seed::seed_categories(locale),
        seed::seed_accounts(locale, &household.base_currency_code),
    ))
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

    /// X-12: a currency marker must name the currency being parsed.
    #[test]
    fn money_parse_checks_the_marker_against_the_currency() {
        let app = App::open_in_memory().unwrap();
        let parse = |amount: &str, currency: &str| {
            app.money_parse(MoneyInput {
                amount: amount.into(),
                currency: currency.into(),
            })
        };
        assert_eq!(parse("€10", "CLP").unwrap_err().code, ErrorCode::Validation);
        assert_eq!(parse("$10", "CLP").unwrap().minor, "10");
        assert_eq!(parse("US$10", "USD").unwrap().minor, "1000");
        assert_eq!(parse("10", "CLP").unwrap().minor, "10");
        let eur = parse("€1.234,56", "EUR").unwrap();
        assert_eq!(
            (eur.minor.as_str(), eur.display.as_str()),
            ("123456", "€1.234,56")
        );
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

    // ── Seeding and category deletion (proposal steps 4 and 6) ──

    fn app_with_household(locale: &str) -> App {
        let app = App::open_in_memory().unwrap();
        app.store
            .connection()
            .execute(
                "INSERT INTO household (id, name, base_currency_code, locale, hlc, hlc_device) \
                 VALUES ('h', 'Casa', 'CLP', ?1, 1, 'dev-a')",
                [locale],
            )
            .unwrap();
        app
    }

    fn dump(app: &App, table: &str) -> Vec<String> {
        let mut stmt = app
            .store
            .connection()
            .prepare(&format!("SELECT * FROM {table} ORDER BY id"))
            .unwrap();
        let cols = stmt.column_count();
        stmt.query_map([], |r| {
            Ok((0..cols)
                .map(|i| format!("{:?}", r.get_ref(i).unwrap()))
                .collect::<Vec<_>>()
                .join("|"))
        })
        .unwrap()
        .map(Result::unwrap)
        .collect()
    }

    fn sid(key: &str) -> String {
        seed::seed_id(key)
    }

    #[test]
    fn seeding_needs_a_household() {
        let mut app = App::open_in_memory().unwrap();
        assert_eq!(
            app.seed_household_defaults().unwrap_err().code,
            ErrorCode::NotFound
        );
    }

    #[test]
    fn seeding_is_idempotent_and_identical_on_two_devices() {
        let mut a = app_with_household("es-CL");
        let first = a.seed_household_defaults().unwrap();
        assert_eq!(first.categories_inserted, 33);
        assert_eq!(first.accounts_inserted, 3);
        let again = a.seed_household_defaults().unwrap();
        assert_eq!((again.categories_inserted, again.accounts_inserted), (0, 0));

        let mut b = app_with_household("es-CL");
        b.seed_household_defaults().unwrap();
        assert_eq!(dump(&a, "category"), dump(&b, "category"));
        assert_eq!(dump(&a, "account"), dump(&b, "account"));
    }

    #[test]
    fn seeds_in_the_household_language() {
        let mut app = app_with_household("en");
        app.seed_household_defaults().unwrap();
        let name: String = app
            .store
            .connection()
            .query_row(
                "SELECT name FROM category WHERE seed_key = 'expense.food'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(name, "Food");
    }

    #[test]
    fn a_deleted_seed_category_is_not_resurrected() {
        let mut app = app_with_household("es-CL");
        app.seed_household_defaults().unwrap();
        let cinema = sid("expense.leisure.cinema");
        app.category_delete(&cinema, None).unwrap();
        app.seed_household_defaults().unwrap();
        let deleted: i64 = app
            .store
            .connection()
            .query_row(
                "SELECT deleted FROM category WHERE id = ?1",
                [&cinema],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(deleted, 1);
    }

    #[test]
    fn seeding_reuses_a_user_category_with_the_same_name() {
        let mut app = app_with_household("es-CL");
        app.store
            .connection()
            .execute(
                "INSERT INTO category (id, kind, name, name_key, hlc, hlc_device) \
                 VALUES ('mine', 'expense', 'COMIDA', 'comida', 1, 'dev-a')",
                [],
            )
            .unwrap();
        let report = app.seed_household_defaults().unwrap();
        assert_eq!(report.categories_kept_existing, 1);
        let under_mine: i64 = app
            .store
            .connection()
            .query_row(
                "SELECT count(*) FROM category WHERE parent_id = 'mine'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(under_mine, 3);
    }

    #[test]
    fn every_name_key_is_the_normalized_name() {
        let mut app = app_with_household("es-CL");
        app.seed_household_defaults().unwrap();
        let mut stmt = app
            .store
            .connection()
            .prepare("SELECT name, name_key FROM category")
            .unwrap();
        let rows: Vec<(String, String)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert!(!rows.is_empty());
        for (name, key) in rows {
            assert_eq!(key, mrk_domain::normalize_name(&name), "{name}");
        }
    }

    #[test]
    fn deleting_with_merge_moves_everything_and_sums_budgets() {
        let mut app = app_with_household("es-CL");
        app.seed_household_defaults().unwrap();
        let (supermarket, delivery) = (
            sid("expense.food.supermarket"),
            sid("expense.food.delivery"),
        );
        let savings = sid("account.savings");
        app.store
            .connection()
            .execute_batch(&format!(
                "INSERT INTO journal_entry (id, entry_date, hlc, hlc_device) VALUES ('e', '2026-11-02', 1, 'dev-a');
                 INSERT INTO posting (id, entry_id, leg, account_id, amount_minor, currency_code, status, hlc, hlc_device)
                     VALUES ('p1', 'e', 'account', '{savings}', -45000, 'CLP', 'cleared', 1, 'dev-a');
                 INSERT INTO posting (id, entry_id, leg, category_id, amount_minor, currency_code, hlc, hlc_device)
                     VALUES ('p2', 'e', 'category', '{supermarket}', 45000, 'CLP', 1, 'dev-a');
                 INSERT INTO category (id, parent_id, kind, name, name_key, hlc, hlc_device)
                     VALUES ('organic', '{supermarket}', 'expense', 'Orgánico', 'organico', 1, 'dev-a');
                 INSERT INTO budget_line (id, period, category_id, scope, amount_minor, currency_code, hlc, hlc_device) VALUES
                     ('b1', '2026-11', '{supermarket}', 'household', 300000, 'CLP', 1, 'dev-a'),
                     ('b2', '2026-12', '{supermarket}', 'household', 280000, 'CLP', 1, 'dev-a'),
                     ('b3', '2026-11', '{delivery}',    'household',  40000, 'CLP', 1, 'dev-a');"
            ))
            .unwrap();

        app.category_delete(&supermarket, Some(&delivery)).unwrap();

        let c = app.store.connection();
        let one = |sql: &str| -> String { c.query_row(sql, [], |r| r.get(0)).unwrap() };
        let num = |sql: &str| -> i64 { c.query_row(sql, [], |r| r.get(0)).unwrap() };
        assert_eq!(
            one("SELECT category_id FROM posting WHERE id = 'p2'"),
            delivery
        );
        assert_eq!(
            one("SELECT parent_id FROM category WHERE id = 'organic'"),
            delivery
        );
        assert_eq!(
            num("SELECT amount_minor FROM budget_line WHERE id = 'b3'"),
            340_000
        );
        assert_eq!(num("SELECT deleted FROM budget_line WHERE id = 'b1'"), 1);
        assert_eq!(
            one("SELECT category_id FROM budget_line WHERE id = 'b2'"),
            delivery
        );
        let merged: (i64, String, String) = c
            .query_row(
                "SELECT deleted, merged_into_id, hlc_device FROM category WHERE id = ?1",
                [&supermarket],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(
            merged,
            (
                1,
                delivery.clone(),
                app.store.local_device_id().unwrap().to_owned()
            )
        );
        assert_eq!(app.store.category_reference_count(&supermarket).unwrap(), 0);
        assert_eq!(num("SELECT count(*) FROM pragma_foreign_key_check"), 0);
    }

    /// #37 F3: references a merge would re-point make a category "used", even where no
    /// posting or budget line does.
    #[test]
    fn deleting_without_merge_counts_every_reference_a_merge_moves() {
        let mut app = app_with_household("es-CL");
        app.seed_household_defaults().unwrap();
        let cinema = sid("expense.leisure.cinema");
        app.store
            .connection()
            .execute(
                "INSERT INTO payee (id, name, default_category_id, hlc, hlc_device) \
                 VALUES ('py', 'Cine', ?1, 1, 'dev-a')",
                [&cinema],
            )
            .unwrap();
        let err = app.category_delete(&cinema, None).unwrap_err();
        assert_eq!(err.code, ErrorCode::Validation);
        app.category_delete(&cinema, Some(&sid("expense.leisure")))
            .unwrap();
        let moved: String = app
            .store
            .connection()
            .query_row(
                "SELECT default_category_id FROM payee WHERE id = 'py'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(moved, sid("expense.leisure"));
    }

    #[test]
    fn deleting_a_used_category_needs_a_merge_target_of_the_same_kind() {
        let mut app = app_with_household("es-CL");
        app.seed_household_defaults().unwrap();
        let food = sid("expense.food");
        let err = app.category_delete(&food, None).unwrap_err();
        assert_eq!(err.code, ErrorCode::Validation);
        let err = app
            .category_delete(&food, Some(&sid("income.salary")))
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::Validation);
        let err = app
            .category_delete(&food, Some(&sid("expense.food.delivery")))
            .unwrap_err();
        assert_eq!(
            err.code,
            ErrorCode::Validation,
            "merging into its own sub-category"
        );
        assert_eq!(
            app.category_delete("nope", None).unwrap_err().code,
            ErrorCode::NotFound
        );
    }
}
