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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let core = App::open(&mr_krabs_core::default_db_file(&data_dir))?;
            app.manage(Mutex::new(core));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![app_health, money_parse])
        .run(tauri::generate_context!())
        .expect("error while running Mr Krabs");
}
