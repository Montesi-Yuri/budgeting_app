//! Finding a paired Device on the local network by its device id
//! (sync-design.md §3.4): the PC announces itself with mDNS while Sync is on,
//! and the phone looks for it when its last known address fails.
//!
//! The mDNS instances here use only the loopback interface and a test UDP
//! port instead of 5353, so the tests neither depend on nor touch the real
//! network.

use std::time::Duration;

use budget_core::sync::{Discovery, Mdns};

/// Not 5353, so the tests neither see nor disturb real mDNS traffic.
const TEST_MDNS_PORT: u16 = 25353;

fn mdns() -> Mdns {
    Mdns::loopback_only(TEST_MDNS_PORT)
}

fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[test]
fn an_announced_device_is_found_by_its_device_id() {
    let pc = mdns();
    let phone = mdns();
    let pc_id = new_id();

    let _announced = pc.announce(&pc_id, "Yuri-PC", 47999).unwrap();
    let found = phone.find(&pc_id, Duration::from_secs(5));

    assert_eq!(found, vec!["127.0.0.1:47999".to_owned()]);
}

#[test]
fn another_devices_announcement_is_not_taken_for_the_paired_pc() {
    let laptop = mdns();
    let phone = mdns();
    let _announced = laptop.announce(&new_id(), "Laptop", 47998).unwrap();

    let found = phone.find(&new_id(), Duration::from_millis(1500));

    assert_eq!(found, Vec::<String>::new());
}

#[test]
fn a_device_that_stopped_announcing_is_no_longer_found() {
    let pc = mdns();
    let pc_id = new_id();
    let announced = pc.announce(&pc_id, "Yuri-PC", 47997).unwrap();
    assert!(!mdns().find(&pc_id, Duration::from_secs(5)).is_empty());

    drop(announced);

    assert_eq!(
        mdns().find(&pc_id, Duration::from_millis(1500)),
        Vec::<String>::new()
    );
}
