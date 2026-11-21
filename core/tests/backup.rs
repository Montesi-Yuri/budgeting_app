//! Backup: an encrypted file made from the sync log, restored by applying
//! every change in it (sync-design.md §4).

use budget_core::accounts::{self, AccountKind, NewAccount};
use budget_core::{backup, settings, Error, Store};

fn new_account(name: &str, opening_minor: i64, is_main: bool) -> NewAccount {
    NewAccount {
        name: name.into(),
        kind: AccountKind::Current,
        opening_minor,
        opening_date: "2026-01-01".into(),
        is_main,
    }
}

/// A Device with some history: Accounts created, edited and deleted, and the Currency set.
fn store_with_history() -> Store {
    let mut store = Store::open_in_memory().unwrap();
    settings::set_currency(&mut store, "EUR").unwrap();
    let checking = accounts::create(&mut store, new_account("Checking", 120_000, true)).unwrap();
    accounts::create(&mut store, new_account("Savings", 500_000, false)).unwrap();
    let old = accounts::create(&mut store, new_account("Old card", -3_000, false)).unwrap();
    accounts::update(
        &mut store,
        accounts::Account {
            name: "Main checking".into(),
            ..checking
        },
    )
    .unwrap();
    accounts::delete(&mut store, &old.id).unwrap();
    store
}

#[test]
fn restoring_into_an_empty_database_on_a_new_installation_gives_the_same_data() {
    let original = store_with_history();
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("budget.bsync.age");

    backup::export(&original, &file, "correct horse").unwrap();
    let mut fresh = Store::open_in_memory().unwrap();
    let report = backup::restore(&mut fresh, &file, "correct horse").unwrap();

    assert_eq!(
        accounts::list(&fresh).unwrap(),
        accounts::list(&original).unwrap()
    );
    assert_eq!(settings::currency(&fresh).unwrap(), Some("EUR".to_owned()));
    let original_ops = original.ops_since(original.device_id(), 0).unwrap();
    assert_eq!(report.added, original_ops.len() as u64);
    assert_eq!(report.already_present, 0);
    assert_eq!(
        fresh.version_vector().unwrap(),
        original.version_vector().unwrap()
    );
}

#[test]
fn restoring_into_the_same_database_twice_changes_nothing_the_second_time() {
    let mut store = store_with_history();
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("budget.bsync.age");
    backup::export(&store, &file, "pass").unwrap();
    let ops = store.ops_since(store.device_id(), 0).unwrap();

    let into_own = backup::restore(&mut store, &file, "pass").unwrap();
    let accounts_after_first = accounts::list(&store).unwrap();
    let again = backup::restore(&mut store, &file, "pass").unwrap();

    assert_eq!(
        into_own,
        backup::RestoreReport {
            added: 0,
            already_present: ops.len() as u64
        }
    );
    assert_eq!(again, into_own);
    assert_eq!(accounts::list(&store).unwrap(), accounts_after_first);
    assert_eq!(store.ops_since(store.device_id(), 0).unwrap(), ops);
}

#[test]
fn restoring_into_an_existing_database_only_adds_what_is_missing() {
    let original = store_with_history();
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("budget.bsync.age");
    backup::export(&original, &file, "pass").unwrap();
    let mut restored = Store::open_in_memory().unwrap();
    backup::restore(&mut restored, &file, "pass").unwrap();

    // Both Devices go on: the original adds an Account, the restored one renames one.
    let mut original = original;
    accounts::create(&mut original, new_account("Holiday fund", 0, false)).unwrap();
    backup::export(&original, &file, "pass").unwrap();
    let savings = accounts::list(&restored)
        .unwrap()
        .into_iter()
        .find(|a| a.account.name == "Savings")
        .unwrap()
        .account;
    accounts::update(
        &mut restored,
        accounts::Account {
            name: "Rainy day".into(),
            ..savings
        },
    )
    .unwrap();

    let report = backup::restore(&mut restored, &file, "pass").unwrap();

    assert_eq!(report.added, 1);
    let names: Vec<String> = accounts::list(&restored)
        .unwrap()
        .into_iter()
        .map(|a| a.account.name)
        .collect();
    assert_eq!(names, ["Main checking", "Holiday fund", "Rainy day"]);
}

#[test]
fn a_wrong_passphrase_gives_a_clear_error_and_changes_nothing() {
    let original = store_with_history();
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("budget.bsync.age");
    backup::export(&original, &file, "right").unwrap();
    let mut other = Store::open_in_memory().unwrap();
    settings::set_currency(&mut other, "USD").unwrap();

    let err = backup::restore(&mut other, &file, "wrong").unwrap_err();

    assert!(matches!(err, Error::WrongPassphrase), "{err:?}");
    assert!(err.to_string().contains("passphrase is wrong"));
    assert!(accounts::list(&other).unwrap().is_empty());
    assert_eq!(settings::currency(&other).unwrap(), Some("USD".to_owned()));
    assert_eq!(other.version_vector().unwrap().len(), 1);
}

#[test]
fn a_file_that_is_not_a_backup_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("notes.txt");
    std::fs::write(&file, "groceries 12.50").unwrap();
    let mut store = Store::open_in_memory().unwrap();

    let err = backup::restore(&mut store, &file, "pass").unwrap_err();

    assert!(matches!(err, Error::NotABackup(_)), "{err:?}");
}

