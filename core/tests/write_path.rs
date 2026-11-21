//! The single write path: every change updates the row and appends an op to
//! the sync log in one DB transaction (sync-design.md §1.4).

use budget_core::accounts::{Account, AccountKind};
use budget_core::store::{OpKind, Store};

fn account(id: &str, name: &str) -> Account {
    Account {
        id: id.into(),
        name: name.into(),
        kind: AccountKind::Current,
        opening_minor: 10_000,
        opening_date: "2026-01-01".into(),
        is_main: false,
    }
}

#[test]
fn a_write_saves_the_row_and_appends_an_op_with_the_full_row_as_payload() {
    let mut store = Store::open_in_memory().unwrap();
    let row = account("acc-1", "Checking");

    let hlc = store.write(|w| w.upsert(&row)).unwrap();

    let ops = store.ops_since(store.device_id(), 0).unwrap();
    assert_eq!(ops.len(), 1);
    let op = &ops[0];
    assert_eq!(op.origin, store.device_id());
    assert_eq!(op.seq, 1);
    assert_eq!(op.hlc, hlc);
    assert_eq!(op.entity, "account");
    assert_eq!(op.entity_id, "acc-1");
    assert_eq!(op.kind, OpKind::Upsert);
    let decoded: Account = op.decode_payload().unwrap().unwrap();
    assert_eq!(decoded, row);
}

#[test]
fn when_the_write_fails_neither_the_row_nor_the_op_is_kept() {
    let mut store = Store::open_in_memory().unwrap();

    let result: budget_core::Result<()> = store.write(|w| {
        w.upsert(&account("acc-1", "Checking"))?;
        Err(budget_core::Error::invalid("something went wrong halfway"))
    });

    assert!(result.is_err());
    assert!(store.ops_since(store.device_id(), 0).unwrap().is_empty());
    let rows: i64 = store
        .conn()
        .query_row("SELECT COUNT(*) FROM account", [], |r| r.get(0))
        .unwrap();
    assert_eq!(rows, 0);
}

#[test]
fn a_delete_is_a_soft_delete_logged_as_a_delete_op() {
    let mut store = Store::open_in_memory().unwrap();
    store
        .write(|w| w.upsert(&account("acc-1", "Checking")))
        .unwrap();

    let hlc = store.write(|w| w.delete::<Account>("acc-1")).unwrap();

    // The row stays as a tombstone stamped with the delete, so the delete can sync.
    let (deleted, updated_hlc): (bool, String) = store
        .conn()
        .query_row(
            "SELECT deleted, updated_hlc FROM account WHERE id = 'acc-1'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert!(deleted);
    assert_eq!(updated_hlc, hlc.to_string());

    let ops = store.ops_since(store.device_id(), 1).unwrap();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].kind, OpKind::Delete);
    assert_eq!(ops[0].entity_id, "acc-1");
    assert_eq!(ops[0].payload, None);
}

#[test]
fn deleting_a_row_that_does_not_exist_fails_and_logs_nothing() {
    let mut store = Store::open_in_memory().unwrap();
    let result = store.write(|w| w.delete::<Account>("missing"));
    assert!(matches!(result, Err(budget_core::Error::NotFound { .. })));
    assert!(store.ops_since(store.device_id(), 0).unwrap().is_empty());
}

#[test]
fn ops_are_numbered_without_gaps_and_in_clock_order() {
    let mut store = Store::open_in_memory().unwrap();
    store
        .write(|w| {
            w.upsert(&account("acc-1", "Checking"))?;
            w.upsert(&account("acc-2", "Savings"))
        })
        .unwrap();
    store
        .write(|w| w.upsert(&account("acc-1", "Main checking")))
        .unwrap();

    let ops = store.ops_since(store.device_id(), 0).unwrap();
    assert_eq!(ops.iter().map(|o| o.seq).collect::<Vec<_>>(), vec![1, 2, 3]);
    assert!(ops.windows(2).all(|pair| pair[0].hlc < pair[1].hlc));
    assert_eq!(
        store.version_vector().unwrap().get(store.device_id()),
        Some(&3)
    );
}

#[test]
fn reopening_the_database_keeps_the_device_id_and_the_clock_keeps_moving_forward() {
    let dir = std::env::temp_dir().join(format!("budget-core-test-{}", uuid_like()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("budget.db");

    let (device_id, first_hlc) = {
        let mut store = Store::open(&path).unwrap();
        store.set_wall_clock(|| 50_000);
        let hlc = store
            .write(|w| w.upsert(&account("acc-1", "Checking")))
            .unwrap();
        (store.device_id().to_owned(), hlc)
    };

    let mut reopened = Store::open(&path).unwrap();
    reopened.set_wall_clock(|| 10_000); // the wall clock went backwards meanwhile
    let second_hlc = reopened
        .write(|w| w.upsert(&account("acc-1", "Renamed")))
        .unwrap();

    assert_eq!(reopened.device_id(), device_id);
    assert!(second_hlc > first_hlc);
    assert_eq!(
        reopened.schema_version().unwrap(),
        budget_core::SCHEMA_VERSION
    );
    drop(reopened);
    let _ = std::fs::remove_dir_all(&dir);
}

fn uuid_like() -> String {
    format!("{}-{:?}", std::process::id(), std::thread::current().id()).replace(['(', ')'], "")
}

#[test]
fn every_new_database_gets_its_own_device_id() {
    let a = Store::open_in_memory().unwrap();
    let b = Store::open_in_memory().unwrap();
    assert_ne!(a.device_id(), b.device_id());
    assert_eq!(a.device_id().len(), 36);
}
