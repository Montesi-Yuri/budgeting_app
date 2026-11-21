//! The apply path for changes received from another Device
//! (sync-design.md §1.4): idempotent and order-independent.

mod common;

use budget_core::accounts::{self, Account, AccountKind};
use budget_core::store::{Op, OpKind};
use budget_core::{Error, Store};
use common::{all_ops, synced_rows};

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

fn names(store: &Store) -> Vec<String> {
    accounts::list(store)
        .unwrap()
        .into_iter()
        .map(|a| a.account.name)
        .collect()
}

#[test]
fn a_new_change_from_another_device_is_recorded_in_the_sync_log_and_updates_the_row() {
    let mut pc = Store::open_in_memory().unwrap();
    let mut phone = Store::open_in_memory().unwrap();
    pc.write(|w| w.upsert(&account("acc-1", "Checking")))
        .unwrap();

    let report = phone.apply_remote(&all_ops(&pc)).unwrap();

    assert_eq!(report.new_ops, 1);
    assert_eq!(names(&phone), ["Checking"]);
    assert_eq!(all_ops(&phone), all_ops(&pc));
    assert_eq!(
        phone.version_vector().unwrap(),
        pc.version_vector().unwrap()
    );
}

#[test]
fn receiving_the_same_change_twice_does_nothing_the_second_time() {
    let mut pc = Store::open_in_memory().unwrap();
    let mut phone = Store::open_in_memory().unwrap();
    pc.write(|w| w.upsert(&account("acc-1", "Checking")))
        .unwrap();
    phone.apply_remote(&all_ops(&pc)).unwrap();

    let again = phone.apply_remote(&all_ops(&pc)).unwrap();

    assert_eq!(again.new_ops, 0);
    assert_eq!(all_ops(&phone).len(), 1);
    assert_eq!(names(&phone), ["Checking"]);
}

#[test]
fn when_both_devices_edit_the_same_row_the_later_edit_wins_on_both() {
    let mut pc = Store::open_in_memory().unwrap();
    let mut phone = Store::open_in_memory().unwrap();
    pc.set_wall_clock(|| 1_000_000);
    phone.set_wall_clock(|| 1_000_000);
    pc.write(|w| w.upsert(&account("acc-1", "Checking")))
        .unwrap();
    phone.apply_remote(&all_ops(&pc)).unwrap();

    // Offline on both: the phone's edit is made later.
    pc.set_wall_clock(|| 2_000_000);
    phone.set_wall_clock(|| 3_000_000);
    pc.write(|w| w.upsert(&account("acc-1", "Earlier name")))
        .unwrap();
    phone
        .write(|w| w.upsert(&account("acc-1", "Later name")))
        .unwrap();

    let from_phone = all_ops(&phone);
    let from_pc = all_ops(&pc);
    pc.apply_remote(&from_phone).unwrap();
    phone.apply_remote(&from_pc).unwrap();

    assert_eq!(names(&pc), ["Later name"]);
    assert_eq!(names(&phone), ["Later name"]);
}

#[test]
fn a_local_write_after_receiving_a_change_is_stamped_after_it() {
    let mut pc = Store::open_in_memory().unwrap();
    let mut phone = Store::open_in_memory().unwrap();
    // The phone's wall clock is far behind the PC's.
    pc.set_wall_clock(|| 9_000_000);
    phone.set_wall_clock(|| 1_000);
    pc.write(|w| w.upsert(&account("acc-1", "From PC")))
        .unwrap();
    phone.apply_remote(&all_ops(&pc)).unwrap();

    phone
        .write(|w| w.upsert(&account("acc-1", "Edited on phone")))
        .unwrap();
    pc.apply_remote(&all_ops(&phone)).unwrap();

    assert_eq!(names(&pc), ["Edited on phone"]);
}

