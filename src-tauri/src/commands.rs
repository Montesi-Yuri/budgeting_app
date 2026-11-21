//! `#[tauri::command]` functions called by the UI with `invoke("...")`.
//!
//! Commands are thin: they lock the [`Store`] and call `budget_core`. All
//! data rules, and the single write path, live in `budget_core`.

use std::sync::Mutex;

use budget_core::accounts::{self, Account, AccountWithBalance, NewAccount};
use budget_core::categories::{self, Category, NewCategory};
use budget_core::transactions::{
    self, NewManualTransaction, Transaction, TransactionEdit, TransactionView,
};
use budget_core::{settings, Store};
use serde::Serialize;
use tauri::State;

pub type AppStore = Mutex<Store>;

/// The error the UI receives. `kind` lets it tell the user's mistakes
/// ("invalid") apart from failures.
#[derive(Debug, Serialize)]
pub struct CommandError {
    kind: &'static str,
    message: String,
}

impl From<budget_core::Error> for CommandError {
    fn from(err: budget_core::Error) -> Self {
        let kind = match err {
            budget_core::Error::Invalid(_) => "invalid",
            budget_core::Error::NotFound { .. } => "not_found",
            _ => "internal",
        };
        CommandError {
            kind,
            message: err.to_string(),
        }
    }
}

type CmdResult<T> = Result<T, CommandError>;

fn lock<'a>(store: &'a State<'_, AppStore>) -> CmdResult<std::sync::MutexGuard<'a, Store>> {
    store.lock().map_err(|_| CommandError {
        kind: "internal",
        message: "the database is unavailable".into(),
    })
}

#[derive(Debug, Serialize)]
pub struct AppInfo {
    device_id: String,
    schema_version: u32,
}

#[tauri::command]
pub fn app_info(store: State<'_, AppStore>) -> CmdResult<AppInfo> {
    let store = lock(&store)?;
    Ok(AppInfo {
        device_id: store.device_id().to_owned(),
        schema_version: store.schema_version()?,
    })
}

#[tauri::command]
pub fn currency_get(store: State<'_, AppStore>) -> CmdResult<Option<String>> {
    settings::currency(&*lock(&store)?).map_err(Into::into)
}

#[tauri::command]
pub fn currency_set(store: State<'_, AppStore>, code: String) -> CmdResult<()> {
    settings::set_currency(&mut *lock(&store)?, &code).map_err(Into::into)
}

#[tauri::command]
pub fn accounts_list(store: State<'_, AppStore>) -> CmdResult<Vec<AccountWithBalance>> {
    accounts::list(&*lock(&store)?).map_err(Into::into)
}

#[tauri::command]
pub fn account_create(store: State<'_, AppStore>, account: NewAccount) -> CmdResult<Account> {
    accounts::create(&mut *lock(&store)?, account).map_err(Into::into)
}

#[tauri::command]
pub fn account_update(store: State<'_, AppStore>, account: Account) -> CmdResult<Account> {
    accounts::update(&mut *lock(&store)?, account).map_err(Into::into)
}

#[tauri::command]
pub fn account_delete(store: State<'_, AppStore>, id: String) -> CmdResult<()> {
    accounts::delete(&mut *lock(&store)?, &id).map_err(Into::into)
}

#[tauri::command]
pub fn categories_list(store: State<'_, AppStore>) -> CmdResult<Vec<Category>> {
    categories::list(&*lock(&store)?).map_err(Into::into)
}

#[tauri::command]
pub fn category_create(store: State<'_, AppStore>, category: NewCategory) -> CmdResult<Category> {
    categories::create(&mut *lock(&store)?, category).map_err(Into::into)
}

#[tauri::command]
pub fn category_rename(
    store: State<'_, AppStore>,
    id: String,
    name: String,
) -> CmdResult<Category> {
    categories::rename(&mut *lock(&store)?, &id, &name).map_err(Into::into)
}

#[tauri::command]
pub fn category_move(store: State<'_, AppStore>, id: String, index: usize) -> CmdResult<Category> {
    categories::move_to(&mut *lock(&store)?, &id, index).map_err(Into::into)
}

#[tauri::command]
pub fn category_delete(store: State<'_, AppStore>, id: String) -> CmdResult<()> {
    categories::delete(&mut *lock(&store)?, &id).map_err(Into::into)
}

#[tauri::command]
pub fn transactions_list(
    store: State<'_, AppStore>,
    account: String,
) -> CmdResult<Vec<TransactionView>> {
    transactions::list_for_account(&*lock(&store)?, &account).map_err(Into::into)
}

#[tauri::command]
pub fn transactions_uncategorised(store: State<'_, AppStore>) -> CmdResult<Vec<TransactionView>> {
    transactions::list_uncategorised(&*lock(&store)?).map_err(Into::into)
}

#[tauri::command]
pub fn transaction_create(
    store: State<'_, AppStore>,
    transaction: NewManualTransaction,
) -> CmdResult<Transaction> {
    transactions::create_manual(&mut *lock(&store)?, transaction).map_err(Into::into)
}

#[tauri::command]
pub fn transaction_update(
    store: State<'_, AppStore>,
    transaction: TransactionEdit,
) -> CmdResult<Transaction> {
    transactions::update(&mut *lock(&store)?, transaction).map_err(Into::into)
}

#[tauri::command]
pub fn transaction_set_category(
    store: State<'_, AppStore>,
    id: String,
    category: Option<String>,
) -> CmdResult<Transaction> {
    transactions::set_category(&mut *lock(&store)?, &id, category.as_deref()).map_err(Into::into)
}

#[tauri::command]
pub fn transaction_delete(store: State<'_, AppStore>, id: String) -> CmdResult<()> {
    transactions::delete(&mut *lock(&store)?, &id).map_err(Into::into)
}