#[test]
fn an_export_needs_a_passphrase() {
    let store = store_with_history();
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("budget.bsync.age");

    let err = backup::export(&store, &file, "").unwrap_err();

    assert!(matches!(err, Error::Invalid(_)), "{err:?}");
    assert!(!file.exists());
}

#[test]
fn the_restoring_device_keeps_its_own_identity_and_no_peers_come_with_the_backup() {
    let original = store_with_history();
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("budget.bsync.age");
    backup::export(&original, &file, "pass").unwrap();
    let mut fresh = Store::open_in_memory().unwrap();
    let own_id = fresh.device_id().to_owned();

    backup::restore(&mut fresh, &file, "pass").unwrap();
    accounts::create(&mut fresh, new_account("New here", 0, false)).unwrap();

    assert_eq!(fresh.device_id(), own_id);
    let devices: Vec<(String, bool)> = {
        let mut stmt = fresh
            .conn()
            .prepare("SELECT device_id, is_self FROM device")
            .unwrap();
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    };
    assert_eq!(devices, [(own_id.clone(), true)]);
    let new_ops = fresh.ops_since(&own_id, 0).unwrap();
    assert_eq!(new_ops.len(), 1);
    assert!(
        new_ops[0].hlc
            > original
                .ops_since(original.device_id(), 0)
                .unwrap()
                .last()
                .unwrap()
                .hlc
    );
}

/// A synced table that only this test knows about, standing in for the tables
/// later tickets add: restore must not need code per table.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct Note {
    id: String,
    account_id: String,
    text: Option<String>,
    pinned: bool,
    weight: f64,
}

const NOTE_TABLE: &str = "CREATE TABLE test_note (
    id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL REFERENCES account(id),
    text TEXT,
    pinned INTEGER NOT NULL,
    weight REAL NOT NULL,
    updated_hlc TEXT NOT NULL,
    deleted INTEGER NOT NULL DEFAULT 0)";

impl budget_core::store::SyncedRow for Note {
    const ENTITY: &'static str = "test_note";
    fn id(&self) -> &str {
        &self.id
    }
    fn save(
        &self,
        conn: &rusqlite::Connection,
        hlc: &budget_core::hlc::Hlc,
    ) -> rusqlite::Result<()> {
        conn.execute(
            "INSERT INTO test_note (id, account_id, text, pinned, weight, updated_hlc, deleted)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0)
             ON CONFLICT(id) DO UPDATE SET account_id = excluded.account_id, text = excluded.text,
               pinned = excluded.pinned, weight = excluded.weight,
               updated_hlc = excluded.updated_hlc, deleted = 0",
            rusqlite::params![
                self.id,
                self.account_id,
                self.text,
                self.pinned,
                self.weight,
                hlc.to_string()
            ],
        )?;
        Ok(())
    }
}

fn notes(store: &Store) -> Vec<(String, String, Option<String>, bool, f64, bool)> {
    let mut stmt = store
        .conn()
        .prepare("SELECT id, account_id, text, pinned, weight, deleted FROM test_note ORDER BY id")
        .unwrap();
    stmt.query_map([], |r| {
        Ok((
            r.get(0)?,
            r.get(1)?,
            r.get(2)?,
            r.get(3)?,
            r.get(4)?,
            r.get(5)?,
        ))
    })
    .unwrap()
    .collect::<Result<_, _>>()
    .unwrap()
}

#[test]
fn restore_works_for_synced_tables_it_has_no_code_for() {
    let mut original = Store::open_in_memory().unwrap();
    original.conn().execute_batch(NOTE_TABLE).unwrap();
    let account = accounts::create(&mut original, new_account("Checking", 0, true)).unwrap();
    let note = |id: &str, text: Option<&str>| Note {
        id: id.into(),
        account_id: account.id.clone(),
        text: text.map(Into::into),
        pinned: true,
        weight: 1.5,
    };
    original
        .write(|w| {
            w.upsert(&note("n1", Some("rent")))?;
            w.upsert(&note("n2", None))?;
            w.upsert(&note("n3", Some("gone")))?;
            w.delete::<Note>("n3")
        })
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("budget.bsync.age");
    backup::export(&original, &file, "pass").unwrap();
    let mut fresh = Store::open_in_memory().unwrap();
    fresh.conn().execute_batch(NOTE_TABLE).unwrap();

    backup::restore(&mut fresh, &file, "pass").unwrap();

    assert_eq!(notes(&fresh), notes(&original));
    assert_eq!(notes(&fresh).len(), 3);
}

#[test]
fn a_backup_with_changes_to_data_this_database_does_not_have_is_refused_without_changes() {
    let mut original = Store::open_in_memory().unwrap();
    original.conn().execute_batch(NOTE_TABLE).unwrap();
    let account = accounts::create(&mut original, new_account("Checking", 0, true)).unwrap();
    original
        .write(|w| {
            w.upsert(&Note {
                id: "n1".into(),
                account_id: account.id.clone(),
                text: None,
                pinned: false,
                weight: 0.0,
            })
        })
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("budget.bsync.age");
    backup::export(&original, &file, "pass").unwrap();
    let mut fresh = Store::open_in_memory().unwrap();

    let err = backup::restore(&mut fresh, &file, "pass").unwrap_err();

    assert!(matches!(err, Error::NotABackup(_)), "{err:?}");
    assert!(accounts::list(&fresh).unwrap().is_empty());
    assert_eq!(fresh.version_vector().unwrap().len(), 0);
}
