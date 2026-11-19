//! Tauri shell: windowing, OS paths and command wiring only (ADR-0002).
//! Every command deserializes its input, delegates to `mr_krabs_core`, and returns.

use std::sync::Mutex;

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

#[tauri::command]
fn app_health(core: State<'_, Core>) -> Result<HealthDto, ApiError> {
    with_core(&core, App::health)
}

#[tauri::command]
fn money_parse(core: State<'_, Core>, input: MoneyInput) -> Result<MoneyDto, ApiError> {
    with_core(&core, |app| app.money_parse(input))
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
        .invoke_handler(tauri::generate_handler![app_health, money_parse])
        .run(tauri::generate_context!())
        .expect("error while running Mr Krabs");
}
