//! `#[tauri::command]` functions called by the UI with `invoke("...")`.
//!
//! Commands are thin: they lock the [`Store`] and call `budget_core`. All
//! data rules, and the single write path, live in `budget_core`.

use std::sync::{Arc, Mutex};

use budget_core::accounts::{self, Account, AccountWithBalance, NewAccount};
use budget_core::categories::{self, Category, NewCategory};
use budget_core::imports::{self, ImportBatch, ImportProfile, ImportSummary, Preview};
use budget_core::transactions::{
    self, NewManualTransaction, Transaction, TransactionEdit, TransactionView,
};
use budget_core::{settings, Store};
use serde::Serialize;
use tauri::State;

/// Shared with the Sync node, which locks it once per message.
pub type AppStore = Arc<Mutex<Store>>;

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
            // The UI offers to rescan the QR code (ticket 15).
            budget_core::Error::Pairing(budget_core::sync::PairingError::PeerUnreachable {
                ..
            }) => "unreachable",
            budget_core::Error::Pairing(_)
            | budget_core::Error::Sync(_)
            | budget_core::Error::Net(_) => "sync",
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
    /// `std::env::consts::OS`: "windows", "android", ...
    platform: &'static str,
    /// The phone build: phone layout and the QR scanner.
    mobile: bool,
}

