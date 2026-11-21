//! `#[tauri::command]` functions called by the UI with `invoke("...")`.
//!
//! Commands are thin: they lock the [`Store`] and call `budget_core`. All
//! data rules, and the single write path, live in `budget_core`.

use std::sync::Mutex;

use budget_core::accounts::{self, Account, AccountWithBalance, NewAccount};
use budget_core::{settings, Store};
use serde::Serialize;
use tauri::State;

pub type AppStore = Mutex<Store>;

/// The error the UI receives. `kind` lets it tell the user's mistakes
/// ("invalid") apart from failures.
#[derive(Debug, Serialize)]
pub struct CommandError {
    kind: &'static str,
    pub(crate) message: String,
}

impl From<budget_core::Error> for CommandError {
    fn from(err: budget_core::Error) -> Self {
        let kind = match err {
            budget_core::Error::Invalid(_)
            | budget_core::Error::WrongPassphrase
            | budget_core::Error::NotABackup(_) => "invalid",
            budget_core::Error::NotFound { .. } => "not_found",
            _ => "internal",
        };
        CommandError {
            kind,
            message: err.to_string(),
        }
    }
}

impl CommandError {
    pub(crate) fn new(kind: &'static str, message: impl Into<String>) -> Self {
        CommandError {
            kind,
            message: message.into(),
        }
    }
}

pub(crate) type CmdResult<T> = Result<T, CommandError>;

pub(crate) fn lock<'a>(
    store: &'a State<'_, AppStore>,
) -> CmdResult<std::sync::MutexGuard<'a, Store>> {
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
