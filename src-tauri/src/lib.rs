//! Tauri shell: opens the database in the app data folder and exposes the
//! commands. Same code for the Windows and Android builds; the Android build
//! adds the QR code scanner (`#[cfg(mobile)]`).

mod backup_commands;
mod commands;
mod firewall;
mod sync_commands;

use std::sync::{Arc, Mutex};

use budget_core::Store;
use tauri::Manager;

/// The database file inside the per-user app data folder
/// (`%APPDATA%\app.budgeting.personal\` on Windows).
const DB_FILE: &str = "budget.sqlite3";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default().plugin(tauri_plugin_dialog::init());
    // The phone scans the PC's pairing QR code with its camera.
    #[cfg(mobile)]
    let builder = builder.plugin(tauri_plugin_barcode_scanner::init());
    builder
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
            app.manage(backup_commands::ScheduleStatus::default());
            backup_commands::start_weekly(app.handle());
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
            commands::rules_list,
            commands::rule_create,
            commands::rule_update,
            commands::rule_delete,
            commands::rule_move,
            commands::rule_recategorise_preview,
            commands::rule_recategorise,
            backup_commands::backup_export,
            backup_commands::backup_restore,
            backup_commands::backup_schedule_get,
            backup_commands::backup_schedule_set,
            sync_commands::sync_status,
            sync_commands::sync_server_set,
            sync_commands::pairing_start,
            sync_commands::pairing_cancel,
            sync_commands::pairing_join,
            sync_commands::peers_list,
            sync_commands::peer_remove,
            sync_commands::sync_now,
            sync_commands::sync_auto,
            commands::planned_items_list,
            commands::planned_item_create,
            commands::planned_item_update,
            commands::planned_item_end,
            commands::planned_item_delete,
            commands::planned_period_get,
            commands::review_list,
            commands::transfers_list,
            commands::transfer_link,
            commands::transfer_confirm,
            commands::transfer_reject,
            commands::savings_progress_get,
        ])
        .run(tauri::generate_context!())
        .expect("error while running the app");
}
