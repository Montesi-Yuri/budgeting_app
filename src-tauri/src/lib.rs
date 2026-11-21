//! Tauri shell: opens the database in the app data folder and exposes the
//! commands. Same code for the Windows and (later) Android builds.

mod backup_commands;
mod commands;

use std::sync::Mutex;

use budget_core::Store;
use tauri::Manager;

/// The database file inside the per-user app data folder
/// (`%APPDATA%\app.budgeting.personal\` on Windows).
const DB_FILE: &str = "budget.sqlite3";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            // Creates the file and runs migrations on first launch; also
            // gives this Device its permanent device id.
            let store = Store::open(dir.join(DB_FILE))?;
            app.manage::<commands::AppStore>(Mutex::new(store));
            app.manage(backup_commands::ScheduleStatus::default());
            backup_commands::start_weekly(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::currency_get,
            commands::currency_set,
            commands::accounts_list,
            commands::account_create,
            commands::account_update,
            commands::account_delete,
            backup_commands::backup_export,
            backup_commands::backup_restore,
            backup_commands::backup_schedule_get,
            backup_commands::backup_schedule_set,
        ])
        .run(tauri::generate_context!())
        .expect("error while running the app");
}
