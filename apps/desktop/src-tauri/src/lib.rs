//! Tauri shell: windowing, OS paths and command wiring only (ADR-0002).
//! Every command deserializes its input, delegates to `mr_krabs_core`, and returns.

use std::sync::Mutex;

use mr_krabs_core::api::{
    AccountDetailDto, AccountDto, AccountInput, AccountListQuery, AccountSummaryDto, BootstrapDto,
    CategoryNodeDto, CategoryTreeQuery, HouseholdDto, HouseholdInput, NewTxInput, Page,
    SettingsDto, TxDetailDto, TxQuery, TxRowDto,
};
use mr_krabs_core::{ApiError, App, HealthDto, MoneyDto, MoneyInput};
use tauri::{Manager, State};

type Core = Mutex<App>;

fn with_core<T>(
    core: &State<'_, Core>,
    f: impl FnOnce(&App) -> Result<T, ApiError>,
) -> Result<T, ApiError> {
    let app = core
        .lock()
        .map_err(|_| ApiError::internal("core lock poisoned"))?;
    f(&app)
}

fn with_core_mut<T>(
    core: &State<'_, Core>,
    f: impl FnOnce(&mut App) -> Result<T, ApiError>,
) -> Result<T, ApiError> {
    let mut app = core
        .lock()
        .map_err(|_| ApiError::internal("core lock poisoned"))?;
    f(&mut app)
}

#[tauri::command]
fn app_health(core: State<'_, Core>) -> Result<HealthDto, ApiError> {
    with_core(&core, App::health)
}

#[tauri::command]
fn money_parse(core: State<'_, Core>, input: MoneyInput) -> Result<MoneyDto, ApiError> {
    with_core(&core, |app| app.money_parse(input))
}

#[tauri::command]
fn app_bootstrap(core: State<'_, Core>) -> Result<BootstrapDto, ApiError> {
    with_core(&core, App::bootstrap)
}

#[tauri::command]
fn household_create(
    core: State<'_, Core>,
    input: HouseholdInput,
) -> Result<HouseholdDto, ApiError> {
    with_core_mut(&core, |app| app.household_create(input))
}

#[tauri::command]
fn settings_get(core: State<'_, Core>) -> Result<SettingsDto, ApiError> {
    with_core(&core, App::settings_get)
}

#[tauri::command]
fn account_list(
    core: State<'_, Core>,
    query: AccountListQuery,
) -> Result<Vec<AccountSummaryDto>, ApiError> {
    with_core(&core, |app| app.account_list(query))
}

#[tauri::command]
fn account_get(core: State<'_, Core>, id: String) -> Result<AccountDetailDto, ApiError> {
    with_core(&core, |app| app.account_get(id))
}

#[tauri::command]
fn account_create(core: State<'_, Core>, input: AccountInput) -> Result<AccountDto, ApiError> {
    with_core_mut(&core, |app| app.account_create(input))
}

#[tauri::command]
fn account_update(
    core: State<'_, Core>,
    id: String,
    input: AccountInput,
) -> Result<AccountDto, ApiError> {
    with_core_mut(&core, |app| app.account_update(id, input))
}

#[tauri::command]
fn account_archive(core: State<'_, Core>, id: String) -> Result<AccountDto, ApiError> {
    with_core_mut(&core, |app| app.account_archive(id))
}

#[tauri::command]
fn category_tree(
    core: State<'_, Core>,
    query: CategoryTreeQuery,
) -> Result<Vec<CategoryNodeDto>, ApiError> {
    with_core(&core, |app| app.category_tree(query))
}

#[tauri::command]
fn tx_create(core: State<'_, Core>, input: NewTxInput) -> Result<TxDetailDto, ApiError> {
    with_core_mut(&core, |app| app.tx_create(input))
}

#[tauri::command]
fn tx_get(core: State<'_, Core>, id: String) -> Result<TxDetailDto, ApiError> {
    with_core(&core, |app| app.tx_get(id))
}

#[tauri::command]
fn tx_list(core: State<'_, Core>, query: TxQuery) -> Result<Page<TxRowDto>, ApiError> {
    with_core(&core, |app| app.tx_list(query))
}

#[tauri::command]
fn tx_update(
    core: State<'_, Core>,
    id: String,
    input: NewTxInput,
) -> Result<TxDetailDto, ApiError> {
    with_core_mut(&core, |app| app.tx_update(id, input))
}

#[tauri::command]
fn tx_delete(core: State<'_, Core>, id: String) -> Result<(), ApiError> {
    with_core_mut(&core, |app| app.tx_delete(id))
}

#[tauri::command]
fn tx_restore(core: State<'_, Core>, id: String) -> Result<(), ApiError> {
    with_core_mut(&core, |app| app.tx_restore(id))
}

/// Daily-rotated local log file; no remote sink (docs/02-architecture.md). `RUST_LOG`
/// overrides the default level. Never log amounts or descriptions at `info`.
/// A logging failure must not stop the app from opening, so it is reported and skipped.
fn init_logging(dir: &std::path::Path) {
    use tracing_appender::rolling::{RollingFileAppender, Rotation};

    let appender = match RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("mrkrabs")
        .filename_suffix("log")
        .max_log_files(14)
        .build(dir)
    {
        Ok(a) => a,
        Err(e) => {
            eprintln!("file logging disabled: {e}");
            return;
        }
    };
    let filter =
        tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into());
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_ansi(false)
        .with_writer(appender)
        .try_init();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            init_logging(&app.path().app_log_dir()?);
            let data_dir = app.path().app_data_dir()?;
            let core = App::open(&mr_krabs_core::default_db_file(&data_dir))?;
            tracing::info!(version = env!("CARGO_PKG_VERSION"), "mr krabs started");
            app.manage(Mutex::new(core));
            Ok(())
        })
        // Keep in step with mr_krabs_core::commands::COMMANDS.
        .invoke_handler(tauri::generate_handler![
            app_health,
            app_bootstrap,
            household_create,
            settings_get,
            money_parse,
            account_list,
            account_get,
            account_create,
            account_update,
            account_archive,
            category_tree,
            tx_create,
            tx_get,
            tx_list,
            tx_update,
            tx_delete,
            tx_restore,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Mr Krabs");
}