#[tauri::command]
pub fn app_info(store: State<'_, AppStore>) -> CmdResult<AppInfo> {
    let store = lock(&store)?;
    Ok(AppInfo {
        device_id: store.device_id().to_owned(),
        schema_version: store.schema_version()?,
        platform: std::env::consts::OS,
        mobile: cfg!(mobile),
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

// ----- Budgets (ticket 10) -----

#[tauri::command]
pub fn budgets_progress(
    store: State<'_, AppStore>,
    today: String,
) -> CmdResult<Vec<budget_core::budgets::BudgetProgress>> {
    budget_core::budgets::progress(&*lock(&store)?, &today).map_err(Into::into)
}

#[tauri::command]
pub fn budget_create(
    store: State<'_, AppStore>,
    budget: budget_core::budgets::NewBudget,
) -> CmdResult<budget_core::budgets::Budget> {
    budget_core::budgets::create(&mut *lock(&store)?, budget).map_err(Into::into)
}

#[tauri::command]
pub fn budget_update(
    store: State<'_, AppStore>,
    budget: budget_core::budgets::BudgetEdit,
) -> CmdResult<budget_core::budgets::Budget> {
    budget_core::budgets::update(&mut *lock(&store)?, budget).map_err(Into::into)
}

#[tauri::command]
pub fn budget_end(
    store: State<'_, AppStore>,
    id: String,
    last_day: String,
) -> CmdResult<budget_core::budgets::Budget> {
    budget_core::budgets::end(&mut *lock(&store)?, &id, &last_day).map_err(Into::into)
}

#[tauri::command]
pub fn budget_delete(store: State<'_, AppStore>, id: String) -> CmdResult<()> {
    budget_core::budgets::delete(&mut *lock(&store)?, &id).map_err(Into::into)
}

#[tauri::command]
pub fn savings_target_get(store: State<'_, AppStore>) -> CmdResult<i64> {
    budget_core::settings::savings_target_minor(&*lock(&store)?).map_err(Into::into)
}

#[tauri::command]
pub fn savings_target_set(store: State<'_, AppStore>, minor: i64) -> CmdResult<()> {
    budget_core::settings::set_savings_target(&mut *lock(&store)?, minor).map_err(Into::into)
}

/// Spendable for `period` (`YYYY-MM`). `today` (`YYYY-MM-DD`) is the UI's
/// local date, so the Period and Week follow the user's clock.
#[tauri::command]
pub fn spendable_get(
    store: State<'_, AppStore>,
    period: String,
    today: String,
) -> CmdResult<budget_core::spendable::PeriodSpendable> {
    let period = period.parse()?;
    let today = today.parse()?;
    budget_core::spendable::for_period(&*lock(&store)?, period, today).map_err(Into::into)
}

#[tauri::command]
pub fn import_profile_get(
    store: State<'_, AppStore>,
    account: String,
) -> CmdResult<Option<ImportProfile>> {
    imports::profile_for_account(&*lock(&store)?, &account).map_err(Into::into)
}

/// A new, unsaved profile for an Account, to start the setup step from.
#[tauri::command]
pub fn import_profile_draft(account: String) -> ImportProfile {
    ImportProfile::draft(&account)
}

#[tauri::command]
pub fn import_profile_save(
    store: State<'_, AppStore>,
    profile: ImportProfile,
) -> CmdResult<ImportProfile> {
    imports::save_profile(&mut *lock(&store)?, profile).map_err(Into::into)
}

/// `file` is the CSV's bytes. Nothing is saved.
#[tauri::command]
pub fn import_preview(
    store: State<'_, AppStore>,
    profile: ImportProfile,
    file: Vec<u8>,
) -> CmdResult<Preview> {
    imports::preview(&*lock(&store)?, &profile, &file).map_err(Into::into)
}

/// Import the CSV's bytes with the Account's saved profile. The file itself
/// is not kept.
#[tauri::command]
pub fn import_run(
    store: State<'_, AppStore>,
    account: String,
    file_name: String,
    file: Vec<u8>,
) -> CmdResult<ImportSummary> {
    imports::import_file(&mut *lock(&store)?, &account, &file_name, &file).map_err(Into::into)
}

#[tauri::command]
pub fn imports_history(store: State<'_, AppStore>, account: String) -> CmdResult<Vec<ImportBatch>> {
    imports::history(&*lock(&store)?, &account).map_err(Into::into)
}

// ----- Planned items and Set-asides (ticket 08) -----

#[tauri::command]
pub fn planned_items_list(
    store: State<'_, AppStore>,
) -> CmdResult<Vec<budget_core::planned::PlannedItem>> {
    budget_core::planned::list(&*lock(&store)?).map_err(Into::into)
}

#[tauri::command]
pub fn planned_item_create(
    store: State<'_, AppStore>,
    item: budget_core::planned::NewPlannedItem,
) -> CmdResult<budget_core::planned::PlannedItem> {
    budget_core::planned::create(&mut *lock(&store)?, item).map_err(Into::into)
}

#[tauri::command]
pub fn planned_item_update(
    store: State<'_, AppStore>,
    item: budget_core::planned::PlannedItem,
) -> CmdResult<budget_core::planned::PlannedItem> {
    budget_core::planned::update(&mut *lock(&store)?, item).map_err(Into::into)
}

/// `last_day` (`YYYY-MM-DD`): the last day the item can occur.
#[tauri::command]
pub fn planned_item_end(
    store: State<'_, AppStore>,
    id: String,
    last_day: String,
) -> CmdResult<budget_core::planned::PlannedItem> {
    let last_day = last_day.parse()?;
    budget_core::planned::end(&mut *lock(&store)?, &id, last_day).map_err(Into::into)
}

#[tauri::command]
pub fn planned_item_delete(store: State<'_, AppStore>, id: String) -> CmdResult<()> {
    budget_core::planned::delete(&mut *lock(&store)?, &id).map_err(Into::into)
}

/// The expected occurrences and Set-asides of `period` (`YYYY-MM`).
#[tauri::command]
pub fn planned_period_get(
    store: State<'_, AppStore>,
    period: String,
) -> CmdResult<budget_core::planned::PeriodPlan> {
    let period = period.parse()?;
    budget_core::planned::for_period(&*lock(&store)?, period).map_err(Into::into)
}

#[tauri::command]
pub fn rules_list(store: State<'_, AppStore>) -> CmdResult<Vec<budget_core::rules::RuleView>> {
    budget_core::rules::list(&*lock(&store)?).map_err(Into::into)
}

#[tauri::command]
pub fn rule_create(
    store: State<'_, AppStore>,
    rule: budget_core::rules::NewRule,
) -> CmdResult<budget_core::rules::Rule> {
    budget_core::rules::create(&mut *lock(&store)?, rule).map_err(Into::into)
}

#[tauri::command]
pub fn rule_update(
    store: State<'_, AppStore>,
    rule: budget_core::rules::RuleEdit,
) -> CmdResult<budget_core::rules::Rule> {
    budget_core::rules::update(&mut *lock(&store)?, rule).map_err(Into::into)
}

#[tauri::command]
pub fn rule_delete(store: State<'_, AppStore>, id: String) -> CmdResult<()> {
    budget_core::rules::delete(&mut *lock(&store)?, &id).map_err(Into::into)
}

#[tauri::command]
pub fn rule_move(
    store: State<'_, AppStore>,
    id: String,
    index: usize,
) -> CmdResult<budget_core::rules::Rule> {
    budget_core::rules::move_to(&mut *lock(&store)?, &id, index).map_err(Into::into)
}

/// The existing Transactions the Rule would re-categorise. Nothing is changed.
#[tauri::command]
pub fn rule_recategorise_preview(
    store: State<'_, AppStore>,
    id: String,
) -> CmdResult<Vec<TransactionView>> {
    budget_core::rules::recategorise_preview(&*lock(&store)?, &id).map_err(Into::into)
}

/// Apply the Rule to the Transactions the user confirmed.
#[tauri::command]
pub fn rule_recategorise(
    store: State<'_, AppStore>,
    id: String,
    transactions: Vec<String>,
) -> CmdResult<usize> {
    budget_core::rules::recategorise(&mut *lock(&store)?, &id, &transactions).map_err(Into::into)
}

/// The review list as of `today` (`YYYY-MM-DD`, the UI's local date), with
/// the count for the "to review" badge.
#[tauri::command]
pub fn review_list(
    store: State<'_, AppStore>,
    today: String,
) -> CmdResult<budget_core::review::ReviewList> {
    let today = today.parse()?;
    budget_core::review::list(&*lock(&store)?, today).map_err(Into::into)
}

/// Every Transfer that isn't rejected, oldest first.
#[tauri::command]
pub fn transfers_list(
    store: State<'_, AppStore>,
) -> CmdResult<Vec<budget_core::transfers::TransferView>> {
    budget_core::transfers::list(&*lock(&store)?).map_err(Into::into)
}

/// Link two Transactions as a Transfer by hand.
#[tauri::command]
pub fn transfer_link(
    store: State<'_, AppStore>,
    a: String,
    b: String,
) -> CmdResult<budget_core::transfers::Transfer> {
    budget_core::transfers::link(&mut *lock(&store)?, &a, &b).map_err(Into::into)
}

/// Confirm a suggested Transfer.
#[tauri::command]
pub fn transfer_confirm(
    store: State<'_, AppStore>,
    id: String,
) -> CmdResult<budget_core::transfers::Transfer> {
    budget_core::transfers::confirm(&mut *lock(&store)?, &id).map_err(Into::into)
}

/// Reject a suggested Transfer, or unlink a confirmed one.
#[tauri::command]
pub fn transfer_reject(
    store: State<'_, AppStore>,
    id: String,
) -> CmdResult<budget_core::transfers::Transfer> {
    budget_core::transfers::reject(&mut *lock(&store)?, &id).map_err(Into::into)
}

/// The Savings target next to what Transfers moved into savings Accounts
/// in `period` (`YYYY-MM`).
#[tauri::command]
pub fn savings_progress_get(
    store: State<'_, AppStore>,
    period: String,
) -> CmdResult<budget_core::transfers::SavingsProgress> {
    let period = period.parse()?;
    budget_core::transfers::savings_progress(&*lock(&store)?, period).map_err(Into::into)
}

/// Confirm a suggested match: the Manual Transaction merges into the
/// imported one.
#[tauri::command]
pub fn manual_match_confirm(
    store: State<'_, AppStore>,
    id: String,
) -> CmdResult<budget_core::manual_matches::ManualMatch> {
    budget_core::manual_matches::confirm(&mut *lock(&store)?, &id).map_err(Into::into)
}

/// Reject a suggested match: both Transactions stay.
#[tauri::command]
pub fn manual_match_reject(
    store: State<'_, AppStore>,
    id: String,
) -> CmdResult<budget_core::manual_matches::ManualMatch> {
    budget_core::manual_matches::reject(&mut *lock(&store)?, &id).map_err(Into::into)
}
