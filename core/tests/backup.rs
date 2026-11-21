//! Backup: an encrypted file made from the sync log, restored by applying
//! every change in it (sync-design.md §4).

mod common;

use std::io::Write;

use budget_core::accounts::{self, AccountKind, NewAccount};
use budget_core::categories::{self, CategoryKind, NewCategory};
use budget_core::transactions::{self, NewManualTransaction};
use budget_core::{backup, settings, Error, Store};
use common::synced_rows;

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

#[test]
fn a_backup_with_categories_and_transactions_restores_them() {
    let mut original = Store::open_in_memory().unwrap();
    let account = accounts::create(&mut original, new_account("Checking", 100_000, true)).unwrap();
    let category = |store: &mut Store, name: &str, parent_id: Option<String>| {
        categories::create(
            store,
            NewCategory {
                name: name.into(),
                kind: CategoryKind::Expense,
                parent_id,
            },
        )
        .unwrap()
    };
    let food = category(&mut original, "Food", None);
    let groceries = category(&mut original, "Groceries", Some(food.id.clone()));
    let restaurants = category(&mut original, "Restaurants", Some(food.id.clone()));
    let spend = |store: &mut Store, description: &str, category_id: Option<String>| {
        transactions::create_manual(
            store,
            NewManualTransaction {
                account_id: account.id.clone(),
                date: "2026-11-03".into(),
                amount_minor: -2_500,
                description: description.into(),
                category_id,
                note: Some("receipt".into()),
            },
        )
        .unwrap()
    };
    let shop = spend(&mut original, "Esselunga", None);
    transactions::set_category(&mut original, &shop.id, Some(&groceries.id)).unwrap();
    spend(&mut original, "Pizza", Some(restaurants.id.clone()));
    let mistake = spend(&mut original, "Typo", None);
    transactions::delete(&mut original, &mistake.id).unwrap();
    categories::delete(&mut original, &restaurants.id).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("budget.bsync.age");
    backup::export(&original, &file, "pass").unwrap();

    let mut fresh = Store::open_in_memory().unwrap();
    let report = backup::restore(&mut fresh, &file, "pass").unwrap();

    assert_eq!(synced_rows(&fresh), synced_rows(&original));
    assert_eq!(
        categories::list(&fresh).unwrap(),
        categories::list(&original).unwrap()
    );
    assert_eq!(
        transactions::list_for_account(&fresh, &account.id).unwrap(),
        transactions::list_for_account(&original, &account.id).unwrap()
    );
    assert_eq!(
        transactions::list_uncategorised(&fresh).unwrap(),
        transactions::list_uncategorised(&original).unwrap()
    );
    assert_eq!(
        accounts::list(&fresh).unwrap(),
        accounts::list(&original).unwrap()
    );
    let all_ops = common::all_ops(&original).len() as u64;
    assert_eq!(
        report,
        backup::RestoreReport {
            added: all_ops,
            already_present: 0
        }
    );
}

/// A synced row type that is not registered in `SYNCED_ENTITIES`, standing in
/// for data from a damaged Backup or a build that doesn't know the entity.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct Note {
    id: String,
    text: String,
}

const NOTE_TABLE: &str = "CREATE TABLE test_note (
    id TEXT PRIMARY KEY,
    text TEXT NOT NULL,
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
            "INSERT INTO test_note (id, text, updated_hlc, deleted) VALUES (?1, ?2, ?3, 0)
             ON CONFLICT(id) DO UPDATE SET text = excluded.text,
               updated_hlc = excluded.updated_hlc, deleted = 0",
            rusqlite::params![self.id, self.text, hlc.to_string()],
        )?;
        Ok(())
    }
}

#[test]
fn a_backup_with_changes_to_an_unregistered_entity_is_refused_without_changes() {
    let mut original = store_with_history();
    original.conn().execute_batch(NOTE_TABLE).unwrap();
    original
        .write(|w| {
            w.upsert(&Note {
                id: "n1".into(),
                text: "rent".into(),
            })
        })
        .unwrap();
    // A registered change after the unknown one, so a partial apply would show.
    accounts::create(&mut original, new_account("After", 0, false)).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("budget.bsync.age");
    backup::export(&original, &file, "pass").unwrap();
    // The table exists here too: it is the registry that refuses, not SQLite.
    let mut other = Store::open_in_memory().unwrap();
    other.conn().execute_batch(NOTE_TABLE).unwrap();
    settings::set_currency(&mut other, "USD").unwrap();
    let rows_before = synced_rows(&other);
    let vv_before = other.version_vector().unwrap();

    let err = backup::restore(&mut other, &file, "pass").unwrap_err();

    assert!(matches!(err, Error::NotABackup(_)), "{err:?}");
    assert!(err.to_string().contains("test_note"), "{err}");
    assert_eq!(synced_rows(&other), rows_before);
    assert_eq!(other.version_vector().unwrap(), vv_before);
    let notes: i64 = other
        .conn()
        .query_row("SELECT COUNT(*) FROM test_note", [], |r| r.get(0))
        .unwrap();
    assert_eq!(notes, 0);
}

/// Encrypt `content` (CBOR) with `passphrase`, the way a Backup is.
fn write_encrypted(path: &std::path::Path, content: &ciborium::Value, passphrase: &str) {
    let mut plain = Vec::new();
    ciborium::into_writer(content, &mut plain).unwrap();
    let encryptor = age::Encryptor::with_user_passphrase(age::secrecy::SecretString::from(
        passphrase.to_owned(),
    ));
    let mut encrypted = Vec::new();
    let mut writer = encryptor.wrap_output(&mut encrypted).unwrap();
    writer.write_all(&plain).unwrap();
    writer.finish().unwrap();
    std::fs::write(path, encrypted).unwrap();
}

#[test]
fn a_backup_from_a_newer_schema_is_refused_without_changes() {
    use ciborium::Value;
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("budget.bsync.age");
    let content = Value::Map(vec![
        (
            Value::Text("magic".into()),
            Value::Text("budget-backup".into()),
        ),
        (Value::Text("format".into()), Value::Integer(1.into())),
        (Value::Text("schema".into()), Value::Integer(999.into())),
        (Value::Text("ops".into()), Value::Array(vec![])),
    ]);
    write_encrypted(&file, &content, "pass");
    let mut store = store_with_history();
    let rows_before = synced_rows(&store);

    let err = backup::restore(&mut store, &file, "pass").unwrap_err();

    assert!(
        matches!(err, Error::SchemaTooNew { found: 999, .. }),
        "{err:?}"
    );
    assert_eq!(synced_rows(&store), rows_before);
}
