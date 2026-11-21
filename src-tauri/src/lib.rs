//! Tauri shell: opens the database in the app data folder and exposes the
//! commands. Same code for the Windows and (later) Android builds.

mod commands;
mod sync_commands;

use std::sync::{Arc, Mutex};

use budget_core::Store;
use tauri::Manager;

/// The database file inside the per-user app data folder
/// (`%APPDATA%\app.budgeting.personal\` on Windows).
const DB_FILE: &str = "budget.sqlite3";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // BUDGET_DATA_DIR runs a second instance with its own data
            // folder, e.g. to try Pairing between two instances on one PC.
            let dir = match std::env::var_os("BUDGET_DATA_DIR") {
                Some(dir) => dir.into(),
                None => app.path().app_data_dir()?,
            };
            std::fs::create_dir_all(&dir)?;
            // Creates the file and runs migrations on first launch; also
            // gives this Device its permanent device id.
            let store = Store::open(dir.join(DB_FILE))?;
            let store: commands::AppStore = Arc::new(Mutex::new(store));
            app.manage(store.clone());
            app.manage(sync_commands::init(app.handle(), store, dir)?);
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
            sync_commands::sync_status,
            sync_commands::sync_server_set,
            sync_commands::pairing_start,
            sync_commands::pairing_cancel,
            sync_commands::pairing_join,
            sync_commands::peers_list,
            sync_commands::peer_remove,
            sync_commands::sync_now,
        ])
        .run(tauri::generate_context!())
        .expect("error while running the app");
}