#[test]
fn a_delete_from_another_device_soft_deletes_the_row() {
    let mut pc = Store::open_in_memory().unwrap();
    let mut phone = Store::open_in_memory().unwrap();
    pc.write(|w| w.upsert(&account("acc-1", "Checking")))
        .unwrap();
    phone.apply_remote(&all_ops(&pc)).unwrap();
    pc.write(|w| w.delete::<Account>("acc-1")).unwrap();

    phone.apply_remote(&all_ops(&pc)).unwrap();

    assert!(names(&phone).is_empty());
    assert_eq!(synced_rows(&phone), synced_rows(&pc));
}

#[test]
fn a_delete_that_arrives_before_the_row_it_deletes_still_wins() {
    let mut pc = Store::open_in_memory().unwrap();
    let mut laptop = Store::open_in_memory().unwrap();
    let mut phone = Store::open_in_memory().unwrap();
    pc.write(|w| w.upsert(&account("acc-1", "Checking")))
        .unwrap();
    laptop.apply_remote(&all_ops(&pc)).unwrap();
    laptop.write(|w| w.delete::<Account>("acc-1")).unwrap();

    // The phone gets the laptop's delete first, then the PC's create.
    let delete = laptop.ops_since(laptop.device_id(), 0).unwrap();
    let create = pc.ops_since(pc.device_id(), 0).unwrap();
    phone.apply_remote(&delete).unwrap();
    phone.apply_remote(&create).unwrap();

    assert!(names(&phone).is_empty());
    assert_eq!(synced_rows(&phone), synced_rows(&laptop));
}

#[test]
fn an_edit_made_after_a_delete_brings_the_row_back() {
    let mut pc = Store::open_in_memory().unwrap();
    let mut phone = Store::open_in_memory().unwrap();
    pc.set_wall_clock(|| 1_000_000);
    phone.set_wall_clock(|| 1_000_000);
    pc.write(|w| w.upsert(&account("acc-1", "Checking")))
        .unwrap();
    phone.apply_remote(&all_ops(&pc)).unwrap();
    pc.write(|w| w.delete::<Account>("acc-1")).unwrap();
    phone.set_wall_clock(|| 2_000_000);
    phone
        .write(|w| w.upsert(&account("acc-1", "Still used")))
        .unwrap();

    let from_phone = all_ops(&phone);
    let from_pc = all_ops(&pc);
    pc.apply_remote(&from_phone).unwrap();
    phone.apply_remote(&from_pc).unwrap();

    assert_eq!(names(&pc), ["Still used"]);
    assert_eq!(synced_rows(&pc), synced_rows(&phone));
}

#[test]
fn a_batch_is_applied_in_one_transaction_so_a_bad_change_keeps_none_of_it() {
    let mut pc = Store::open_in_memory().unwrap();
    let mut phone = Store::open_in_memory().unwrap();
    pc.write(|w| w.upsert(&account("acc-1", "Checking")))
        .unwrap();
    pc.write(|w| w.upsert(&account("acc-2", "Savings")))
        .unwrap();
    let mut batch = all_ops(&pc);
    batch[1].payload = Some(vec![0xff, 0x00]); // not a valid row

    let result = phone.apply_remote(&batch);

    assert!(matches!(result, Err(Error::Codec(_))), "{result:?}");
    assert!(all_ops(&phone).is_empty());
    assert!(names(&phone).is_empty());
}

#[test]
fn a_change_to_an_unknown_entity_is_rejected() {
    let mut phone = Store::open_in_memory().unwrap();
    let pc = Store::open_in_memory().unwrap();
    let op = Op {
        origin: pc.device_id().to_owned(),
        seq: 1,
        hlc: "0000000001000000-0000-pc".parse().unwrap(),
        entity: "no_such_table".into(),
        entity_id: "x".into(),
        kind: OpKind::Delete,
        payload: None,
    };

    let result = phone.apply_remote(&[op]);

    assert!(matches!(result, Err(Error::Codec(_))), "{result:?}");
    assert!(all_ops(&phone).is_empty());
}
