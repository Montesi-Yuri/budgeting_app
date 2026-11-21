//! The weekly automatic Backup on the PC: a folder chosen per Device, one
//! Backup a week, the last 8 kept (sync-design.md §4).

use std::path::Path;

use budget_core::accounts::{self, AccountKind, NewAccount};
use budget_core::{backup, Store};

fn store_with_an_account() -> Store {
    let mut store = Store::open_in_memory().unwrap();
    accounts::create(
        &mut store,
        NewAccount {
            name: "Checking".into(),
            kind: AccountKind::Current,
            opening_minor: 100,
            opening_date: "2026-01-01".into(),
            is_main: true,
        },
    )
    .unwrap();
    store
}

fn files_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn the_backup_folder_is_a_setting_of_this_device_that_is_not_synced_or_backed_up() {
    let mut store = store_with_an_account();
    let dir = tempfile::tempdir().unwrap();
    let ops_before = store.version_vector().unwrap();
    assert_eq!(backup::schedule_folder(&store).unwrap(), None);

    backup::set_schedule_folder(&mut store, Some(dir.path())).unwrap();

    assert_eq!(
        backup::schedule_folder(&store).unwrap(),
        Some(dir.path().to_path_buf())
    );
    assert_eq!(
        store.version_vector().unwrap(),
        ops_before,
        "no change in the sync log"
    );
    let file = dir.path().join("manual.bsync.age");
    backup::export(&store, &file, "pass").unwrap();
    let mut fresh = Store::open_in_memory().unwrap();
    backup::restore(&mut fresh, &file, "pass").unwrap();
    assert_eq!(backup::schedule_folder(&fresh).unwrap(), None);

    backup::set_schedule_folder(&mut store, None).unwrap();
    assert_eq!(backup::schedule_folder(&store).unwrap(), None);
}

#[test]
fn a_backup_is_written_once_a_week() {
    let store = store_with_an_account();
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(backup::latest_weekly(dir.path()).unwrap(), None);

    let first = backup::run_weekly(&store, dir.path(), "pass", "2026-11-02").unwrap();
    let same_week = backup::run_weekly(&store, dir.path(), "pass", "2026-11-08").unwrap();
    let next_week = backup::run_weekly(&store, dir.path(), "pass", "2026-11-09").unwrap();

    assert_eq!(first, Some(dir.path().join("budget-2026-11-02.bsync.age")));
    assert_eq!(same_week, None);
    assert_eq!(
        next_week,
        Some(dir.path().join("budget-2026-11-09.bsync.age"))
    );
    assert_eq!(
        files_in(dir.path()),
        ["budget-2026-11-02.bsync.age", "budget-2026-11-09.bsync.age"]
    );
    assert_eq!(backup::latest_weekly(dir.path()).unwrap(), next_week);
    let mut fresh = Store::open_in_memory().unwrap();
    backup::restore(&mut fresh, next_week.as_ref().unwrap(), "pass").unwrap();
    assert_eq!(
        accounts::list(&fresh).unwrap(),
        accounts::list(&store).unwrap()
    );
}

#[test]
fn the_weekly_backup_keeps_the_last_8_and_leaves_other_files_alone() {
    let store = store_with_an_account();
    let dir = tempfile::tempdir().unwrap();
    // Nine older weekly Backups across a year boundary, and the user's own files.
    let older = [
        "2025-11-03",
        "2025-11-10",
        "2025-11-17",
        "2025-11-24",
        "2025-12-01",
        "2025-12-08",
        "2025-12-15",
        "2025-12-22",
        "2025-12-29",
    ];
    for date in older {
        std::fs::write(dir.path().join(backup::file_name(date)), b"old").unwrap();
    }
    std::fs::write(dir.path().join("my-notes.txt"), b"keep me").unwrap();
    std::fs::write(dir.path().join("budget-export.bsync.age"), b"manual").unwrap();

    backup::run_weekly(&store, dir.path(), "pass", "2026-01-05").unwrap();

    assert_eq!(
        files_in(dir.path()),
        [
            "budget-2025-11-17.bsync.age",
            "budget-2025-11-24.bsync.age",
            "budget-2025-12-01.bsync.age",
            "budget-2025-12-08.bsync.age",
            "budget-2025-12-15.bsync.age",
            "budget-2025-12-22.bsync.age",
            "budget-2025-12-29.bsync.age",
            "budget-2026-01-05.bsync.age",
            "budget-export.bsync.age",
            "my-notes.txt",
        ]
    );
}

#[test]
fn today_is_a_calendar_date() {
    let today = backup::today_utc();
    assert!(budget_core::date::is_valid(&today), "{today}");
    assert!(today.as_str() >= "2026-01-01");
}

#[test]
fn utc_dates_are_counted_from_the_unix_epoch() {
    assert_eq!(backup::utc_date(0), "1970-01-01");
    assert_eq!(backup::utc_date(951_782_400_000), "2000-02-29");
    assert_eq!(backup::utc_date(1_798_761_599_000), "2026-12-31");
}
