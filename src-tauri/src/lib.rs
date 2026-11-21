//! Tauri shell: opens the database in the app data folder and exposes the
//! commands. Same code for the Windows and (later) Android builds.

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
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            // Creates the file and runs migrations on first launch; also
            // gives this Device its permanent device id.
            let store = Store::open(dir.join(DB_FILE))?;
            app.manage::<commands::AppStore>(Mutex::new(store));
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
            commands::categories_list,
            commands::category_create,
            commands::category_rename,
            commands::category_move,
            commands::category_delete,
            commands::transactions_list,
            commands::transactions_uncategorised,
            commands::transaction_create,
            commands::transaction_update,
            commands::transaction_set_category,
            commands::transaction_delete,
            commands::budgets_progress,
            commands::budget_create,
            commands::budget_update,
            commands::budget_end,
            commands::budget_delete,
            commands::savings_target_get,
            commands::savings_target_set,
            commands::spendable_get,
            commands::import_profile_get,
            commands::import_profile_draft,
            commands::import_profile_save,
            commands::import_preview,
            commands::import_run,
            commands::imports_history,
        ])
        .run(tauri::generate_context!())
        .expect("error while running the app");
}
