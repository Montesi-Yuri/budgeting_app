//! Backup commands, and the weekly automatic Backup on the PC.
//!
//! The logic is in `budget_core::backup`. This shell adds what needs the OS:
//! the passphrase of the weekly Backup lives in the Windows credential store
//! (so no prompt is needed), and a background thread runs the weekly Backup
//! on app start and then every hour while the app is running.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use budget_core::backup::{self, RestoreReport};
use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::commands::{lock, AppStore, CmdResult, CommandError};

/// The last error of the weekly Backup, shown in Settings.
#[derive(Default)]
pub struct ScheduleStatus(Mutex<Option<String>>);

#[tauri::command]
pub fn backup_export(
    store: State<'_, AppStore>,
    path: String,
    passphrase: String,
) -> CmdResult<()> {
    backup::export(&*lock(&store)?, Path::new(&path), &passphrase).map_err(Into::into)
}

#[tauri::command]
pub fn backup_restore(
    store: State<'_, AppStore>,
    path: String,
    passphrase: String,
) -> CmdResult<RestoreReport> {
    backup::restore(&mut *lock(&store)?, Path::new(&path), &passphrase).map_err(Into::into)
}

#[derive(Debug, Serialize)]
pub struct BackupSchedule {
    /// The weekly Backup exists only on the PC.
    available: bool,
    folder: Option<String>,
    /// The file name of the newest weekly Backup in the folder.
    latest: Option<String>,
    last_error: Option<String>,
}

#[tauri::command]
pub fn backup_schedule_get(
    store: State<'_, AppStore>,
    status: State<'_, ScheduleStatus>,
) -> CmdResult<BackupSchedule> {
    let folder = backup::schedule_folder(&*lock(&store)?)?;
    schedule_info(folder, &status)
}

/// Choose the weekly Backup folder and its passphrase (saved in the
/// credential store), and write this week's Backup now if it is missing.
/// `folder: None` turns the weekly Backup off and forgets the passphrase.
#[tauri::command]
pub fn backup_schedule_set(
    store: State<'_, AppStore>,
    status: State<'_, ScheduleStatus>,
    folder: Option<String>,
    passphrase: Option<String>,
) -> CmdResult<BackupSchedule> {
    if !credentials::AVAILABLE {
        return Err(CommandError::new(
            "invalid",
            "The weekly Backup is only available on the PC.",
        ));
    }
    let mut store = lock(&store)?;
    match folder {
        None => {
            backup::set_schedule_folder(&mut store, None)?;
            credentials::forget()?;
            set_status(&status, None);
            schedule_info(None, &status)
        }
        Some(folder) => {
            let passphrase = passphrase.unwrap_or_default();
            if passphrase.is_empty() {
                return Err(CommandError::new(
                    "invalid",
                    "The weekly Backup needs a passphrase.",
                ));
            }
            let folder = PathBuf::from(folder);
            if !folder.is_dir() {
                return Err(CommandError::new("invalid", "That folder does not exist."));
            }
            credentials::save(&passphrase)?;
            backup::set_schedule_folder(&mut store, Some(&folder))?;
            let result = backup::run_weekly(&store, &folder, &passphrase, &backup::today_utc());
            set_status(&status, result.as_ref().err().map(|e| e.to_string()));
            result?;
            schedule_info(Some(folder), &status)
        }
    }
}

fn schedule_info(folder: Option<PathBuf>, status: &ScheduleStatus) -> CmdResult<BackupSchedule> {
    let latest = match &folder {
        Some(folder) => backup::latest_weekly(folder)
            .ok()
            .flatten()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned())),
        None => None,
    };
    Ok(BackupSchedule {
        available: credentials::AVAILABLE,
        folder: folder.map(|f| f.to_string_lossy().into_owned()),
        latest,
        last_error: status.0.lock().ok().and_then(|s| s.clone()),
    })
}

fn set_status(status: &ScheduleStatus, error: Option<String>) {
    if let Ok(mut slot) = status.0.lock() {
        *slot = error;
    }
}

/// Start the weekly Backup thread (PC only): it checks on app start, then
/// every hour, and writes a Backup when the newest one is a week old.
pub fn start_weekly(app: &AppHandle) {
    if !credentials::AVAILABLE {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || loop {
        run_weekly_now(&app);
        std::thread::sleep(std::time::Duration::from_secs(60 * 60));
    });
}

fn run_weekly_now(app: &AppHandle) {
    let store = app.state::<AppStore>();
    let status = app.state::<ScheduleStatus>();
    let Ok(store) = store.lock() else { return };
    let folder = match backup::schedule_folder(&store) {
        Ok(Some(folder)) => folder,
        _ => return,
    };
    let result =
        credentials::load()
            .map_err(|e| e.message)
            .and_then(|passphrase| match passphrase {
                Some(p) => backup::run_weekly(&store, &folder, &p, &backup::today_utc())
                    .map(|_| ())
                    .map_err(|e| e.to_string()),
                None => Err(
                    "The passphrase for the weekly Backup is missing. Choose the folder again."
                        .into(),
                ),
            });
    set_status(&status, result.err());
}

/// The weekly Backup's passphrase in the Windows credential store.
#[cfg(windows)]
mod credentials {
    use crate::commands::{CmdResult, CommandError};

    pub const AVAILABLE: bool = true;
    const SERVICE: &str = "app.budgeting.personal";
    const USER: &str = "weekly-backup-passphrase";

    fn entry() -> CmdResult<keyring::Entry> {
        keyring::Entry::new(SERVICE, USER).map_err(failed)
    }

    fn failed(err: keyring::Error) -> CommandError {
        CommandError::new("internal", format!("Windows credential store: {err}"))
    }

    pub fn save(passphrase: &str) -> CmdResult<()> {
        entry()?.set_password(passphrase).map_err(failed)
    }

    pub fn load() -> CmdResult<Option<String>> {
        match entry()?.get_password() {
            Ok(p) => Ok(Some(p)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(err) => Err(failed(err)),
        }
    }

    pub fn forget() -> CmdResult<()> {
        match entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(failed(err)),
        }
    }
}

/// Not on the PC (the phone): there is no weekly Backup.
#[cfg(not(windows))]
mod credentials {
    use crate::commands::CmdResult;

    pub const AVAILABLE: bool = false;

    pub fn save(_: &str) -> CmdResult<()> {
        Ok(())
    }

    pub fn load() -> CmdResult<Option<String>> {
        Ok(None)
    }

    pub fn forget() -> CmdResult<()> {
        Ok(())
    }
}
